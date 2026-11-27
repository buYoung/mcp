//! C++ member calls reached through source evidence: an unqualified or `this->` call inside a
//! member function, a call through a parameter, local variable or field declared with the class
//! type (directly, behind a pointer or reference, or behind a smart pointer such as
//! `std::unique_ptr<T>`), and `Class::method()`. The class must be declared with a body in the
//! calling file or a header it reaches. The method must have one definition, or one declaration
//! when no definition is indexed; overloads and inherited members stay unresolved.

use super::module_functions::is_shadowed;
use super::*;

/// A class declared with a body, as a target of member lookups.
#[derive(Clone, Copy)]
pub(in crate::callers::resolution) struct CppClass<'a> {
    pub file: &'a ExtractedFile,
    pub symbol: &'a ExtractedSymbol,
}

/// A class or inline struct body whose fields can be resolved during member-access traversal.
#[derive(Clone)]
enum CppMemberScope<'a> {
    Class(CppClass<'a>),
    InlineStruct {
        file: &'a ExtractedFile,
        range: crate::parser::CodeRange,
    },
}

const MAX_FIELD_CHAIN_DEPTH: usize = 4;

impl<'a> SourceResolver<'a> {
    pub(in crate::callers::resolution) fn cpp_member_call(
        &self,
        file: &'a ExtractedFile,
        call: &CallSite,
    ) -> Option<Target<'a>> {
        if crate::lang::spec_for_path(Path::new(&file.file_path))?.language_name() != "cpp" {
            return None;
        }
        let source = self.source(&file.file_path)?;
        let at = node_at(&source, &call.range)?;
        if at.kind() != "call_expression" {
            return None;
        }
        let function = at.child_by_field_name("function")?;
        let class = match function.kind() {
            "identifier" => {
                if text(function, &source)? != call.name
                    || is_shadowed(&source, at, &call.name, "cpp")
                {
                    return None;
                }
                self.enclosing_class(file, &source, at)?
            }
            "field_expression" => {
                let field = function.child_by_field_name("field")?;
                if field.kind() != "field_identifier" || text(field, &source)? != call.name {
                    return None;
                }
                let is_arrow = function
                    .child_by_field_name("operator")
                    .is_some_and(|operator| operator.kind() == "->");
                let argument = function.child_by_field_name("argument")?;
                match self.receiver_scope(file, &source, at, argument, is_arrow, 0)? {
                    CppMemberScope::Class(class) => class,
                    CppMemberScope::InlineStruct { .. } => return None,
                }
            }
            "qualified_identifier" => {
                // `Class::method()` or `ns::Class::method()`; a namespace scope is left to
                // free-function resolution.
                let mut qualified = function;
                let name = loop {
                    let name = qualified.child_by_field_name("name")?;
                    if name.kind() != "qualified_identifier" {
                        break name;
                    }
                    qualified = name;
                };
                if name.kind() != "identifier" || text(name, &source)? != call.name {
                    return None;
                }
                let scope = qualified.child_by_field_name("scope")?;
                self.visible_class(file, scope_name(scope, &source)?)?
            }
            _ => return None,
        };
        self.cpp_method(class, &call.name)
    }

    /// The member function `name` of `class`: its one definition, in the class body or out of
    /// line in a file that reaches the class, else its one declaration when none is indexed.
    pub(in crate::callers::resolution) fn cpp_method(
        &self,
        class: CppClass<'a>,
        name: &str,
    ) -> Option<Target<'a>> {
        let lookups = self.module_lookups();
        let members = lookups.cpp_methods(self.files, class.symbol.name.as_str(), name);
        if members.is_empty() {
            return None;
        }
        let mut definitions = Vec::new();
        let mut declarations = Vec::new();
        for &(file, symbol) in &members {
            let Some(source) = self.source(&file.file_path) else {
                continue;
            };
            let Some(node) = node_at(&source, &symbol.range) else {
                continue;
            };
            if !current_symbol(&source, symbol) {
                continue;
            }
            let is_in_body = std::ptr::eq(file, class.file)
                && class.symbol.range.start_line <= symbol.range.start_line
                && symbol.range.end_line_inclusive() <= class.symbol.range.end_line_inclusive();
            let is_out_of_line = !is_in_body
                && node.kind() == "function_definition"
                && crate::lang::c_family::cpp::cpp_outofline_owner(node, source.text.as_bytes())
                    .as_deref()
                    == Some(class.symbol.name.as_str())
                && (std::ptr::eq(file, class.file)
                    || self
                        .reachable_headers(&file.file_path)
                        .iter()
                        .any(|header| std::ptr::eq(*header, class.file)));
            if !is_in_body && !is_out_of_line {
                continue;
            }
            let target = Target {
                file,
                symbol,
                is_precise: true,
            };
            match node.kind() {
                "function_definition" => definitions.push(target),
                "field_declaration" | "declaration" => declarations.push(target),
                _ => {}
            }
        }
        if definitions.is_empty() {
            unique(declarations)
        } else {
            unique(definitions)
        }
    }

    /// The class whose member function contains `at`: the class body around an in-class
    /// definition, or the class that qualifies an out-of-line definition's name.
    fn enclosing_class(
        &self,
        file: &'a ExtractedFile,
        source: &Source,
        at: Node,
    ) -> Option<CppClass<'a>> {
        let mut node = at;
        while let Some(parent) = node.parent() {
            node = parent;
            match node.kind() {
                "class_specifier" | "struct_specifier" => {
                    let name = text(node.child_by_field_name("name")?, source)?;
                    return self.visible_class(file, name);
                }
                "function_definition" => {
                    if let Some(owner) = crate::lang::c_family::cpp::cpp_outofline_owner(
                        node,
                        source.text.as_bytes(),
                    ) {
                        return self.visible_class(file, &owner);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The class or inline struct body of the object a call or member access is made on: `this`,
    /// a parameter, local variable or field, or a chained field expression (`a.b->m()`,
    /// `this->data.owner->m()`).
    fn receiver_scope(
        &self,
        file: &'a ExtractedFile,
        source: &Source,
        at: Node,
        argument: Node,
        is_arrow: bool,
        depth: usize,
    ) -> Option<CppMemberScope<'a>> {
        if depth >= MAX_FIELD_CHAIN_DEPTH {
            return None;
        }
        match argument.kind() {
            "this" if is_arrow => self
                .enclosing_class(file, source, at)
                .map(CppMemberScope::Class),
            "identifier" => {
                let name = text(argument, source)?;
                if let Some((declaration, declarator)) = local_declaration(source, at, name) {
                    return self.scope_from_declaration(
                        file,
                        file,
                        source,
                        None,
                        declaration,
                        declarator,
                        is_arrow,
                    );
                }
                let owner = self.enclosing_class(file, source, at)?;
                self.field_scope(file, &CppMemberScope::Class(owner), name, is_arrow)
            }
            "field_expression" => {
                let object = argument.child_by_field_name("argument")?;
                let object_is_arrow = argument
                    .child_by_field_name("operator")
                    .is_some_and(|operator| operator.kind() == "->");
                let field = argument.child_by_field_name("field")?;
                if field.kind() != "field_identifier" {
                    return None;
                }
                let parent =
                    self.receiver_scope(file, source, at, object, object_is_arrow, depth + 1)?;
                self.field_scope(file, &parent, text(field, source)?, is_arrow)
            }
            "parenthesized_expression" => {
                let inner = argument.named_child(0)?;
                self.receiver_scope(file, source, at, inner, is_arrow, depth + 1)
            }
            _ => None,
        }
    }

    /// The class or inline struct body named by field `name` inside `owner`.
    fn field_scope(
        &self,
        calling_file: &'a ExtractedFile,
        owner: &CppMemberScope<'a>,
        name: &str,
        is_arrow: bool,
    ) -> Option<CppMemberScope<'a>> {
        let (owner_file, owner_range) = match owner {
            CppMemberScope::Class(class) => (class.file, &class.symbol.range),
            CppMemberScope::InlineStruct { file, range } => (*file, range),
        };
        let owner_source = self.source(&owner_file.file_path)?;
        let container_node = node_at(&owner_source, owner_range)?;
        let body = container_node.child_by_field_name("body")?;
        let mut cursor = body.walk();
        let mut found = None;
        for member in body.named_children(&mut cursor) {
            if member.kind() != "field_declaration" {
                continue;
            }
            let mut declarators = member.walk();
            for declarator in member.children_by_field_name("declarator", &mut declarators) {
                if declared_name(declarator, &owner_source) == Some(name) {
                    if found.is_some() {
                        return None;
                    }
                    found = Some((member, declarator));
                }
            }
        }
        let (member, declarator) = found?;
        self.scope_from_declaration(
            calling_file,
            owner_file,
            &owner_source,
            Some(body),
            member,
            declarator,
            is_arrow,
        )
    }

    /// Resolves a variable/field declaration to either a visible class (checking both the
    /// calling translation unit's reachable headers — for forward-declared pointer fields —
    /// and the declaring file's headers) or an inline/nested struct body.
    #[allow(clippy::too_many_arguments)]
    fn scope_from_declaration(
        &self,
        calling_file: &'a ExtractedFile,
        decl_file: &'a ExtractedFile,
        decl_source: &Source,
        enclosing_body: Option<Node>,
        declaration: Node,
        declarator: Node,
        is_arrow: bool,
    ) -> Option<CppMemberScope<'a>> {
        if let Some(typ) = declaration.child_by_field_name("type") {
            let pointers = declarator_pointer_depth(declarator);
            if matches!(typ.kind(), "struct_specifier" | "class_specifier")
                && typ.child_by_field_name("body").is_some()
                && ((is_arrow && pointers == 1) || (!is_arrow && pointers == 0))
            {
                return Some(CppMemberScope::InlineStruct {
                    file: decl_file,
                    range: node_code_range(typ),
                });
            }
        }
        let class_name = declared_class(decl_source, declaration, declarator, is_arrow)?;
        // Check a nested `struct Name { ... };` inside the same class body first so
        // class-local helper structs (like `struct Data`) are never confused with siblings.
        if let Some(body) = enclosing_body {
            if let Some(nested) = find_nested_struct(body, decl_source, &class_name) {
                return Some(CppMemberScope::InlineStruct {
                    file: decl_file,
                    range: node_code_range(nested),
                });
            }
        }
        // In C++, headers routinely forward-declare pointer fields (`class Foo; Foo *f;`)
        // while the `.cpp` calling `f->m()` includes `foo.h`. Check the calling file's
        // reachable headers as well as the declaring header's.
        self.visible_class(calling_file, &class_name)
            .or_else(|| {
                (!std::ptr::eq(calling_file, decl_file))
                    .then(|| self.visible_class(decl_file, &class_name))
                    .flatten()
            })
            .map(CppMemberScope::Class)
    }

    /// The one class named `name` declared with a body in `file` or a header it reaches.
    pub(in crate::callers::resolution) fn visible_class(
        &self,
        file: &'a ExtractedFile,
        name: &str,
    ) -> Option<CppClass<'a>> {
        let lookups = self.module_lookups();
        let candidates = lookups.cpp_classes(self.files, name);
        if candidates.is_empty() {
            return None;
        }
        let reachable = self.reachable_headers(&file.file_path);
        let mut classes = Vec::new();
        for &(declaring, symbol) in &candidates {
            if !std::ptr::eq(declaring, file)
                && !reachable
                    .iter()
                    .any(|header| std::ptr::eq(*header, declaring))
            {
                continue;
            }
            let Some(declaring_source) = self.source(&declaring.file_path) else {
                continue;
            };
            let has_body = current_symbol(&declaring_source, symbol)
                && node_at(&declaring_source, &symbol.range)
                    .is_some_and(|node| node.child_by_field_name("body").is_some());
            if has_body {
                classes.push(CppClass {
                    file: declaring,
                    symbol,
                });
            }
        }
        match classes.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }
}

