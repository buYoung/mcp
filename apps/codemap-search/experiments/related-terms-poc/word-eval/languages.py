"""Go 밖의 언어에서 정의된 이름을 tree-sitter로 모은다. collect_words.py가 쓴다.

언어마다 visit 함수가 노드 하나를 보고 정의를 add로 기록한 뒤, 자식에게 넘길 (owner, is_local, context)를 돌려준다.
kind 어휘는 Go와 같고 언어에 따라 class, trait, module, namespace, enum, enum_member, record, property,
constructor, macro, union이 더해진다. 정의 위치에서 쓰인 이름만 모으고 참조·호출은 모으지 않는다.
"""
import re

# 이름으로 세는 잎 노드(정의·사용 모두). 등장 횟수와 '쓰기만 한 이름'에 쓴다.
NAME_LEAVES = {
    "identifier", "type_identifier", "field_identifier", "property_identifier", "private_property_identifier",
    "shorthand_property_identifier", "shorthand_property_identifier_pattern", "shorthand_field_identifier",
    "namespace_identifier", "statement_identifier", "package_identifier", "label_name", "name", "constant",
    "instance_variable", "class_variable", "global_variable", "implicit_parameter",
}
SIGILS = "#@$"


def clean_name(text):
    """`#priv` → priv, `@ivar` → ivar, `@@count` → count, `$global` → global."""
    return text.lstrip(SIGILS)


def walk(node):
    yield node
    for child in node.children:
        yield from walk(child)


class Collector:
    def __init__(self, source):
        self.source = source
        self.found = []

    def text(self, node):
        return self.source[node.start_byte:node.end_byte].decode(errors="replace")

    def add(self, node, kind, owner, is_local, role=None):
        if node is None or not node.is_named:
            return
        name = clean_name(self.text(node))
        if name and name not in ("_", "this"):
            self.found.append((node, kind, role, owner, is_local, name))


def field_children(node, field):
    return [child for child in node.children_by_field_name(field) if child.is_named]


# ---------------------------------------------------------------- Python

def python_targets(node):
    """할당·반복·as 대상에서 새로 묶이는 이름 노드. `self.x`는 ("field", x)로 돌려준다."""
    if node.type == "identifier":
        yield "name", node
    elif node.type in ("pattern_list", "tuple_pattern", "list_pattern", "tuple", "list", "as_pattern_target",
                       "list_splat_pattern", "parenthesized_expression"):
        for child in node.named_children:
            yield from python_targets(child)
    elif node.type == "attribute":
        target = node.child_by_field_name("object")
        if target is not None and target.type == "identifier" and target.text in (b"self", b"cls"):
            yield "field", node.child_by_field_name("attribute")


def python_parameter_names(node):
    for child in node.named_children:
        if child.type == "identifier":
            yield child
        elif child.type in ("default_parameter", "typed_default_parameter"):
            yield child.child_by_field_name("name")
        elif child.type == "typed_parameter":
            first = child.named_children[0] if child.named_children else None
            if first is not None and first.type == "identifier":
                yield first
            elif first is not None:
                yield from (c for c in first.named_children if c.type == "identifier")
        elif child.type in ("list_splat_pattern", "dictionary_splat_pattern"):
            yield from (c for c in child.named_children if c.type == "identifier")


