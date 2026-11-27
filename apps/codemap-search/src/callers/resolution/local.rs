//! Conservative lexical lookup for languages without full module/type resolution. TypeScript
//! imports (relative or tsconfig paths) and declared class field types are source evidence;
//! receiver hints and repository-wide spelling matches never are.

use super::*;

mod cpp_members;
mod dispatched_calls;
mod import_reach;
mod module_functions;
mod reexports;
mod script_modules;
mod typed_fields;

pub(crate) use import_reach::{may_reach_unnamed, ImportReach};
pub(super) use module_functions::ModuleLookups;
use module_functions::{is_top_level, only_binding};
pub(super) use reexports::ExportLookup;
pub(super) use script_modules::{ModulePaths, WorkspacePackages};

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

/// Whether `declarator`, a declarator of a node around `call`, declares `name` for the call.
/// A declarator that holds the call is the variable the call initializes, as in
/// `int total = sum();`, and declares nothing the call can name.
fn declarator_declares(declarator: Node, call: Node, source: &Source, name: &str) -> bool {
    !contains(declarator, call) && declares(declarator, false, source, name)
}

/// Whether the declarator `node` declares `name`: as one of the parameters of a function
/// declarator, or as the name of any other declarator. The name of a function is the function
/// itself, which a recursive call names, and an initial value declares nothing.
fn declares(node: Node, is_function: bool, source: &Source, name: &str) -> bool {
    if let Some(parameters) = node.child_by_field_name("parameters") {
        return has_identifier(parameters, source, name)
            || node
                .child_by_field_name("declarator")
                .is_some_and(|inner| declares(inner, true, source, name));
    }
    if let Some(inner) = node
        .child_by_field_name("declarator")
        .or_else(|| node.child_by_field_name("name"))
    {
        return declares(inner, is_function, source, name);
    }
    if node.named_child_count() == 0 {
        return !is_function && has_identifier(node, source, name);
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| declares(child, is_function, source, name));
    found
}

fn has_parameter(mut at: Node, source: &Source, name: &str) -> bool {
    let call = at;
    loop {
        if ["parameters", "parameter", "type_parameters", "signature"]
            .iter()
            .any(|field| {
                at.child_by_field_name(field)
                    .is_some_and(|node| has_identifier(node, source, name))
            })
            || at
                .child_by_field_name("declarator")
                .is_some_and(|node| declarator_declares(node, call, source, name))
        {
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

/// Path keywords that stand for a type or module the path does not spell out.
const RELATIVE_SCOPES: &[&str] = &["Self", "self", "super", "crate", "static", "parent"];

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

/// The type a parameter writes when its grammar gives that type no field, as Kotlin does.
fn unlabelled_type(parameter: Node) -> Option<Node> {
    let mut cursor = parameter.walk();
    let found = parameter
        .named_children(&mut cursor)
        .find(|child| child.kind().ends_with("_type"));
    found
}

/// Whether the function declared at `node` extends a type it is not a member of, as Kotlin
/// `fun Type.name()`, C# `Name(this Type value)` and the members of a Dart or Scala
/// `extension` do: member syntax on a value of that type or of any subtype reaches it.
fn extends_receiver(node: Node, source: &Source) -> bool {
    let follows_receiver = node
        .child_by_field_name("name")
        .and_then(|name| name.prev_sibling())
        .is_some_and(|previous| previous.kind() == ".");
    let mut block = node.parent();
    while let Some(parent) = block.filter(|parent| !parent.kind().starts_with("extension")) {
        block = parent.parent();
    }
    follows_receiver
        || block.is_some()
        || node
            .child_by_field_name("parameters")
            .and_then(|parameters| parameters.named_child(0))
            .is_some_and(|parameter| {
                let mut cursor = parameter.walk();
                let extends = parameter
                    .children(&mut cursor)
                    .any(|child| child.kind() == "modifier" && text(child, source) == Some("this"));
                extends
            })
}

/// The path before the last `::` of the callee at `at`, e.g. `std::cmp` in
/// `std::cmp::max(a, b)`.
fn scope_path<'s>(at: Node, source: &'s Source) -> Option<&'s str> {
    // PHP keeps the scope in the call itself; Rust and C++ keep it in the callee.
    if at.kind() == "scoped_call_expression" {
        return at
            .child_by_field_name("scope")
            .and_then(|scope| text(scope, source));
    }
    let mut callee = at.child_by_field_name("function")?;
    while let Some(inner) = callee.child_by_field_name("function") {
        callee = inner;
    }
    if !matches!(callee.kind(), "scoped_identifier" | "qualified_identifier") {
        return None;
    }
    text(callee, source)?
        .rsplit_once("::")
        .map(|(path, _)| path)
}

fn names_type(node: Node, source: &Source, name: &str) -> bool {
    if matches!(node.kind(), "type_identifier" | "identifier") && text(node, source) == Some(name) {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| names_type(child, source, name));
    found
}

/// Whether a declaration around `at` introduces `name` as a type parameter or names it in a
/// bound of one.
fn has_type_parameter(mut at: Node, source: &Source, name: &str) -> bool {
    loop {
        let declares = ["type_parameters", "parameters"].iter().any(|field| {
            at.child_by_field_name(field).is_some_and(|list| {
                matches!(list.kind(), "type_parameters" | "template_parameter_list")
                    && names_type(list, source, name)
            })
        });
        if declares {
            return true;
        }
        let Some(parent) = at.parent() else {
            return false;
        };
        at = parent;
    }
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
                    .or_else(|| unlabelled_type(parameter))
                    .and_then(|node| text(node, source))
                    .map(|value| value.trim().trim_start_matches(':').trim().to_string())
                    .filter(|value| !value.is_empty());
            }
        }
        current = node.parent();
    }
    None
}

