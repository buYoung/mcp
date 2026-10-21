//! Conservative lexical lookup for languages without full module/type resolution. TypeScript
//! imports (relative or tsconfig paths) and declared class field types are source evidence;
//! receiver hints and repository-wide spelling matches never are.

use super::*;

mod script_modules;
mod typed_fields;

pub(super) use script_modules::ModulePaths;

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
                | "abstract_class_declaration"
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

/// Function declaration statements; a nested one is local to its enclosing function.
const LOCAL_FUNCTION_DECLARATIONS: &[&str] = &[
    "function_definition",
    "decorated_definition",
    "function_declaration",
    "local_function_statement",
    "function_item",
    "function_statement",
];

/// Declarations that name no type: such a variable may hold anything.
const UNTYPED: &[&str] = &[
    "var", "let", "const", "auto", "val", "dynamic", "any", "Any", "object", "Object", "unknown",
    "mixed",
];

fn is_simple_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// Whether an import source could name the module that holds `target_file`. Relative sources
/// stay open; any other source must share a path component with the target file.
fn module_may_hold(module: &str, target_file: &str) -> bool {
    if module.starts_with('.') {
        return true;
    }
    let components: HashSet<String> = target_file
        .split('/')
        .map(|part| part.split('.').next().unwrap_or(part).to_lowercase())
        .collect();
    module
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .filter(|segment| !segment.is_empty())
        .any(|segment| components.contains(&segment.to_lowercase()))
}

/// A bare callee or receiver bound only by imports from modules that cannot hold the target
/// file is another module's name, e.g. `from copy import copy` or `import json`.
fn import_rules_out(source: &Source, at: Node, call: &CallSite, target_file: &str) -> bool {
    let binding = match call.receiver.as_deref() {
        Some(receiver) if is_simple_identifier(receiver) => receiver.trim_start_matches('$'),
        Some(_) => return false,
        None => call.name.as_str(),
    };
    // A local binding or parameter of the same name shadows the import.
    if has_binding(source, at, binding, None) {
        return false;
    }
    let Some(navigation) = source.extracted.navigation.as_ref() else {
        return false;
    };
    let mut modules = navigation
        .imports
        .iter()
        .filter(|import| {
            import.kind != ImportKind::Glob
                && import.local_name.trim_start_matches('$') == binding
                && node_at(source, &import.range).is_some_and(|node| same_tree(node, at))
        })
        .map(|import| import.source.as_deref())
        .peekable();
    modules.peek().is_some()
        && modules.all(|module| module.is_some_and(|module| !module_may_hold(module, target_file)))
}

/// The written type of the nearest declaration of `name` visible at `at`: a local binding, else
/// a parameter. `None` when there is none or the nearest one has no written type.
fn declared_type(source: &Source, at: Node, name: &str) -> Option<String> {
    let navigation = source.extracted.navigation.as_ref()?;
    let nearest = navigation
        .local_bindings
        .iter()
        .filter(|binding| binding.name.trim_start_matches('$') == name)
        .filter_map(|binding| {
            let node = node_at(source, &binding.range)?;
            let scope = lexical_scope(node);
            (same_tree(node, at) && node.start_byte() <= at.start_byte() && contains(scope, at))
                .then(|| {
                    (
                        (
                            scope.end_byte() - scope.start_byte(),
                            std::cmp::Reverse(node.start_byte()),
                        ),
                        binding,
                    )
                })
        })
        .min_by_key(|(key, _)| *key);
    if let Some((_, binding)) = nearest {
        // An untyped nearest binding shadows a typed outer declaration.
        return binding.type_name.clone();
    }
    let mut current = Some(at);
    while let Some(node) = current {
        let mut lists: Vec<_> = ["parameters", "parameter"]
            .iter()
            .filter_map(|field| node.child_by_field_name(field))
            .collect();
        let mut cursor = node.walk();
        lists.extend(node.named_children(&mut cursor).filter(|child| {
            matches!(
                child.kind(),
                "formal_parameters"
                    | "parameters"
                    | "parameter_list"
                    | "function_value_parameters"
                    | "lambda_parameters"
            )
        }));
        for list in lists {
            let mut cursor = list.walk();
            for parameter in list.named_children(&mut cursor) {
                let declared = if matches!(
                    parameter.kind(),
                    "identifier" | "variable_name" | "simple_identifier"
                ) {
                    Some(parameter)
                } else {
                    parameter
                        .child_by_field_name("name")
                        .or_else(|| parameter.child_by_field_name("pattern"))
                        .or_else(|| parameter.named_child(0))
                };
                if declared
                    .and_then(|node| text(node, source))
                    .map(|value| value.trim_start_matches('$'))
                    != Some(name)
                {
                    continue;
                }
                return parameter
                    .child_by_field_name("type")
                    .and_then(|node| text(node, source))
                    .map(|value| value.trim().trim_start_matches(':').trim().to_string())
                    .filter(|value| !value.is_empty());
            }
        }
        current = node.parent();
    }
    None
}