/// The local variable or parameter `name` in scope at `at`: the nearest earlier declaration in
/// an enclosing block, else a parameter of the enclosing function or lambda.
fn local_declaration<'t>(
    source: &'t Source,
    at: Node<'t>,
    name: &str,
) -> Option<(Node<'t>, Node<'t>)> {
    let mut node = at;
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "compound_statement" => {
                let mut found = None;
                let mut cursor = parent.walk();
                for child in parent.named_children(&mut cursor) {
                    if child.start_byte() >= node.start_byte() {
                        break;
                    }
                    if child.kind() != "declaration" {
                        continue;
                    }
                    let mut declarators = child.walk();
                    for declarator in child.children_by_field_name("declarator", &mut declarators) {
                        if declared_name(declarator, source) == Some(name) {
                            found = Some((child, declarator));
                        }
                    }
                }
                if found.is_some() {
                    return found;
                }
            }
            "function_definition" | "lambda_expression" => {
                let mut declarator = parent.child_by_field_name("declarator");
                while let Some(current) = declarator {
                    if let Some(parameters) = current.child_by_field_name("parameters") {
                        let mut cursor = parameters.walk();
                        for parameter in parameters.named_children(&mut cursor) {
                            if let Some(declarator) = parameter.child_by_field_name("declarator") {
                                if declared_name(declarator, source) == Some(name) {
                                    return Some((parameter, declarator));
                                }
                            }
                        }
                        break;
                    }
                    declarator = current.child_by_field_name("declarator").or_else(|| {
                        current
                            .named_child(0)
                            .filter(|_| current.kind() == "reference_declarator")
                    });
                }
                // A function body ends the search; a lambda may still see outer locals.
                if parent.kind() == "function_definition" {
                    return None;
                }
            }
            _ => {}
        }
        node = parent;
    }
    None
}