/// Extracts the constructor/factory callee path (e.g. `Flask`, `flask.Flask`) when `binding_node`
/// is initialized directly by a call or `new` expression (`app = Flask(...)`,
/// `const app = createApp()`, `var app = new Client()`).
fn initializer_callee_path(binding_node: Node, source: &Source) -> Option<String> {
    let mut cur = binding_node;
    let mut init_expr = None;
    for _ in 0..4 {
        if let Some(rhs) = cur
            .child_by_field_name("right")
            .or_else(|| cur.child_by_field_name("value"))
            .or_else(|| cur.child_by_field_name("initializer"))
        {
            if !contains(rhs, binding_node) {
                init_expr = Some(rhs);
                break;
            }
        }
        if matches!(
            cur.kind(),
            "assignment"
                | "augmented_assignment"
                | "variable_declarator"
                | "init_declarator"
                | "short_var_declaration"
                | "property_declaration"
                | "let_declaration"
        ) {
            // Some grammars (e.g. Kotlin `property_declaration`, Go `short_var_declaration`) hold
            // the initializer as the last named child rather than a `value`/`right` field.
            let mut cursor = cur.walk();
            for child in cur.named_children(&mut cursor) {
                if !contains(child, binding_node)
                    && matches!(
                        child.kind(),
                        "call"
                            | "call_expression"
                            | "new_expression"
                            | "object_creation_expression"
                            | "invocation_expression"
                            | "expression_list"
                    )
                {
                    init_expr = Some(if child.kind() == "expression_list" {
                        child.named_child(0)?
                    } else {
                        child
                    });
                    break;
                }
            }
            if init_expr.is_some() {
                break;
            }
        }
        cur = cur.parent()?;
    }
    let expr = init_expr?;
    let callee = match expr.kind() {
        "call" | "call_expression" | "invocation_expression" => expr
            .child_by_field_name("function")
            .or_else(|| expr.named_child(0))?,
        "new_expression" => expr.child_by_field_name("constructor")?,
        "object_creation_expression" => expr.child_by_field_name("type")?,
        _ => return None,
    };
    let raw = text(callee, source)?.trim();
    let base = raw.split(['<', '(']).next()?.trim();
    if base.is_empty()
        || !base
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '$'))
    {
        return None;
    }
    Some(base.to_string())
}

