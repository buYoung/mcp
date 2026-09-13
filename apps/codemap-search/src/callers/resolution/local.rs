//! Conservative lexical lookup for languages without module/type resolution.
//! Receiver hints and repository-wide spelling matches are never definition evidence.

use super::*;

pub(super) fn supports(path: &str) -> bool {
    crate::lang::spec_for_path(Path::new(path)).is_some_and(|spec| {
        matches!(
            spec.language_name(),
            "python"
                | "typescript"
                | "javascript"
                | "java"
                | "csharp"
                | "php"
                | "ruby"
                | "lua"
                | "kotlin"
                | "swift"
                | "dart"
                | "scala"
                | "groovy"
                | "powershell"
                | "c"
                | "cpp"
                | "asm"
                | "vue"
                | "astro"
                | "svelte"
        )
    })
}

fn lexical_scope(mut node: Node) -> Node {
    while let Some(parent) = node.parent() {
        node = parent;
        if matches!(
            node.kind(),
            "program"
                | "module"
                | "source_file"
                | "translation_unit"
                | "chunk"
                | "block"
                | "statement_block"
                | "function_body"
                | "class_body"
                | "enum_body"
                | "namespace_body"
                | "declaration_list"
                | "template_body"
        ) {
            break;
        }
    }
    node
}

fn same_tree(mut left: Node, mut right: Node) -> bool {
    while let Some(parent) = left.parent() {
        left = parent;
    }
    while let Some(parent) = right.parent() {
        right = parent;
    }
    left.id() == right.id()
}

fn class_scope(mut node: Node) -> Option<usize> {
    while let Some(parent) = node.parent() {
        node = parent;
        if matches!(
            node.kind(),
            "class"
                | "class_definition"
                | "class_declaration"
                | "class_specifier"
                | "struct_specifier"
                | "class_statement"
                | "object_definition"
                | "object_declaration"
                | "trait_definition"
                | "interface_declaration"
                | "enum_declaration"
        ) {
            return Some(node.id());
        }
    }
    None
}

fn has_self_context(mut node: Node) -> bool {
    while let Some(parent) = node.parent() {
        node = parent;
        if matches!(
            node.kind(),
            "method_definition" | "method_declaration" | "class_method_definition"
        ) {
            return class_scope(node).is_some();
        }
        if matches!(
            node.kind(),
            "function_declaration"
                | "function_definition"
                | "function_expression"
                | "function_statement"
                | "anonymous_function_creation_expression"
                | "closure_expression"
        ) {
            return false;
        }
    }
    false
}

fn has_identifier(node: Node, source: &Source, name: &str) -> bool {
    if matches!(
        node.kind(),
        "identifier" | "variable_name" | "simple_identifier"
    ) && text(node, source).is_some_and(|value| value.trim_start_matches('$') == name)
    {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| has_identifier(child, source, name));
    found
}

fn has_parameter(mut at: Node, source: &Source, name: &str) -> bool {
    loop {
        if [
            "parameters",
            "parameter",
            "type_parameters",
            "declarator",
            "signature",
        ]
        .iter()
        .any(|field| {
            at.child_by_field_name(field)
                .is_some_and(|node| has_identifier(node, source, name))
        }) {
            return true;
        }
        // Some grammars expose formal parameters as named children without a field.
        let mut cursor = at.walk();
        if at.named_children(&mut cursor).any(|node| {
            matches!(
                node.kind(),
                "formal_parameters"
                    | "parameters"
                    | "parameter_list"
                    | "function_value_parameters"
                    | "lambda_parameters"
            ) && has_identifier(node, source, name)
        }) {
            return true;
        }
        let Some(parent) = at.parent() else {
            return false;
        };
        at = parent;
    }
}

fn current_symbol(source: &Source, symbol: &ExtractedSymbol) -> bool {
    source.extracted.symbols.iter().any(|current| {
        current.name == symbol.name && current.kind == symbol.kind && current.range == symbol.range
    })
}

fn has_binding(source: &Source, at: Node, name: &str, target: Option<&ExtractedSymbol>) -> bool {
    has_parameter(at, source, name)
        || source
            .extracted
            .navigation
            .as_ref()
            .is_some_and(|navigation| {
                navigation
                    .local_bindings
                    .iter()
                    .filter(|binding| binding.name.trim_start_matches('$') == name)
                    .any(|binding| {
                        if target.is_some_and(|target| target.range == binding.range) {
                            return false;
                        }
                        node_at(source, &binding.range).is_some_and(|node| {
                            same_tree(node, at) && contains(lexical_scope(node), at)
                        })
                    })
            })
}