/// The name a declarator declares, through initializers, pointers, references and arrays.
fn declared_name<'t>(mut declarator: Node, source: &'t Source) -> Option<&'t str> {
    loop {
        match declarator.kind() {
            "identifier" | "field_identifier" => return text(declarator, source),
            "init_declarator" | "pointer_declarator" | "array_declarator" => {
                declarator = declarator.child_by_field_name("declarator")?;
            }
            "reference_declarator" => declarator = declarator.named_child(0)?,
            _ => return None,
        }
    }
}

/// The class a declaration's type names for a call made with `.` or `->`: `T` or `T&` with
/// `.`, and `T*` or a smart pointer `P<T>` with `->`. `auto` counts only when initialized with
/// `new T` or `std::make_unique<T>`/`std::make_shared<T>`.
fn declared_class(
    source: &Source,
    declaration: Node,
    declarator: Node,
    is_arrow: bool,
) -> Option<String> {
    let mut pointers = 0;
    let mut current = declarator;
    let mut value = None;
    loop {
        match current.kind() {
            "init_declarator" => {
                value = current.child_by_field_name("value");
                current = current.child_by_field_name("declarator")?;
            }
            "pointer_declarator" => {
                pointers += 1;
                current = current.child_by_field_name("declarator")?;
            }
            "reference_declarator" => current = current.named_child(0)?,
            "identifier" | "field_identifier" => break,
            _ => return None,
        }
    }
    let typ = declaration.child_by_field_name("type")?;
    let innermost = innermost_type(typ);
    if typ.kind() == "placeholder_type_specifier" || text(typ, source) == Some("auto") {
        if pointers > 0 || !is_arrow {
            return None;
        }
        let value = value?;
        return match value.kind() {
            "new_expression" => type_name(value.child_by_field_name("type")?, source),
            "call_expression" => {
                let function = value.child_by_field_name("function")?;
                let factory = match function.kind() {
                    "template_function" => function,
                    "qualified_identifier" => function
                        .child_by_field_name("name")
                        .filter(|name| name.kind() == "template_function")?,
                    _ => return None,
                };
                let factory_name = text(factory.child_by_field_name("name")?, source)?;
                if !matches!(factory_name, "make_unique" | "make_shared") {
                    return None;
                }
                first_template_argument(factory.child_by_field_name("arguments")?, source)
            }
            _ => None,
        };
    }
    match (is_arrow, pointers) {
        (false, 0) if innermost.kind() != "template_type" => type_name(typ, source),
        (true, 1) => type_name(typ, source),
        (true, 0) if innermost.kind() == "template_type" => {
            first_template_argument(innermost.child_by_field_name("arguments")?, source)
        }
        _ => None,
    }
}