def visit_python(c, node, owner, is_local, context):
    kind = node.type
    if kind == "function_definition":
        name = node.child_by_field_name("name")
        c.add(name, "method" if context == "class_body" else "function", owner, is_local)
        return f"{owner}.{c.text(name)}" if owner else c.text(name), True, None
    if kind == "class_definition":
        name = node.child_by_field_name("name")
        c.add(name, "class", owner, is_local)
        return c.text(name), is_local, "class"
    if kind == "block" and context == "class":
        return owner, is_local, "class_body"
    if kind in ("parameters", "lambda_parameters"):
        for index, name in enumerate(python_parameter_names(node)):
            role = "receiver" if index == 0 and c.text(name) in ("self", "cls") else "parameter"
            c.add(name, "parameter", owner, True, role)
    elif kind == "lambda":
        return owner, True, None
    elif kind == "assignment":
        for target_kind, name in python_targets(node.child_by_field_name("left")):
            if target_kind == "field" or context == "class_body":
                c.add(name, "field", owner, is_local)
            else:
                text = c.text(name)
                is_constant = not is_local and text.upper() == text and any(ch.isalpha() for ch in text)
                c.add(name, "constant" if is_constant else "variable", owner, is_local)
    elif kind in ("for_statement", "for_in_clause"):
        for _, name in python_targets(node.child_by_field_name("left")):
            c.add(name, "variable", owner, is_local)
    elif kind == "as_pattern":
        alias = node.child_by_field_name("alias")
        for _, name in (python_targets(alias) if alias is not None else []):
            c.add(name, "variable", owner, is_local)
    elif kind == "named_expression":
        c.add(node.child_by_field_name("name"), "variable", owner, is_local)
    elif kind == "aliased_import":
        c.add(node.child_by_field_name("alias"), "import_alias", owner, is_local)
    # 클래스 본문의 문장(장식된 정의, 식 문장)까지만 클래스 본문으로 본다. 연쇄 대입(a = b = c)의 안쪽 대입도 같은 문장이다.
    return owner, is_local, context if kind in ("decorated_definition", "expression_statement", "assignment") else None


# ---------------------------------------------------------------- JavaScript / TypeScript

def js_pattern_names(node):
    if node is None:
        return
    if node.type in ("identifier", "shorthand_property_identifier_pattern"):
        yield node
    elif node.type == "pair_pattern":
        yield from js_pattern_names(node.child_by_field_name("value"))
    elif node.type in ("assignment_pattern", "object_assignment_pattern"):
        yield from js_pattern_names(node.child_by_field_name("left"))
    elif node.type in ("required_parameter", "optional_parameter"):
        yield from js_pattern_names(node.child_by_field_name("pattern"))
    elif node.type in ("rest_pattern", "object_pattern", "array_pattern"):
        for child in node.named_children:
            yield from js_pattern_names(child)


JS_TYPES = {"class_declaration": "class", "abstract_class_declaration": "class", "class": "class",
            "interface_declaration": "interface", "type_alias_declaration": "type_alias", "enum_declaration": "enum",
            "internal_module": "namespace", "module": "namespace"}
JS_FUNCTIONS = {"function_declaration", "generator_function_declaration", "function_expression", "generator_function",
                "function", "function_signature"}


def visit_js(c, node, owner, is_local, context):
    kind = node.type
    if kind in JS_FUNCTIONS:
        name = node.child_by_field_name("name")
        if name is not None:
            c.add(name, "function", owner, is_local)
        return (c.text(name) if name is not None else owner), True, None
    if kind in JS_TYPES:
        name = node.child_by_field_name("name")
        if name is not None and name.type != "string":
            c.add(name, JS_TYPES[kind], owner, is_local)
        return (c.text(name) if name is not None else owner), is_local, kind
    if kind == "method_definition":
        name = node.child_by_field_name("name")
        c.add(name, "method", owner, is_local)
        return f"{owner}.{c.text(name)}" if name is not None else owner, True, None
    if kind in ("method_signature", "abstract_method_signature"):
        c.add(node.child_by_field_name("name"), "interface_method" if context == "interface_declaration" else "method",
              owner, is_local)
    elif kind in ("field_definition", "public_field_definition"):
        c.add(node.child_by_field_name("property") or node.child_by_field_name("name"), "field", owner, is_local)
    elif kind == "property_signature":
        c.add(node.child_by_field_name("name"), "field", owner, is_local)
    elif kind == "enum_body":
        for child in node.named_children:
            c.add(child if child.type == "property_identifier" else child.child_by_field_name("name"),
                  "enum_member", owner, is_local)
    elif kind == "type_parameter":
        c.add(node.child_by_field_name("name"), "type_parameter", owner, is_local)
    elif kind == "variable_declarator":
        for name in js_pattern_names(node.child_by_field_name("name")):
            c.add(name, "variable", owner, is_local)
    elif kind == "formal_parameters":
        for child in node.named_children:
            for name in js_pattern_names(child):
                c.add(name, "parameter", owner, True)
    elif kind == "arrow_function":
        parameter = node.child_by_field_name("parameter")
        if parameter is not None:
            c.add(parameter, "parameter", owner, True)
        return owner, True, None
    elif kind == "catch_clause":
        for name in js_pattern_names(node.child_by_field_name("parameter")):
            c.add(name, "variable", owner, True)
    elif kind == "for_in_statement" and any(child.type in ("const", "let", "var") for child in node.children):
        for name in js_pattern_names(node.child_by_field_name("left")):
            c.add(name, "variable", owner, is_local)
    elif kind == "import_specifier":
        c.add(node.child_by_field_name("alias"), "import_alias", owner, is_local)
    elif kind == "namespace_import":
        for child in node.named_children:
            c.add(child, "import_alias", owner, is_local)
    elif kind == "labeled_statement":
        c.add(node.child_by_field_name("label"), "label", owner, is_local)
    return owner, is_local, context