/// Counts arguments at call site `at` when the argument list parses cleanly and contains no
/// splat/spread/unpack arguments (`*args`, `**kwargs`, `...spread`).
fn fixed_call_argument_count(source: &Source, at: Node) -> Option<usize> {
    let args = at.child_by_field_name("arguments").or_else(|| {
        let mut cursor = at.walk();
        let found = at.named_children(&mut cursor).find(|c| {
            matches!(
                c.kind(),
                "argument_list" | "arguments" | "value_arguments" | "call_suffix"
            )
        });
        found
    })?;
    let args = if args.kind() == "call_suffix" {
        let mut cursor = args.walk();
        let found = args
            .named_children(&mut cursor)
            .find(|c| c.kind() == "value_arguments")?;
        found
    } else {
        args
    };
    if args.has_error() {
        return None;
    }
    let mut count = 0usize;
    let mut cursor = args.walk();
    for child in args.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "comment" | "line_comment" | "block_comment" | "documentation_comment"
        ) {
            continue;
        }
        if matches!(
            child.kind(),
            "list_splat"
                | "dictionary_splat"
                | "spread_element"
                | "splat_argument"
                | "hash_splat_argument"
                | "variadic_argument"
                | "argument_unpacking"
        ) {
            return None;
        }
        if let Some(raw) = text(child, source) {
            let t = raw.trim();
            if t.starts_with('*') || t.starts_with("...") || t.ends_with("...") {
                return None;
            }
        }
        count += 1;
    }
    Some(count)
}

/// Returns `(min_args, max_args)` for `target_node` when its parameter list is non-variadic and
/// unambiguous. Returns `None` on variadic parameters, syntax errors, or unrecognized nodes.
fn fixed_target_param_bounds(
    source: &Source,
    target_node: Node,
    language: &str,
    target: &ExtractedSymbol,
    has_receiver: bool,
) -> Option<(usize, usize)> {
    let mut fn_node = target_node;
    if fn_node.kind() == "decorated_definition" {
        fn_node = fn_node.child_by_field_name("definition")?;
    }
    // C/C++ declarators hold parameters inside `function_declarator`.
    let params = fn_node
        .child_by_field_name("parameters")
        .or_else(|| {
            fn_node
                .child_by_field_name("declarator")
                .and_then(|d| d.child_by_field_name("parameters"))
        })
        .or_else(|| {
            let mut cursor = fn_node.walk();
            let found = fn_node.named_children(&mut cursor).find(|c| {
                matches!(
                    c.kind(),
                    "parameters"
                        | "formal_parameters"
                        | "parameter_list"
                        | "function_value_parameters"
                )
            });
            found
        })?;
    if params.has_error() {
        return None;
    }
    let is_py_staticmethod = language == "python"
        && target_node.kind() == "decorated_definition"
        && text(target_node, source).is_some_and(|raw| raw.contains("@staticmethod"));
    let mut min_args = 0usize;
    let mut max_args = 0usize;
    let mut first = true;
    let mut cursor = params.walk();
    for param in params.named_children(&mut cursor) {
        if matches!(
            param.kind(),
            "comment"
                | "line_comment"
                | "block_comment"
                | "documentation_comment"
                | "positional_separator"
                | "keyword_separator"
        ) {
            continue;
        }
        let raw = text(param, source).unwrap_or("").trim();
        if matches!(
            param.kind(),
            "list_splat_pattern"
                | "dictionary_splat_pattern"
                | "rest_pattern"
                | "spread_parameter"
                | "variadic_parameter"
                | "variadic_parameter_declaration"
                | "splat_parameter"
                | "hash_splat_parameter"
        ) || raw.starts_with('*')
            || raw.contains("...")
            || raw.starts_with("params ")
            || raw.starts_with("vararg ")
            || raw.contains(" vararg ")
        {
            return None;
        }
        if first {
            first = false;
            if param.kind() == "self_parameter" {
                continue;
            }
            if language == "python"
                && target.owner.is_some()
                && !is_py_staticmethod
                && (raw == "self"
                    || raw.starts_with("self:")
                    || raw.starts_with("self ")
                    || raw == "cls"
                    || raw.starts_with("cls:")
                    || raw.starts_with("cls "))
            {
                continue;
            }
            if language == "csharp" && has_receiver && raw.starts_with("this ") {
                continue;
            }
            if matches!(language, "typescript" | "javascript")
                && (raw == "this" || raw.starts_with("this:"))
            {
                continue;
            }
        }
        // Go `func f(a, b int)` groups multiple identifiers inside one `parameter_declaration`.
        if language == "go" && param.kind() == "parameter_declaration" {
            let mut id_count = 0usize;
            let mut p_cursor = param.walk();
            for c in param.named_children(&mut p_cursor) {
                if c.kind() == "identifier" {
                    id_count += 1;
                }
            }
            let n = id_count.max(1);
            min_args += n;
            max_args += n;
            continue;
        }
        let is_optional = matches!(
            param.kind(),
            "default_parameter" | "typed_default_parameter" | "optional_parameter"
        ) || param.child_by_field_name("default_value").is_some()
            || raw.contains('=')
            || (language == "typescript" && raw.contains("?:"));
        max_args += 1;
        if !is_optional {
            min_args += 1;
        }
    }
    Some((min_args, max_args))
}