/// The last name of a qualified type: `std::unique_ptr<T>` → `unique_ptr<T>`.
fn innermost_type(mut typ: Node) -> Node {
    while typ.kind() == "qualified_identifier" {
        match typ.child_by_field_name("name") {
            Some(name) => typ = name,
            None => break,
        }
    }
    typ
}

/// The class name a type specifier spells: `T`, `ns::T`, `T<U>` or `struct T`.
fn type_name(typ: Node, source: &Source) -> Option<String> {
    match typ.kind() {
        "type_identifier" => text(typ, source).map(str::to_string),
        "qualified_identifier" => type_name(typ.child_by_field_name("name")?, source),
        "template_type" => type_name(typ.child_by_field_name("name")?, source),
        "struct_specifier" | "class_specifier" => {
            type_name(typ.child_by_field_name("name")?, source)
        }
        _ => None,
    }
}

/// The class named by the first template argument, when it is a plain (non-pointer) type.
fn first_template_argument(arguments: Node, source: &Source) -> Option<String> {
    let argument = arguments.named_child(0)?;
    if argument.kind() != "type_descriptor" || argument.child_by_field_name("declarator").is_some()
    {
        return None;
    }
    type_name(argument.child_by_field_name("type")?, source)
}

/// The rightmost name of a qualifying scope: `ns::T` → `T`, `T<U>` → `T`.
fn scope_name<'t>(scope: Node, source: &'t Source) -> Option<&'t str> {
    match scope.kind() {
        "namespace_identifier" | "type_identifier" => text(scope, source),
        "template_type" => text(scope.child_by_field_name("name")?, source),
        _ => None,
    }
}