# ---------------------------------------------------------------- Java

JAVA_TYPES = {"class_declaration": "class", "interface_declaration": "interface", "enum_declaration": "enum",
              "record_declaration": "record", "annotation_type_declaration": "interface"}


def visit_java(c, node, owner, is_local, context):
    kind = node.type
    if kind in JAVA_TYPES:
        name = node.child_by_field_name("name")
        c.add(name, JAVA_TYPES[kind], owner, is_local)
        return c.text(name), is_local, kind
    if kind in ("method_declaration", "constructor_declaration", "compact_constructor_declaration",
                "annotation_type_element_declaration"):
        name = node.child_by_field_name("name")
        method_kind = ("constructor" if kind.endswith("constructor_declaration")
                       else "interface_method" if context == "interface_declaration" else "method")
        c.add(name, method_kind, owner, is_local)
        return f"{owner}.{c.text(name)}" if name is not None else owner, True, None
    if kind in ("field_declaration", "constant_declaration", "local_variable_declaration"):
        declared = {"field_declaration": "field", "constant_declaration": "constant"}.get(kind, "variable")
        for declarator in field_children(node, "declarator"):
            c.add(declarator.child_by_field_name("name"), declared, owner, is_local)
    elif kind == "formal_parameter":
        c.add(node.child_by_field_name("name"), "field" if context == "record_declaration" else "parameter",
              owner, context != "record_declaration")
    elif kind == "spread_parameter":
        for declarator in (n for n in node.named_children if n.type == "variable_declarator"):
            c.add(declarator.child_by_field_name("name"), "parameter", owner, True)
    elif kind in ("catch_formal_parameter", "resource", "enhanced_for_statement", "instanceof_expression"):
        c.add(node.child_by_field_name("name"), "variable", owner, True)
    elif kind == "inferred_parameters":
        for child in node.named_children:
            c.add(child, "parameter", owner, True)
    elif kind == "lambda_expression":
        parameters = node.child_by_field_name("parameters")
        if parameters is not None and parameters.type == "identifier":
            c.add(parameters, "parameter", owner, True)
        return owner, True, None
    elif kind == "enum_constant":
        c.add(node.child_by_field_name("name"), "enum_member", owner, is_local)
    elif kind == "type_parameter":
        c.add(next((n for n in node.named_children if n.type in ("type_identifier", "identifier")), None),
              "type_parameter", owner, is_local)
    elif kind == "labeled_statement":
        c.add(next((n for n in node.named_children if n.type == "identifier"), None), "label", owner, True)
    elif kind in ("record_declaration_body", "class_body", "interface_body", "enum_body"):
        return owner, is_local, context if kind != "class_body" or context != "record_declaration" else None
    return owner, is_local, context if kind == "formal_parameters" else None


# ---------------------------------------------------------------- C#

CSHARP_TYPES = {"class_declaration": "class", "struct_declaration": "struct", "interface_declaration": "interface",
                "enum_declaration": "enum", "record_declaration": "record", "record_struct_declaration": "record",
                "delegate_declaration": "type"}


def csharp_pattern_names(node):
    """`(a, b)`·`var (a, b)`에서 묶이는 이름 노드."""
    if node is None:
        return
    if node.type == "identifier":
        yield node
    elif node.type in ("tuple_pattern", "parenthesized_variable_designation"):
        for child in node.named_children:
            yield from csharp_pattern_names(child)
    elif node.type in ("declaration_expression", "single_variable_designation"):
        name = node.child_by_field_name("name")
        yield from ([name] if name is not None else (n for n in node.named_children if n.type == "identifier"))