impl<'a> SourceResolver<'a> {
    /// P1: Verifies that `name` on 1-indexed `line` of `file_path` is a genuine code identifier
    /// reference (not a comment/string/import/declaration) and is scope-compatible with
    /// `target_file` (same file, explicit import from a compatible module, or unique definition
    /// not imported from an unrelated module).
    pub(crate) fn is_verified_non_call_reference(
        &self,
        file_path: &str,
        line: usize,
        name: &str,
        target_file: &str,
        own_def_count: usize,
    ) -> bool {
        let Some(source) = self.source(file_path) else {
            return false;
        };
        let row = line.saturating_sub(1);
        let root = source.syntax.tree.root_node();
        let mut stack = vec![root];
        let mut matched_node = None;
        while let Some(node) = stack.pop() {
            if node.end_position().row < row || node.start_position().row > row {
                continue;
            }
            if node.start_position().row == row
                && matches!(
                    node.kind(),
                    "identifier" | "simple_identifier" | "property_identifier" | "field_identifier"
                )
                && text(node, &source) == Some(name)
            {
                let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
                let in_import_or_use =
                    std::iter::successors(node.parent(), |n| n.parent()).any(|anc| {
                        matches!(
                            anc.kind(),
                            "import_from_statement"
                                | "import_statement"
                                | "import_declaration"
                                | "use_declaration"
                                | "import"
                        )
                    });
                let in_cpp_fn_declarator = parent_kind == "function_declarator"
                    || (parent_kind == "qualified_identifier"
                        && node
                            .parent()
                            .and_then(|p| p.parent())
                            .is_some_and(|gp| gp.kind() == "function_declarator"));
                if !in_import_or_use
                    && !in_cpp_fn_declarator
                    && !matches!(
                        parent_kind,
                        "aliased_import"
                            | "import_specifier"
                            | "function_definition"
                            | "function_declaration"
                            | "method_definition"
                            | "method_declaration"
                            | "function_item"
                    )
                {
                    matched_node = Some(node);
                    break;
                }
            }
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                stack.push(c);
            }
        }
        let Some(at) = matched_node else {
            return false;
        };
        if has_parameter(at, &source, name) {
            return false;
        }
        if file_path == target_file {
            return true;
        }
        let Some(navigation) = source.extracted.navigation.as_ref() else {
            return own_def_count == 1;
        };
        let matching_imports: Vec<_> = navigation
            .imports
            .iter()
            .filter(|imp| imp.local_name == name || imp.imported_name.as_deref() == Some(name))
            .collect();
        if !matching_imports.is_empty() {
            return matching_imports.iter().any(|imp| {
                imp.source
                    .as_deref()
                    .is_some_and(|m| module_may_hold(m, target_file))
            });
        }
        // Qualified reference like `mod.handler`: check module import if present.
        if let Some(parent) = at.parent().filter(|p| {
            matches!(
                p.kind(),
                "attribute"
                    | "member_expression"
                    | "field_expression"
                    | "scoped_identifier"
                    | "qualified_identifier"
            )
        }) {
            if let Some(obj) = parent.named_child(0).and_then(|o| text(o, &source)) {
                let mod_imports: Vec<_> = navigation
                    .imports
                    .iter()
                    .filter(|imp| imp.local_name == obj)
                    .collect();
                if !mod_imports.is_empty() {
                    return mod_imports.iter().any(|imp| {
                        imp.source
                            .as_deref()
                            .or(imp.imported_name.as_deref())
                            .or(Some(imp.local_name.as_str()))
                            .is_some_and(|m| module_may_hold(m, target_file))
                    });
                }
            }
        }
        own_def_count == 1
    }

    /// P1: Verifies whether a Python class call `Cls(...)` on `line` of `file_path` is scope-
    /// compatible with the Python class `name` declared in `target_file`.
    pub(crate) fn is_python_class_call_compatible(
        &self,
        file_path: &str,
        line: usize,
        name: &str,
        target_file: &str,
        own_def_count: usize,
    ) -> bool {
        if !file_path.ends_with(".py") {
            return false;
        }
        let Some(source) = self.source(file_path) else {
            return false;
        };
        if file_path == target_file {
            return true;
        }
        let Some(navigation) = source.extracted.navigation.as_ref() else {
            return own_def_count == 1;
        };
        let matching_imports: Vec<_> = navigation
            .imports
            .iter()
            .filter(|imp| imp.local_name == name || imp.imported_name.as_deref() == Some(name))
            .collect();
        if !matching_imports.is_empty() {
            return matching_imports.iter().any(|imp| {
                imp.source.as_deref().is_some_and(|m| {
                    self.python_module(file_path, m)
                        .map(|f| f.file_path == target_file)
                        .unwrap_or_else(|| module_may_hold(m, target_file))
                })
            });
        }
        // Check `pkg.Cls(...)` qualified call on `line`.
        let row = line.saturating_sub(1);
        let root = source.syntax.tree.root_node();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if node.end_position().row < row || node.start_position().row > row {
                continue;
            }
            if node.kind() == "call" && node.start_position().row == row {
                if let Some(func) = node.child_by_field_name("function") {
                    if func.kind() == "attribute"
                        && func
                            .child_by_field_name("attribute")
                            .and_then(|a| text(a, &source))
                            == Some(name)
                    {
                        if let Some(obj) = func
                            .child_by_field_name("object")
                            .and_then(|o| text(o, &source))
                        {
                            let head = obj.split('.').next().unwrap_or(obj);
                            let mod_imports: Vec<_> = navigation
                                .imports
                                .iter()
                                .filter(|imp| imp.local_name == head)
                                .collect();
                            if !mod_imports.is_empty() {
                                return mod_imports.iter().any(|imp| {
                                    let module =
                                        imp.source.as_deref().unwrap_or(imp.local_name.as_str());
                                    self.python_module(file_path, module)
                                        .map(|f| f.file_path == target_file)
                                        .unwrap_or_else(|| module_may_hold(module, target_file))
                                });
                            }
                        }
                    }
                }
            }
            let mut cursor = node.walk();
            for c in node.named_children(&mut cursor) {
                stack.push(c);
            }
        }
        false
    }
}