impl<'a> SourceResolver<'a> {
    /// Source evidence that an unresolved call reaches something other than `target`: the
    /// calling file declares a type under the callee's name, the name is imported from an
    /// unrelated module, or the receiver is declared with a type that is neither the target's
    /// owner nor any indexed type.
    pub(super) fn rules_out_local_call(
        &self,
        file: &ExtractedFile,
        call: &CallSite,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        if !super::supports(&file.file_path) {
            return false;
        }
        // A function declaration directly inside another function is reachable by name only
        // from inside that function. Methods of local classes and functions stored in objects
        // are not covered: they can escape with their owner.
        if let Some(declaring) = self.files_by_path.get(target_file) {
            let parent = declaring
                .symbols
                .iter()
                .filter(|symbol| {
                    symbol.range != target.range
                        && symbol.range.start_line <= target.range.start_line
                        && target.range.end_line_inclusive() <= symbol.range.end_line_inclusive()
                })
                .min_by_key(|symbol| symbol.range.end_line_inclusive() - symbol.range.start_line);
            if let Some(parent) = parent.filter(|parent| crate::declarations::callable(parent)) {
                let is_declaration = self.source(target_file).is_some_and(|source| {
                    node_at(&source, &target.range)
                        .is_some_and(|node| LOCAL_FUNCTION_DECLARATIONS.contains(&node.kind()))
                });
                let is_inside = file.file_path == target_file
                    && parent.range.start_line <= call.range.start_line
                    && call.range.start_line <= parent.range.end_line_inclusive();
                if is_declaration && !is_inside {
                    return true;
                }
            }
        }
        // A bare `Name(...)` or `new Name(...)` constructs the type its own file declares.
        if call.receiver.is_none()
            && file.file_path != target_file
            && file
                .symbols
                .iter()
                .any(|symbol| symbol.name == call.name && crate::declarations::container(symbol))
        {
            return true;
        }
        let Some(source) = self.source(&file.file_path) else {
            return false;
        };
        let Some(at) = node_at(&source, &call.range) else {
            return false;
        };
        import_rules_out(&source, at, call, target_file)
            || self.receiver_type_rules_out(&source, at, call, target)
    }