def csharp_declared_names(node):
    """node 바로 아래 variable_declaration의 선언자마다 선언된 이름 노드."""
    for declaration in (n for n in node.named_children if n.type == "variable_declaration"):
        for declarator in (n for n in declaration.named_children if n.type == "variable_declarator"):
            name = declarator.child_by_field_name("name")
            yield from ([name] if name is not None else csharp_pattern_names(declarator.named_children[0]))


def visit_csharp(c, node, owner, is_local, context):
    kind = node.type
    if kind in CSHARP_TYPES:
        name = node.child_by_field_name("name")
        c.add(name, CSHARP_TYPES[kind], owner, is_local)
        return c.text(name), is_local, kind
    if kind in ("namespace_declaration", "file_scoped_namespace_declaration"):
        name = node.child_by_field_name("name")
        for part in ([name] if name is not None and name.type == "identifier" else
                     [n for n in walk(name) if n.type == "identifier"] if name is not None else []):
            c.add(part, "namespace", owner, is_local)
        return c.text(name) if name is not None else owner, is_local, None
    if kind in ("method_declaration", "constructor_declaration", "local_function_statement", "operator_declaration"):
        name = node.child_by_field_name("name")
        method_kind = {"constructor_declaration": "constructor", "local_function_statement": "function"}.get(
            kind, "interface_method" if context == "interface_declaration" else "method")
        c.add(name, method_kind, owner, is_local)
        return f"{owner}.{c.text(name)}" if name is not None else owner, True, None
    if kind == "property_declaration":
        c.add(node.child_by_field_name("name"), "property", owner, is_local)
    elif kind in ("field_declaration", "event_field_declaration", "local_declaration_statement"):
        is_const = any(m.type == "modifier" and c.text(m) == "const" for m in node.named_children)
        declared = "variable" if kind == "local_declaration_statement" else "constant" if is_const else "field"
        for name in csharp_declared_names(node):
            c.add(name, declared, owner, is_local)
    elif kind in ("using_statement", "for_statement", "fixed_statement"):
        # using (var x = ...), for (var i = 0; ...), fixed (byte* p = ...)의 선언
        for name in csharp_declared_names(node):
            c.add(name, "variable", owner, True)
    elif kind == "parameter":
        c.add(node.child_by_field_name("name"), "parameter", owner, True)
    elif kind == "parameter_list":
        for child in node.children:
            if child.type == "identifier" and node.field_name_for_child(list(node.children).index(child)) == "name":
                c.add(child, "parameter", owner, True)
    elif kind == "implicit_parameter":
        c.add(node, "parameter", owner, True)
    elif kind in ("catch_declaration", "declaration_pattern", "declaration_expression"):
        c.add(node.child_by_field_name("name"), "variable", owner, True)
    elif kind == "foreach_statement":
        for name in csharp_pattern_names(node.child_by_field_name("left")):
            c.add(name, "variable", owner, True)
    elif kind == "enum_member_declaration":
        c.add(node.child_by_field_name("name"), "enum_member", owner, is_local)
    elif kind == "type_parameter":
        c.add(node.child_by_field_name("name"), "type_parameter", owner, is_local)
    elif kind == "using_directive":
        c.add(node.child_by_field_name("name"), "import_alias", owner, is_local)
    elif kind == "labeled_statement":
        c.add(next((n for n in node.named_children if n.type == "identifier"), None), "label", owner, True)
    elif kind == "lambda_expression":
        return owner, True, None
    return owner, is_local, context if kind in ("declaration_list",) else None


# ---------------------------------------------------------------- C / C++

DECLARATOR_LEAVES = {"identifier", "field_identifier", "type_identifier"}


def declarator_names(node):
    """선언자를 따라 내려가 선언된 이름 노드를 찾는다. 함수 선언자면 ("function", 이름, 범위)로 돌려준다."""
    is_function, scope = False, None
    while node is not None:
        if node.type in DECLARATOR_LEAVES:
            return [(is_function, node, scope)]
        if node.type in ("destructor_name", "operator_name", "template_function", "template_method"):
            return []
        if node.type == "qualified_identifier":
            scope = c_text_scope(node)
            node = node.child_by_field_name("name")
            continue
        if node.type == "structured_binding_declarator":
            return [(False, n, scope) for n in node.named_children if n.type == "identifier"]
        if node.type == "function_declarator":
            is_function = True
        inner = node.child_by_field_name("declarator")
        if inner is None:
            inner = next((n for n in node.named_children if n.type not in ("type_qualifier", "attribute_declaration",
                                                                          "ms_pointer_modifier")), None)
        node = inner
    return []