impl<'a> SourceResolver<'a> {
    /// Source evidence that an unresolved call reaches something other than `target`: the
    /// calling file declares a type under the callee's name, the name is imported from an
    /// unrelated module, the receiver is declared with a type that is neither the target's
    /// owner nor any indexed type, or the call path names a type this repository does not
    /// declare.
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
        if let Some(declaring) = self.file_by_path(target_file) {
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
            || self.receiver_type_rules_out(&source, at, call, target_file, target)
            || self.external_initializer_rules_out(&source, at, call, target_file, target)
            || self.arity_rules_out(&source, at, call, target_file, target)
            || self.scope_rules_out(&source, at, target_file, target)
    }

    /// C2: A receiver initialized directly from an external imported constructor/factory
    /// (`from flask import Flask; app = Flask(__name__)`) cannot hold an instance of a workspace
    /// type (`Router`), moving `@app.route(...)` out of `unconfirmed callers`.
    fn external_initializer_rules_out(
        &self,
        source: &Source,
        at: Node,
        call: &CallSite,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        let Some(receiver) = call
            .receiver
            .as_deref()
            .filter(|receiver| is_simple_identifier(receiver))
        else {
            return false;
        };
        let receiver = receiver.trim_start_matches('$');
        if self.is_extension(target_file, target) {
            return false;
        }
        let Some(navigation) = source.extracted.navigation.as_ref() else {
            return false;
        };
        let nearest = navigation
            .local_bindings
            .iter()
            .filter(|binding| binding.name.trim_start_matches('$') == receiver)
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
                            node,
                        )
                    })
            })
            .min_by_key(|(key, _)| *key);
        let Some((_, binding_node)) = nearest else {
            return false;
        };
        // A closer function parameter shadows an outer module-level binding.
        if has_parameter(at, source, receiver) {
            return false;
        }
        let Some(callee_path) = initializer_callee_path(binding_node, source) else {
            return false;
        };
        let head = callee_path
            .split(['.', ':'])
            .next()
            .unwrap_or(callee_path.as_str())
            .trim();
        let bare = callee_path
            .rsplit(['.', ':'])
            .next()
            .unwrap_or(callee_path.as_str())
            .trim();
        if head.is_empty() || bare.is_empty() {
            return false;
        }
        let owner = target
            .owner
            .as_deref()
            .map(|owner| owner.rsplit(['.', ':']).next().unwrap_or(owner));
        if Some(bare) == owner
            || self.is_indexed_type(bare)
            || source
                .extracted
                .symbols
                .iter()
                .any(|s| s.name == head || s.name == bare)
        {
            return false;
        }
        let mut modules = navigation
            .imports
            .iter()
            .filter(|import| {
                import.kind != ImportKind::Glob
                    && import.local_name.trim_start_matches('$') == head
                    && node_at(source, &import.range).is_some_and(|node| same_tree(node, at))
            })
            .map(|import| import.source.as_deref())
            .peekable();
        if modules.peek().is_none() {
            return false;
        }
        let lang = crate::lang::spec_for_path(Path::new(&source.extracted.file_path))
            .map(|s| s.language_name());
        modules.all(|module| {
            let Some(module) = module else {
                return false;
            };
            if module_may_hold(module, target_file) {
                return false;
            }
            match lang {
                Some("python") => self
                    .python_module(&source.extracted.file_path, module)
                    .is_none(),
                Some("typescript" | "javascript" | "vue" | "astro" | "svelte") => self
                    .script_module(&source.extracted.file_path, module)
                    .is_none(),
                _ => !module.starts_with('.'),
            }
        })
    }

    /// C2: Fixed-arity mismatch between a call site and a non-variadic target declaration (e.g.
    /// `config.get("port", 8080)` cannot target `Cache.get(self, key)`). Conservative: returns
    /// `false` whenever either side has variadic/splat syntax, missing AST, or JS dynamic arity.
    fn arity_rules_out(
        &self,
        source: &Source,
        at: Node,
        call: &CallSite,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        if !crate::declarations::callable(target) {
            return false;
        }
        let Some(caller_lang) = crate::lang::spec_for_path(Path::new(&source.extracted.file_path))
            .map(|s| s.language_name())
        else {
            return false;
        };
        let Some(target_lang) =
            crate::lang::spec_for_path(Path::new(target_file)).map(|s| s.language_name())
        else {
            return false;
        };
        // JavaScript allows calling any function with extra or omitted arguments at runtime.
        if matches!(caller_lang, "javascript" | "vue" | "astro" | "svelte")
            || matches!(target_lang, "javascript" | "vue" | "astro" | "svelte")
        {
            return false;
        }
        let Some(arg_count) = fixed_call_argument_count(source, at) else {
            return false;
        };
        let Some(target_source) = self.source(target_file) else {
            return false;
        };
        let Some(target_node) = node_at(&target_source, &target.range) else {
            return false;
        };
        let Some((min_args, max_args)) = fixed_target_param_bounds(
            &target_source,
            target_node,
            target_lang,
            target,
            call.receiver.is_some(),
        ) else {
            return false;
        };
        arg_count < min_args || arg_count > max_args
    }

    /// A receiver declared with a type this repository does not declare calls a member of
    /// that type: neither a member of the target's owner nor a function outside any type.
    fn receiver_type_rules_out(
        &self,
        source: &Source,
        at: Node,
        call: &CallSite,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        let owner = target
            .owner
            .as_deref()
            .map(|owner| owner.rsplit(['.', ':']).next().unwrap_or(owner));
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
                .any(|name| Some(*name) == owner || self.is_indexed_type(name))
            && !self.is_extension(target_file, target)
    }

    /// Whether `target` extends a receiver type, or its declaration cannot be read to tell.
    fn is_extension(&self, target_file: &str, target: &ExtractedSymbol) -> bool {
        let has_extensions =
            crate::lang::spec_for_path(Path::new(target_file)).is_some_and(|spec| {
                matches!(
                    spec.language_name(),
                    "kotlin" | "csharp" | "dart" | "scala" | "swift"
                )
            });
        if !has_extensions {
            return false;
        }
        self.source(target_file).is_none_or(|source| {
            node_at(&source, &target.range).is_none_or(|node| extends_receiver(node, &source))
        })
    }

    /// A `Type::name(...)` path names another declaration when this repository neither
    /// declares nor imports that type, e.g. `String::from` beside the only indexed `from`.
    fn scope_rules_out(
        &self,
        source: &Source,
        at: Node,
        target_file: &str,
        target: &ExtractedSymbol,
    ) -> bool {
        let Some(path) = scope_path(at, source) else {
            return false;
        };
        // An implementation block for a type without a plain name records no owner.
        let Some(owner) = target
            .owner
            .clone()
            .or_else(|| self.implemented_type(target_file, target))
        else {
            return false;
        };
        let owner = owner.rsplit(['.', ':']).next().unwrap_or(&owner);
        let segments: Vec<&str> = path
            .split("::")
            .flat_map(|part| part.split('\\'))
            .filter(|segment| !segment.is_empty())
            .collect();
        let (Some(head), Some(name)) = (segments.first().copied(), segments.last().copied()) else {
            return false;
        };
        let is_plain = segments.iter().all(|segment| {
            segment
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
        });
        is_plain
            && !RELATIVE_SCOPES.contains(&head)
            && name != owner
            && !has_type_parameter(at, source, head)
            && !has_binding(source, at, head, None)
            && self.declared_name(head).is_none()
            && self.declared_name(name).is_none()
            && !self.is_reached_through(target_file, target, owner, name)
    }

    /// The type that the implementation block around `target` is written for, when the block
    /// names it by a plain path, e.g. `i32` in `impl From<Code> for i32`.
    fn implemented_type(&self, target_file: &str, target: &ExtractedSymbol) -> Option<String> {
        if !target_file.ends_with(".rs") {
            return None;
        }
        let source = self.source(target_file)?;
        let mut node = node_at(&source, &target.range)?;
        while node.kind() != "impl_item" {
            node = node.parent()?;
        }
        let written = text(node.child_by_field_name("type")?, &source)?;
        written
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | ':'))
            .then(|| written.to_string())
    }

    /// Whether a path through a type called `name` can still reach `target`, whose owner is
    /// `owner`: a trait or an interface lends its members to other types, a type parameter
    /// stands for any type, and a path through the trait that the holding block implements
    /// reaches every implementation. Also true when the declaration cannot be read.
    fn is_reached_through(
        &self,
        target_file: &str,
        target: &ExtractedSymbol,
        owner: &str,
        name: &str,
    ) -> bool {
        if self.declared_name(owner) == Some(true) {
            return true;
        }
        let Some(source) = self.source(target_file) else {
            return true;
        };
        let Some(node) = node_at(&source, &target.range) else {
            return true;
        };
        if has_type_parameter(node, &source, owner) {
            return true;
        }
        let mut block = Some(node);
        while let Some(current) = block {
            if let Some(implemented) = current.child_by_field_name("trait") {
                let written = text(implemented, &source).unwrap_or_default();
                let plain = written.split('<').next().unwrap_or(written);
                return plain.rsplit("::").next().map(str::trim) == Some(name);
            }
            block = current.parent();
        }
        false
    }

    /// `None` when no indexed file declares a symbol called `name` or imports something
    /// under it, else whether a trait or an interface has that name.
    fn declared_name(&self, name: &str) -> Option<bool> {
        if let Some(lookup) = self.call_lookup {
            return lookup.declared_names(self.files).get(name).copied();
        }
        let names = self
            .declared_names
            .borrow_mut()
            .get_or_insert_with(|| {
                let mut names = HashMap::new();
                for file in self.files {
                    for symbol in &file.symbols {
                        *names.entry(symbol.name.as_str()).or_default() |=
                            matches!(symbol.kind.as_str(), "trait" | "interface");
                    }
                    for import in file.navigation.iter().flat_map(|file| &file.imports) {
                        names.entry(import.local_name.as_str()).or_default();
                    }
                }
                Rc::new(names)
            })
            .clone();
        names.get(name).copied()
    }

    fn is_indexed_type(&self, name: &str) -> bool {
        if let Some(lookup) = self.call_lookup {
            return lookup.type_names(self.files).contains(name);
        }
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
        let glob_shadows_local =
            !matches!(language, "kotlin" | "java" | "csharp" | "scala" | "swift");
        if !has_explicit_self
            && source
                .extracted
                .navigation
                .as_ref()?
                .imports
                .iter()
                .any(|import| {
                    (import.local_name == call.name
                        || (glob_shadows_local && import.kind == ImportKind::Glob))
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
        let module = self.script_module(&file.file_path, import.source.as_deref()?)?;
        let name = if call.receiver.is_some() {
            call.name.as_str()
        } else {
            import.imported_name.as_deref()?
        };
        // The module declares the function or re-exports it from the module that does.
        let mut targets = Vec::new();
        for origin in self.export_origins(&module.file_path, name).iter() {
            let Some(origin_source) = self.source(&origin.file.file_path) else {
                continue;
            };
            targets.extend(
                origin
                    .file
                    .symbols
                    .iter()
                    .filter(|symbol| {
                        symbol.name == origin.name
                            && symbol.kind == "fn"
                            && symbol.owner.is_none()
                            && symbol.flags.is_exported
                            && current_symbol(&origin_source, symbol)
                    })
                    .map(|symbol| Target {
                        file: origin.file,
                        symbol,
                        is_precise: true,
                    }),
            );
        }
        unique(targets)
    }
}

/// Targets of the calls named `name` on `line` of `caller`, as `path:line`, or `-`.
#[cfg(test)]
fn targets(entries: &[(&str, &str)], caller: &str, line: usize, name: &str) -> Vec<String> {
    // Module settings such as tsconfig files are read from the workspace on disk.
    let root = tempfile::tempdir().unwrap();
    for (path, source) in entries {
        let path = root.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    let sources: HashMap<String, String> = entries
        .iter()
        .map(|(path, source)| (path.to_string(), source.to_string()))
        .collect();
    let files = std::sync::Arc::new(
        entries
            .iter()
            .map(|(path, source)| {
                std::sync::Arc::new(
                    crate::parser::TreeSitterExtractor::new()
                        .extract(source, path)
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>(),
    );
    let index = crate::implementations::ImplementationIndex::build_at(
        std::sync::Arc::clone(&files),
        &sources,
        root.path(),
    );
    let resolver = SourceResolver::from_stored_sources(&files, root.path(), &sources)
        .with_implementations(&index);
    let file = files.iter().find(|file| file.file_path == caller).unwrap();
    resolver
        .calls(file)
        .iter()
        .filter(|call| call.range.start_line == line && call.name == name)
        .map(|call| {
            resolver
                .resolve_call(file, call)
                .map_or("-".into(), |target| {
                    format!(
                        "{}:{}",
                        target.file.file_path, target.symbol.range.start_line
                    )
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(
        sources: &[(&str, &str)],
    ) -> (tempfile::TempDir, Vec<std::sync::Arc<ExtractedFile>>) {
        let (root, path) = crate::callers::fixtures::write_repo(sources);
        let files = sources
            .iter()
            .map(|(name, source)| {
                std::sync::Arc::new(
                    crate::parser::TreeSitterExtractor::new()
                        .extract(source, name)
                        .unwrap(),
                )
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