fn declarator_pointer_depth(mut declarator: Node) -> usize {
    let mut pointers = 0;
    loop {
        match declarator.kind() {
            "init_declarator" => match declarator.child_by_field_name("declarator") {
                Some(inner) => declarator = inner,
                None => break,
            },
            "pointer_declarator" => {
                pointers += 1;
                match declarator.child_by_field_name("declarator") {
                    Some(inner) => declarator = inner,
                    None => break,
                }
            }
            "reference_declarator" => match declarator.named_child(0) {
                Some(inner) => declarator = inner,
                None => break,
            },
            _ => break,
        }
    }
    pointers
}

fn node_code_range(node: Node) -> crate::parser::CodeRange {
    let start = node.start_position();
    let end = node.end_position();
    crate::parser::CodeRange {
        start_line: start.row + 1,
        start_col: start.column + 1,
        end_line: end.row + 1,
        end_col: end.column + 1,
    }
}

fn find_nested_struct<'t>(body: Node<'t>, source: &Source, name: &str) -> Option<Node<'t>> {
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        let candidate = match member.kind() {
            "struct_specifier" | "class_specifier" => Some(member),
            "field_declaration" => member
                .child_by_field_name("type")
                .filter(|t| matches!(t.kind(), "struct_specifier" | "class_specifier")),
            _ => None,
        };
        if let Some(spec) = candidate {
            if spec.child_by_field_name("body").is_some()
                && spec
                    .child_by_field_name("name")
                    .and_then(|n| text(n, source))
                    == Some(name)
            {
                return Some(spec);
            }
        }
    }
    None
}