def c_text_scope(node):
    scope = node.child_by_field_name("scope")
    return scope.text.decode(errors="replace") if scope is not None else None


C_TYPES = {"struct_specifier": "struct", "union_specifier": "union", "class_specifier": "class", "enum_specifier": "enum"}
# Windows SAL 주석(`_In_`, `_In_opt_`, `_Out_writes_bytes_(n)`, `_Check_return_` ...)은 빈 매크로라 tree-sitter 해석만 깨뜨린다.
SAL_ANNOTATION = re.compile(rb"\b_[A-Z][a-z]+(?:_[a-z0-9]+)*_\b(?:\s*\((?:[^()]|\([^()]*\))*\))?")


def blank_sal_annotations(source):
    """C/C++ 원문에서 SAL 주석을 같은 길이의 공백으로 바꾼다(줄·열 위치 유지). 어노테이션 이름은 세지 않는 규칙과 같다."""
    return SAL_ANNOTATION.sub(lambda match: re.sub(rb"[^\n]", b" ", match.group(0)), source)


def visit_c(c, node, owner, is_local, context):
    kind = node.type
    if kind == "function_definition":
        for _, name, scope in declarator_names(node.child_by_field_name("declarator"))[:1]:
            c.add(name, "method" if scope or context == "class_body" else "function", scope or owner, is_local)
            return f"{scope or owner}::{c.text(name)}" if (scope or owner) else c.text(name), True, None
        return owner, True, None
    if kind in C_TYPES:
        name = node.child_by_field_name("name")
        if name is not None and name.type == "template_type":
            name = name.child_by_field_name("name")  # 특수화 `struct Wrapper<T<U>>`의 이름은 Wrapper
        if name is not None and node.child_by_field_name("body") is not None:
            c.add(name, C_TYPES[kind], owner, is_local)
            return c.text(name), is_local, "class_body"
        return owner, is_local, context
    if kind in ("declaration", "field_declaration"):
        for declarator in field_children(node, "declarator"):
            for is_function, name, scope in declarator_names(declarator):
                if is_function:
                    declared = "method" if context == "class_body" or kind == "field_declaration" else "function"
                elif kind == "field_declaration" or context == "class_body":
                    declared = "field"
                else:
                    declared = "variable"
                c.add(name, declared, scope or owner, is_local)
    elif kind in ("parameter_declaration", "optional_parameter_declaration", "variadic_parameter_declaration"):
        for _, name, _ in declarator_names(node.child_by_field_name("declarator")):
            c.add(name, "parameter", owner, True)
    elif kind == "type_definition":
        for declarator in field_children(node, "declarator"):
            for _, name, _ in declarator_names(declarator):
                c.add(name, "type", owner, is_local)
    elif kind == "alias_declaration":
        c.add(node.child_by_field_name("name"), "type_alias", owner, is_local)
    elif kind == "enumerator":
        c.add(node.child_by_field_name("name"), "enum_member", owner, is_local)
    elif kind in ("preproc_def", "preproc_function_def"):
        c.add(node.child_by_field_name("name"), "macro", owner, is_local)
    elif kind == "preproc_params":
        for child in node.named_children:
            c.add(child, "parameter", owner, True)
    elif kind == "namespace_definition":
        name = node.child_by_field_name("name")
        # 중첩 `namespace a::b::c`는 구성 요소마다 하나씩 센다.
        for part in ([name] if name is not None and name.type != "nested_namespace_specifier" else
                     [n for n in walk(name) if n.type == "namespace_identifier"] if name is not None else []):
            c.add(part, "namespace", owner, is_local)
        return c.text(name) if name is not None else owner, is_local, None
    elif kind in ("type_parameter_declaration", "variadic_type_parameter_declaration",
                  "optional_type_parameter_declaration"):
        c.add(node.child_by_field_name("name") or next((n for n in node.named_children if n.type == "type_identifier"), None),
              "type_parameter", owner, is_local)
    elif kind == "for_range_loop":
        for _, name, _ in declarator_names(node.child_by_field_name("declarator")):
            c.add(name, "variable", owner, True)
    elif kind == "labeled_statement":
        c.add(node.child_by_field_name("label"), "label", owner, True)
    elif kind == "lambda_expression":
        return owner, True, None
    if kind == "field_declaration_list":
        return owner, is_local, "class_body"
    return owner, is_local, None