impl<'a> SourceResolver<'a> {
    pub(super) fn resolve_local_call(
        &self,
        file: &'a ExtractedFile,
        call: &CallSite,
    ) -> Option<Target<'a>> {
        let source = self.source(&file.file_path)?;
        // Live extraction must still identify the same executable call.
        if !source
            .extracted
            .navigation
            .as_ref()?
            .calls
            .iter()
            .any(|current| current == call)
        {
            return None;
        }
        let at = node_at(&source, &call.range)?;
        let language = crate::lang::spec_for_path(Path::new(&file.file_path))?.language_name();
        if let Some(target) = self.relative_script_import(file, call, &source, at) {
            return Some(target);
        }
        let has_explicit_self = matches!(
            (language, call.receiver.as_deref()),
            ("php" | "powershell", Some("$this"))
                | (
                    "javascript" | "typescript" | "vue" | "astro" | "svelte",
                    Some("this")
                )
        ) && has_self_context(at);
        if !has_explicit_self
            && (call.receiver.is_some() || source.syntax.is_member_call(call) == Some(true))
        {
            return None;
        }
        let invocation = text(at, &source)?;
        let callee = if language == "asm" {
            invocation.split_whitespace().last().unwrap_or("")
        } else {
            invocation.split_once('(').map_or_else(
                || invocation.split_whitespace().next().unwrap_or(""),
                |(callee, _)| callee.trim(),
            )
        };
        if !has_explicit_self && callee != call.name {
            return None;
        }
        if !has_explicit_self
            && source
                .extracted
                .navigation
                .as_ref()?
                .imports
                .iter()
                .any(|import| {
                    (import.local_name == call.name || import.kind == ImportKind::Glob)
                        && node_at(&source, &import.range).is_some_and(|node| same_tree(node, at))
                })
        {
            return None;
        }
        let needs_explicit_member = matches!(
            language,
            "python"
                | "javascript"
                | "typescript"
                | "php"
                | "lua"
                | "powershell"
                | "vue"
                | "astro"
                | "svelte"
        );
        let mut candidates: Vec<_> = file
            .symbols
            .iter()
            .filter(|symbol| symbol.name == call.name)
            .map(|symbol| Target {
                file,
                symbol,
                is_precise: true,
            })
            .filter(|target| {
                target.file.file_path == file.file_path
                    && target.symbol.kind == "fn"
                    && (!has_explicit_self || target.symbol.owner.is_some())
                    && !(needs_explicit_member
                        && !has_explicit_self
                        && target.symbol.owner.is_some())
                    && current_symbol(&source, target.symbol)
            })
            .filter_map(|target| {
                let node = node_at(&source, &target.symbol.range)?;
                let visible = lexical_scope(node);
                (same_tree(node, at)
                    && (target.symbol.owner.is_none()
                        || class_scope(at).is_some() && class_scope(at) == class_scope(node))
                    && contains(visible, at)
                    && (has_explicit_self
                        || !has_binding(&source, at, &call.name, Some(target.symbol))))
                .then_some((visible.end_byte() - visible.start_byte(), target))
            })
            .collect();
        candidates.sort_by_key(|(size, _)| *size);
        let nearest = candidates.first()?.0;
        unique(
            candidates
                .into_iter()
                .take_while(|(size, _)| *size == nearest)
                .map(|(_, target)| target)
                .collect(),
        )
    }

    fn relative_script_import(
        &self,
        file: &ExtractedFile,
        call: &CallSite,
        source: &Source,
        at: Node,
    ) -> Option<Target<'a>> {
        let language = crate::lang::spec_for_path(Path::new(&file.file_path))?.language_name();
        if !matches!(
            language,
            "typescript" | "javascript" | "vue" | "astro" | "svelte"
        ) {
            return None;
        }
        let binding_name = call.receiver.as_deref().unwrap_or(&call.name);
        if has_binding(source, at, binding_name, None) {
            return None;
        }
        let imports: Vec<_> = source
            .extracted
            .navigation
            .as_ref()?
            .imports
            .iter()
            .filter(|import| {
                import.local_name == binding_name
                    && node_at(source, &import.range).is_some_and(|node| same_tree(node, at))
                    && matches!(
                        (&import.kind, call.receiver.is_some()),
                        (ImportKind::Named, false) | (ImportKind::Namespace, true)
                    )
            })
            .collect();
        if imports.len() != 1 {
            return None;
        }
        let import = imports[0];
        let path = import.source.as_deref()?;
        if !(path.starts_with("./") || path.starts_with("../")) {
            return None;
        }
        let relative = self.root.join(&file.file_path).parent()?.join(path);
        let extensions = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
        let paths = if relative.extension().is_some() {
            vec![relative]
        } else {
            extensions
                .iter()
                .flat_map(|ext| {
                    [
                        relative.with_extension(ext),
                        relative.join(format!("index.{ext}")),
                    ]
                })
                .collect()
        };
        let files: Vec<_> = paths
            .iter()
            .filter_map(|path| self.file(&crate::workspace::canonicalize_path_lenient(path)))
            .collect();
        if files.len() != 1 {
            return None;
        }
        let target_file = files[0];
        let target_source = self.source(&target_file.file_path)?;
        let name = if call.receiver.is_some() {
            call.name.as_str()
        } else {
            import.imported_name.as_deref()?
        };
        let targets = target_file
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .map(|symbol| Target {
                file: target_file,
                symbol,
                is_precise: true,
            })
            .filter(|target| {
                target.file.file_path == target_file.file_path
                    && target.symbol.kind == "fn"
                    && target.symbol.owner.is_none()
                    && target.symbol.flags.is_exported
                    && current_symbol(&target_source, target.symbol)
            })
            .collect();
        unique(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(sources: &[(&str, &str)]) -> (tempfile::TempDir, Vec<ExtractedFile>) {
        let (root, path) = crate::callers::fixtures::write_repo(sources);
        let files = sources
            .iter()
            .map(|(name, source)| {
                crate::parser::TreeSitterExtractor::new()
                    .extract(source, name)
                    .unwrap()
            })
            .collect();
        assert_eq!(root.path(), path);
        (root, files)
    }

    #[test]
    fn local_calls_do_not_guess_receivers_or_shadowed_parameters() {
        for (name, text) in [
            ("case.ts", "function target() {}\nfunction run() { target(); }\nfunction unknown(receiver: any) { receiver.target(); }\nfunction shadow(target: () => void) { target(); }\n"),
            ("case.py", "def target(): pass\ndef run(): target()\ndef unknown(receiver): receiver.target()\ndef shadow(target): target()\n"),
            ("case.c", "int target(void) { return 7; }\nint run(void) { return target(); }\nint shadow(int (*target)(void)) { return target(); }\n"),
        ] {
            let (root, files) = project(&[(name, text)]);
            let resolver = SourceResolver::new(&files, root.path());
            let calls = resolver.calls(&files[0]);
            assert!(!calls.is_empty(), "{name}");
            for call in &calls {
                let target = resolver.resolve_call(&files[0], call);
                if call.range.start_line == 2 {
                    assert!(target.is_some(), "local declaration missing: {name}: {call:?}");
                } else {
                    assert!(target.is_none(), "unproven target linked: {name}: {call:?}");
                }
            }
        }
    }

    #[test]
    fn explicit_script_imports_resolve_but_ambient_names_and_shadowed_imports_do_not() {
        let (root, files) = project(&[
            ("target.ts", "export function target() {}\n"),
            ("use.ts", "import { target as renamed } from './target';\nexport function run() { renamed(); }\nexport function shadow(renamed: () => void) { renamed(); }\n"),
            ("other.ts", "export function run() { target(); }\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let calls = resolver.calls(&files[1]);
        assert_eq!(
            resolver
                .resolve_call(&files[1], &calls[0])
                .unwrap()
                .file
                .file_path,
            "target.ts"
        );
        assert!(resolver.resolve_call(&files[1], &calls[1]).is_none());
        assert!(resolver
            .resolve_call(&files[2], &resolver.calls(&files[2])[0])
            .is_none());
    }

    #[test]
    fn component_calls_respect_script_and_markup_boundaries() {
        for extension in ["vue", "astro", "svelte"] {
            let name = format!("case.{extension}");
            let text = "<script>\nfunction target() {}\nfunction run() { target(); }\nconst literal = 'target()';\n</script>\n<div>target()</div>\n<script>\nfunction other() { target(); }\n</script>\n";
            let (root, files) = project(&[(&name, text)]);
            let resolver = SourceResolver::new(&files, root.path());
            let calls = resolver.calls(&files[0]);
            assert_eq!(
                calls.len(),
                2,
                "literal/markup call extracted: {extension}: {calls:?}"
            );
            assert!(
                resolver.resolve_call(&files[0], &calls[0]).is_some(),
                "actual script call missing: {extension}"
            );
            assert!(
                resolver.resolve_call(&files[0], &calls[1]).is_none(),
                "separate script scopes linked: {extension}"
            );
            let syntax = SourceSyntax::parse(&name, text.as_bytes()).unwrap();
            let markup = text.find("<div>target").unwrap() + "<div>".len();
            assert!(
                !syntax.is_code(markup..markup + 6),
                "markup accepted as code: {extension}"
            );
        }
    }

    #[test]
    fn class_members_require_the_languages_receiver_and_the_same_class_scope() {
        for (name, source, good_line) in [
            ("case.ts", "class A {\n target() {}\n run() { this.target(); }\n bare() { target(); }\n nested() { function inner() { this.target(); } }\n}\n", 3),
            ("case.php", "<?php class A {\n function target() {}\n function run() { $this->target(); }\n function bare() { target(); }\n}\n", 3),
            ("case.ps1", "class A {\n [void] target() {}\n [void] run() { $this.target() }\n [void] bare() { target }\n}\n", 3),
            ("case.cs", "class A { void target() {}\n void run() { target(); } }\nclass B { void wrong() { target(); } }\n", 2),
        ] {
            let (root, files) = project(&[(name, source)]);
            let resolver = SourceResolver::new(&files, root.path());
            let calls = resolver.calls(&files[0]);
            assert!(calls.len() >= 2, "missing call coverage: {name}: {calls:?}");
            for call in calls.iter().filter(|call| call.name == "target") {
                assert_eq!(resolver.resolve_call(&files[0], call).is_some(), call.range.start_line == good_line,
                    "wrong receiver/class attribution: {name}: {call:?}");
            }
        }
    }

    #[test]
    fn assembly_calls_only_resolve_labels_declared_in_the_same_source() {
        let (root, files) = project(&[
            (
                "a.s",
                "target:\n ret\ncaller:\n call target\n call external\n",
            ),
            ("b.s", "external:\n ret\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let calls = resolver.calls(&files[0]);
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            resolver
                .resolve_call(&files[0], &calls[0])
                .unwrap()
                .symbol
                .name,
            "target"
        );
        assert!(resolver.resolve_call(&files[0], &calls[1]).is_none());
    }

    #[test]
    fn dart_member_bodies_and_constructor_initializers_keep_their_full_ranges() {
        let source = "class Worker {\n Worker(int seed)\n   : value = seed;\n final int value;\n int target() {\n   return value;\n }\n int run() {\n   return target();\n }\n int shadow(int Function() target) {\n   return target();\n }\n}\n";
        let (root, files) = project(&[("worker.dart", source)]);
        let spans: Vec<_> = files[0]
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == "fn")
            .map(|symbol| {
                (
                    symbol.name.as_str(),
                    symbol.range.start_line,
                    symbol.range.end_line,
                )
            })
            .collect();
        assert!(spans.contains(&("Worker", 2, 3)), "{spans:?}");
        assert!(spans.contains(&("target", 5, 7)), "{spans:?}");
        assert!(spans.contains(&("run", 8, 10)), "{spans:?}");
        let resolver = SourceResolver::new(&files, root.path());
        assert_eq!(
            resolver.caller_name(&files[0], 9).as_deref(),
            Some("Worker.run")
        );
        let calls = resolver.calls(&files[0]);
        let direct = calls
            .iter()
            .find(|call| call.range.start_line == 9)
            .unwrap();
        assert_eq!(
            resolver
                .resolve_call(&files[0], direct)
                .unwrap()
                .symbol
                .range
                .end_line,
            7
        );
        let shadow = calls
            .iter()
            .find(|call| call.range.start_line == 12)
            .unwrap();
        assert!(resolver.resolve_call(&files[0], shadow).is_none());
    }
}