    fn receiver_type_rules_out(
        &self,
        source: &Source,
        at: Node,
        call: &CallSite,
        target: &ExtractedSymbol,
    ) -> bool {
        let Some(owner) = target.owner.as_deref() else {
            return false;
        };
        let owner = owner.rsplit(['.', ':']).next().unwrap_or(owner);
        let Some(receiver) = call
            .receiver
            .as_deref()
            .filter(|receiver| is_simple_identifier(receiver))
        else {
            return false;
        };
        let Some(declared) = declared_type(source, at, receiver.trim_start_matches('$')) else {
            return false;
        };
        // Last segments of the written type names, e.g. `dict[str, t.Any]` -> dict, str, Any.
        let names: Vec<_> = declared
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == ':'))
            .filter_map(|qualified| qualified.rsplit(['.', ':']).find(|part| !part.is_empty()))
            .collect();
        !names.is_empty()
            && !names.iter().all(|name| UNTYPED.contains(name))
            && !names
                .iter()
                .any(|name| *name == owner || self.is_indexed_type(name))
    }

    fn is_indexed_type(&self, name: &str) -> bool {
        let names = self
            .type_names
            .borrow_mut()
            .get_or_insert_with(|| {
                Rc::new(
                    self.files
                        .iter()
                        .flat_map(|file| &file.symbols)
                        .filter(|symbol| crate::declarations::container(symbol))
                        .map(|symbol| symbol.name.as_str())
                        .collect(),
                )
            })
            .clone();
        names.contains(name)
    }

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
        if let Some(target) = self.script_import(file, call, &source, at) {
            return Some(target);
        }
        if language == "typescript" {
            if let Some(target) = self.typed_field_call(file, call, &source, at) {
                return Some(target);
            }
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

    fn script_import(
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
        let target_file = self.script_module(&file.file_path, import.source.as_deref()?)?;
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
            ("abstract.ts", "abstract class A {\n abstract target(): void;\n run() { this.target(); }\n bad() { target(); }\n}\nclass B extends A { target() {} }\n", 3),
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

    #[test]
    fn typed_field_calls_resolve_through_jsonc_tsconfig_paths() {
        let (root, files) = project(&[
            (
                "tsconfig.base.json",
                "{\n  // shared\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@app/*\": [\"src/*\"], },\n  },\n}\n",
            ),
            ("tsconfig.json", "{ /* app */ \"extends\": \"./tsconfig.base\", }\n"),
            ("src/user.service.ts", "export class UserService {\n  find() {}\n}\n"),
            ("src/api/user.controller.ts", "import { UserService } from '@app/user.service.js';\nexport class UserController {\n  constructor(private readonly users: UserService) {}\n  list() { return this.users.find(); }\n  static build() { return this.users.find(); }\n}\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let controller = "src/api/user.controller.ts";
        assert_eq!(
            resolver
                .script_module(controller, "@app/user.service")
                .map(|file| file.file_path.as_str()),
            Some("src/user.service.ts")
        );
        assert!(resolver.script_module(controller, "lodash").is_none());
        let calls = resolver.calls(&files[3]);
        let resolved: Vec<_> = calls
            .iter()
            .filter(|call| call.name == "find")
            .map(|call| {
                let target = resolver.resolve_call(&files[3], call);
                (
                    call.range.start_line,
                    target.map(|target| {
                        (
                            target.file.file_path.as_str(),
                            target.symbol.range.start_line,
                        )
                    }),
                )
            })
            .collect();
        // A static member's `this` is the class itself, not an instance holding the field.
        assert_eq!(
            resolved,
            [(4, Some(("src/user.service.ts", 2))), (5, None)],
            "{calls:?}"
        );
    }

    #[test]
    fn unresolved_calls_are_ruled_out_only_by_source_evidence() {
        let (root, files) = project(&[
            ("pkg/helpers.py", "def copy(value):\n    return value\n"),
            ("pkg/stdlib.py", "from copy import copy\n\ndef run(value):\n    return copy(value)\n"),
            ("pkg/relative.py", "from .helpers import copy\n\ndef run(value):\n    return copy(value)\n"),
            ("cache.ts", "export class Cache {\n  get(key: string) { return key; }\n}\n"),
            ("use.ts", "export function run(items: Map<string, number>, cache: Cache, loose: any) {\n  items.get('a');\n  cache.get('b');\n  loose.get('c');\n}\n"),
            ("widget.py", "def Widget():\n    return 1\n"),
            ("screen.py", "class Widget:\n    pass\n\ndef render():\n    return Widget()\n"),
            ("plain.py", "def render():\n    return Widget()\n"),
            ("outer.py", "def outer():\n    def inner():\n        return 1\n    return inner()\n"),
            ("caller.py", "def run():\n    return inner()\n"),
        ]);
        let resolver = SourceResolver::new(&files, root.path());
        let file = |path: &str| files.iter().find(|file| file.file_path == path).unwrap();
        let rules_out = |caller: &str, line: usize, target_file: &str, target: &str| {
            let caller = file(caller);
            let calls = resolver.calls(caller);
            let call = calls
                .iter()
                .find(|call| call.range.start_line == line && call.name == target)
                .unwrap_or_else(|| panic!("no call at {}:{line}: {calls:?}", caller.file_path));
            let symbol = file(target_file)
                .symbols
                .iter()
                .find(|symbol| symbol.name == target)
                .unwrap();
            resolver.rules_out_local_call(caller, call, target_file, symbol)
        };
        // Imported from an unrelated module, unlike a relative import.
        assert!(rules_out("pkg/stdlib.py", 4, "pkg/helpers.py", "copy"));
        assert!(!rules_out("pkg/relative.py", 4, "pkg/helpers.py", "copy"));
        // A receiver typed as neither the owner nor any indexed type; untyped stays open.
        assert!(rules_out("use.ts", 2, "cache.ts", "get"));
        assert!(!rules_out("use.ts", 3, "cache.ts", "get"));
        assert!(!rules_out("use.ts", 4, "cache.ts", "get"));
        // The calling file declares a type of the same name.
        assert!(rules_out("screen.py", 5, "widget.py", "Widget"));
        assert!(!rules_out("plain.py", 2, "widget.py", "Widget"));
        // A nested function is reachable by name only inside its parent.
        assert!(rules_out("caller.py", 2, "outer.py", "inner"));
        assert!(!rules_out("outer.py", 4, "outer.py", "inner"));
    }
}