# ---------------------------------------------------------------- Rust

def rust_pattern_names(node):
    if node is None:
        return
    for child in walk(node):
        if child.type == "shorthand_field_identifier":
            yield child
        elif child.type == "identifier":
            parent = child.parent
            field = parent.field_name_for_child(list(parent.children).index(child)) if parent is not None else None
            if field in ("type", "path", "name") and parent.type not in ("captured_pattern",):
                continue
            if parent is not None and parent.type in ("scoped_identifier", "scoped_type_identifier", "macro_invocation"):
                continue
            if c_is_constant_like(child):
                continue
            yield child


def c_is_constant_like(node):
    text = node.text.decode(errors="replace")
    return text[:1].isupper()


RUST_ITEMS = {"struct_item": "struct", "enum_item": "enum", "union_item": "union", "trait_item": "trait",
              "type_item": "type_alias", "mod_item": "module", "macro_definition": "macro", "const_item": "constant",
              "static_item": "constant"}


def visit_rust(c, node, owner, is_local, context):
    kind = node.type
    if kind in ("function_item", "function_signature_item"):
        name = node.child_by_field_name("name")
        c.add(name, "method" if context in ("impl", "trait") else "function", owner, is_local)
        return f"{owner}::{c.text(name)}" if owner else c.text(name), True, None
    if kind in RUST_ITEMS:
        name = node.child_by_field_name("name")
        c.add(name, RUST_ITEMS[kind], owner, is_local)
        return c.text(name) if name is not None else owner, is_local, "trait" if kind == "trait_item" else None
    if kind == "impl_item":
        target = node.child_by_field_name("type")
        return c.text(target) if target is not None else owner, is_local, "impl"
    if kind == "enum_variant":
        c.add(node.child_by_field_name("name"), "enum_member", owner, is_local)
    elif kind == "field_declaration":
        c.add(node.child_by_field_name("name"), "field", owner, is_local)
    elif kind in ("type_parameter", "const_parameter"):
        c.add(node.child_by_field_name("name"), "type_parameter", owner, is_local)
    elif kind == "parameter":
        for name in rust_pattern_names(node.child_by_field_name("pattern")):
            c.add(name, "parameter", owner, True)
    elif kind == "closure_parameters":
        for child in node.named_children:
            target = child.child_by_field_name("pattern") if child.type == "parameter" else child
            for name in rust_pattern_names(target):
                c.add(name, "parameter", owner, True)
        return owner, True, None
    elif kind in ("let_declaration", "for_expression", "let_condition"):
        for name in rust_pattern_names(node.child_by_field_name("pattern")):
            c.add(name, "variable", owner, True)
    elif kind == "match_pattern":
        guard = node.child_by_field_name("condition")
        for child in (n for n in node.named_children if n != guard):
            for name in rust_pattern_names(child):
                c.add(name, "variable", owner, True)
    return owner, is_local, context if kind == "declaration_list" else None


# ---------------------------------------------------------------- PHP

def php_variable(node):
    """`variable_name`의 이름 노드(`$x` → x)."""
    if node is not None and node.type == "variable_name":
        return next((n for n in node.named_children if n.type == "name"), None)
    return None


PHP_TYPES = {"class_declaration": "class", "interface_declaration": "interface", "trait_declaration": "trait",
             "enum_declaration": "enum"}


def visit_php(c, node, owner, is_local, context):
    kind = node.type
    if kind in PHP_TYPES:
        name = node.child_by_field_name("name")
        c.add(name, PHP_TYPES[kind], owner, is_local)
        return c.text(name), is_local, kind
    if kind in ("function_definition", "method_declaration"):
        name = node.child_by_field_name("name")
        c.add(name, "function" if kind == "function_definition" else
              "interface_method" if context == "interface_declaration" else "method", owner, is_local)
        return f"{owner}::{c.text(name)}" if owner and kind != "function_definition" else c.text(name), True, None
    if kind == "namespace_definition":
        name = node.child_by_field_name("name")
        for part in ([n for n in name.named_children if n.type == "name"] if name is not None else []):
            c.add(part, "namespace", owner, is_local)
    elif kind == "property_element":
        c.add(php_variable(node.child_by_field_name("name") or next(
            (n for n in node.named_children if n.type == "variable_name"), None)), "field", owner, is_local)
    elif kind == "const_element":
        c.add(next((n for n in node.named_children if n.type == "name"), None), "constant", owner, is_local)
    elif kind in ("simple_parameter", "variadic_parameter"):
        c.add(php_variable(node.child_by_field_name("name")), "parameter", owner, True)
    elif kind == "property_promotion_parameter":
        c.add(php_variable(node.child_by_field_name("name")), "field", owner, is_local)
    elif kind == "assignment_expression":
        left = node.child_by_field_name("left")
        targets = [left] if left is not None and left.type == "variable_name" else \
            [n for n in walk(left) if n.type == "variable_name"] if left is not None and left.type == "list_literal" else []
        for target in targets:
            c.add(php_variable(target), "variable", owner, is_local)
    elif kind == "foreach_statement":
        for child in node.named_children[1:]:
            for target in ([child] if child.type == "variable_name" else
                           [n for n in child.named_children if n.type == "variable_name"] if child.type in ("pair", "list_literal") else []):
                c.add(php_variable(target), "variable", owner, True)
    elif kind == "catch_clause":
        c.add(php_variable(node.child_by_field_name("name")), "variable", owner, True)
    elif kind == "enum_case":
        c.add(node.child_by_field_name("name"), "enum_member", owner, is_local)
    elif kind == "namespace_use_clause":
        c.add(node.child_by_field_name("alias"), "import_alias", owner, is_local)
    elif kind in ("anonymous_function", "arrow_function"):
        return owner, True, None
    return owner, is_local, context if kind == "declaration_list" else None


# ---------------------------------------------------------------- Ruby

RUBY_PARAMETERS = {"optional_parameter", "splat_parameter", "hash_splat_parameter", "keyword_parameter", "block_parameter"}


def visit_ruby(c, node, owner, is_local, context):
    kind = node.type
    if kind in ("method", "singleton_method"):
        name = node.child_by_field_name("name")
        c.add(name, "method", owner, is_local)
        return f"{owner}#{c.text(name)}" if owner else c.text(name), True, None
    if kind in ("class", "module"):
        name = node.child_by_field_name("name")
        if name is not None and name.type == "scope_resolution":
            name = name.child_by_field_name("name")
        c.add(name, kind, owner, is_local)
        return c.text(name) if name is not None else owner, is_local, None
    if kind in ("method_parameters", "block_parameters", "lambda_parameters"):
        for child in node.named_children:
            if child.type == "identifier":
                c.add(child, "parameter", owner, True)
            elif child.type in RUBY_PARAMETERS:
                c.add(child.child_by_field_name("name"), "parameter", owner, True)
            elif child.type == "destructured_parameter":
                for name in (n for n in walk(child) if n.type == "identifier"):
                    c.add(name, "parameter", owner, True)
    elif kind == "assignment":
        left = node.child_by_field_name("left")
        targets = [left] if left is not None and left.type != "left_assignment_list" else \
            list(left.named_children) if left is not None else []
        for target in targets:
            declared = {"identifier": "variable", "constant": "constant", "instance_variable": "field",
                        "class_variable": "field", "global_variable": "variable"}.get(target.type)
            if declared:
                c.add(target, declared, owner, is_local)
    elif kind == "for":
        pattern = node.child_by_field_name("pattern")
        for name in ([pattern] if pattern is not None and pattern.type == "identifier" else
                     [n for n in walk(pattern) if n.type == "identifier"] if pattern is not None else []):
            c.add(name, "variable", owner, True)
    return owner, is_local, None


VISITORS = {"python": visit_python, "javascript": visit_js, "typescript": visit_js, "tsx": visit_js,
            "java": visit_java, "csharp": visit_csharp, "c": visit_c, "cpp": visit_c, "rust": visit_rust,
            "php": visit_php, "ruby": visit_ruby}


def collect(language, tree, source):
    """정의마다 (name_node, kind, role, owner, is_local, name)."""
    c = Collector(source)
    visit = VISITORS[language]

    def recurse(node, owner, is_local, context):
        child_owner, child_local, child_context = visit(c, node, owner, is_local, context)
        for child in node.children:
            recurse(child, child_owner, child_local, child_context)

    recurse(tree.root_node, None, False, None)
    return c.found
