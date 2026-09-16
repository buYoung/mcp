"""Qualified Rust pointer primitives; a type alone never supplies a pointee."""

from model import UNKNOWN, Value, merge_values, slot, value_options
from syntax import child
import re


POINTERS = {"rust:nonnull", "rust:raw_pointer"}
POINTER_CONDITIONS = ("pointer_provenance_required", "pointer_lifetime_alignment_and_aliasing_unproven")


def option_result_call(interpreter, receiver, method, arguments, node, actuals):
    """Operate only on proven standard variants, preserving take's old payload."""
    variants = [item for item in actuals if item.kind == "wrapper" and item.name in
                {"rust:option", "rust:option_none", "rust:result_ok", "rust:result_err"}]
    if not variants:
        return None
    options = [item for item in variants if item.name in {"rust:option", "rust:option_none"}]
    if options and method == "take" and not arguments:
        empty = Value("wrapper", "rust:option_none", UNKNOWN)
        # Return the captured values, never an alias of the newly empty slot.
        returned = merge_values(options)
        interpreter.emit("remove", receiver, UNKNOWN, node, ("standard_option_take",))
        if receiver.kind == "slot":
            interpreter.emit("store", receiver, empty, node, ("standard_option_take",))
        else:
            function = child(node, "function")
            base = child(function, "value")
            while base is not None and base.type in {"parenthesized_expression", "reference_expression"}:
                base = child(base, "value") or (base.named_children[-1] if base.named_children else None)
            if base is not None and base.type == "identifier":
                interpreter.env[interpreter.text(base)] = empty
        interpreter.conditions += ("standard_option_take",)
        return returned
    payloads = [item.base for item in options if item.name == "rust:option"]
    if options and method in {"as_ref", "as_mut", "as_deref", "as_deref_mut"} and not arguments:
        if not payloads:
            return Value("wrapper", "rust:option_none", UNKNOWN)
        if method in {"as_deref", "as_deref_mut"}:
            pointed = []
            for value in payloads:
                stored = [value] + interpreter.analyzer.read_values(value)
                if not any(interpreter.analyzer.value_types.get(part) == "std::boxed::Box<_>" for part in stored):
                    return None
                pointed.append(slot(value, "pointee"))
            payloads = pointed
            interpreter.conditions += ("standard_box_deref_semantics",)
        return Value("wrapper", "rust:option", merge_values(payloads))
    if options and method == "ok_or" and len(arguments) == 1:
        return merge_values([Value("wrapper", "rust:result_ok", item.base) if item.name == "rust:option"
                             else Value("wrapper", "rust:result_err", arguments[0]) for item in options])
    results = [item for item in variants if item.name in {"rust:result_ok", "rust:result_err"}]
    if results and method == "map_err" and len(arguments) == 1:
        return merge_values([item if item.name == "rust:result_ok" else Value("wrapper", "rust:result_err", UNKNOWN) for item in results])
    if method in {"unwrap", "expect"} and len(arguments) == int(method == "expect"):
        values = [item.base for item in variants if item.name in {"rust:option", "rust:result_ok"}]
        if not values:
            return UNKNOWN
        interpreter.conditions += ("option_some_required" if options else "result_ok_required",)
        return merge_values(values)
    return None


def concrete_type_identity(interpreter, node):
    """TypeId keys require fully resolved types, including every generic argument."""
    from rust_types import type_expression
    result = type_expression(interpreter, node)
    return result.display() if result.is_concrete else None


def type_id_call(interpreter, callee_text, callee_node, arguments):
    if arguments or not interpreter.rust_standard_path(callee_text, {"std::any::TypeId::of", "core::any::TypeId::of"}):
        return None
    args = child(callee_node, "type_arguments")
    identity = concrete_type_identity(interpreter, args.named_children[0]) if args is not None and len(args.named_children) == 1 else None
    if identity:
        interpreter.conditions += ("standard_type_id_identity",)
        return Value("key", "rust:type_id:" + identity)
    interpreter.analyzer.notices.add(("type_id_type_argument_unresolved", interpreter.identifier))
    return Value("opaque_key", interpreter.identifier + ":type_id:" + interpreter.text(args))


def map_constructor_call(interpreter, callee_text, arguments, node):
    constructors = {"std::collections::HashMap::new", "std::collections::hash_map::HashMap::new",
                    "std::collections::BTreeMap::new", "std::collections::btree_map::BTreeMap::new"}
    if arguments or not interpreter.rust_standard_path(callee_text, constructors):
        return None
    value = Value("allocation", interpreter.allocation_name(node) + ":map")
    interpreter.analyzer.allocation_owners[value] = interpreter.identifier
    interpreter.analyzer.value_types[value] = "map"
    interpreter.conditions += ("standard_map_constructor",)
    return value


def any_downcast_call(interpreter, receiver, method, arguments, callee_node):
    if arguments or method not in {"downcast_ref", "downcast_mut"}:
        return None
    declared = interpreter.analyzer.type_text(receiver, interpreter.source).strip()
    declared = re.sub(r"^&(?:'\w+\s*)?(?:mut\s*)?", "", declared)
    boxed = re.fullmatch(r"((?:(?:std|alloc)::boxed::)?Box)\s*<\s*dyn\s+(.+)>", declared)
    trait = boxed[2] if boxed else (declared[4:] if declared.startswith("dyn ") else "")
    trait = trait.split("+", 1)[0].strip()
    if not trait or not interpreter.rust_standard_path(trait, {"std::any::Any", "core::any::Any"}):
        return None
    if boxed and not interpreter.rust_standard_path(boxed[1] + "::new", {"std::boxed::Box::new", "alloc::boxed::Box::new"}):
        return None
    args = child(callee_node, "type_arguments")
    identity = concrete_type_identity(interpreter, args.named_children[0]) if args is not None and len(args.named_children) == 1 else None
    if not identity or "<" in identity:
        interpreter.analyzer.notices.add(("any_downcast_target_unresolved", interpreter.identifier))
        return UNKNOWN
    pointed = slot(receiver, "pointee") if boxed else receiver
    actuals = interpreter.analyzer.read_values(pointed)
    owners = {interpreter.analyzer.owner(actual, interpreter.source) for actual in actuals}
    if owners and "" not in owners and identity not in owners:
        interpreter.analyzer.notices.add(("known_any_downcast_type_mismatch", interpreter.identifier))
        return UNKNOWN
    conditions = interpreter.conditions + ("standard_any_downcast_type_match_required",)
    if boxed:
        conditions += ("standard_box_deref_semantics",)
    interpreter.analyzer.require_value_conditions(pointed, conditions)
    interpreter.analyzer.value_types[pointed] = identity
    interpreter.analyzer.value_owners[pointed] = identity
    return Value("wrapper", "rust:option", pointed)


def pointer_values(interpreter, value):
    values = value_options(value) + [option for stored in interpreter.analyzer.read_values(value)
                                     for option in value_options(stored)]
    return list(dict.fromkeys(item for item in values if item.kind == "wrapper" and item.name in POINTERS))


def pointer_cast(interpreter, node, value):
    """Retain an existing address origin, never recover one from an integer."""
    target = child(node, "type")
    if target is None or target.type != "pointer_type":
        return None
    origins = pointer_values(interpreter, value)
    if origins:
        interpreter.conditions += POINTER_CONDITIONS + ("pointer_cast_layout_compatibility_required",)
        return merge_values([Value("wrapper", "rust:raw_pointer", item.base) for item in origins])
    operand = child(node, "value")
    declared = interpreter.analyzer.type_text(value, interpreter.source).strip()
    if (operand is not None and operand.type == "reference_expression") or declared.startswith("&"):
        interpreter.conditions += POINTER_CONDITIONS + ("pointer_cast_layout_compatibility_required",)
        return Value("wrapper", "rust:raw_pointer", value)
    interpreter.analyzer.notices.add(("pointer_origin_unresolved", interpreter.identifier))
    return UNKNOWN


def pointer_dereference(interpreter, value):
    pointers = pointer_values(interpreter, value)
    if pointers:
        interpreter.conditions += POINTER_CONDITIONS
        return merge_values([item.base for item in pointers])
    return None


def pointer_call(interpreter, callee_text, receiver, method, arguments, node):
    paths = {f"{root}::mem::MaybeUninit::new" for root in ("std", "core")}
    if len(arguments) == 1 and interpreter.rust_standard_path(callee_text, paths):
        value = Value("wrapper", "rust:maybeuninit_initialized", arguments[0])
        interpreter.analyzer.require_value_conditions(value, ("standard_maybeuninit_initialized_payload",))
        return value
    initialized = [value for value in value_options(receiver) + interpreter.analyzer.read_values(receiver)
                   if value.kind == "wrapper" and value.name == "rust:maybeuninit_initialized"]
    if initialized and not arguments and method in {"as_ptr", "as_mut_ptr"}:
        interpreter.conditions += POINTER_CONDITIONS + ("standard_maybeuninit_initialized_payload",)
        return merge_values([Value("wrapper", "rust:raw_pointer", value.base) for value in initialized])
    paths = {f"{root}::ptr::NonNull::from" for root in ("std", "core")}
    if len(arguments) == 1 and interpreter.rust_standard_path(callee_text, paths):
        nodes = child(node, "arguments")
        operand = nodes.named_children[0] if nodes is not None and len(nodes.named_children) == 1 else None
        declared = interpreter.analyzer.type_text(arguments[0], interpreter.source).strip()
        if operand is not None and operand.type == "identifier" and interpreter.function:
            declared = dict(interpreter.function.parameters).get(interpreter.text(operand), declared)
        if (operand is not None and operand.type == "reference_expression") or declared.startswith("&"):
            value = arguments[0]
            if value.kind == "wrapper" and value.name == "rust:maybeuninit_initialized":
                interpreter.conditions += ("standard_maybeuninit_initialized_payload",)
                value = value.base
            interpreter.conditions += POINTER_CONDITIONS
            return Value("wrapper", "rust:nonnull", value)
        return UNKNOWN
    for constructor in ("from_ref", "from_mut"):
        paths = {f"{root}::ptr::NonNull::{constructor}" for root in ("std", "core")}
        if len(arguments) == 1 and interpreter.rust_standard_path(callee_text, paths):
            interpreter.conditions += POINTER_CONDITIONS
            return Value("wrapper", "rust:nonnull", arguments[0])
    paths = {f"{root}::ptr::NonNull::new_unchecked" for root in ("std", "core")}
    if len(arguments) == 1 and interpreter.rust_standard_path(callee_text, paths):
        pointers = pointer_values(interpreter, arguments[0])
        if pointers:
            interpreter.conditions += POINTER_CONDITIONS + ("nonnull_precondition_required",)
            return merge_values([Value("wrapper", "rust:nonnull", item.base) for item in pointers])
        interpreter.analyzer.notices.add(("pointer_origin_unresolved", interpreter.identifier))
        return UNKNOWN
    if arguments or method not in {"as_ref", "as_mut", "as_ptr", "cast", "clone", "read"}:
        return None
    pointers = pointer_values(interpreter, receiver)
    if method == "as_ptr":
        pointers = [item for item in pointers if item.name == "rust:nonnull"]
    if method == "read":
        pointers = [item for item in pointers if item.name == "rust:raw_pointer"]
    if not pointers:
        return None
    interpreter.conditions += POINTER_CONDITIONS
    interpreter.conditions += tuple(sorted({condition for value in pointers
                                           for condition in interpreter.analyzer.value_conditions.get(value, ())}))
    if method == "read":
        interpreter.conditions += ("raw_pointer_read_requires_initialized_valid_value", "pointer_read_may_invalidate_source")
        return merge_values([item.base for item in pointers])
    if method in {"as_ref", "as_mut"}:
        # Raw-pointer as_ref/as_mut returns Option. NonNull returns a reference.
        return merge_values([Value("wrapper", "rust:option", item.base) if item.name == "rust:raw_pointer"
                             else item.base for item in pointers])
    if method == "cast":
        interpreter.conditions += ("pointer_cast_layout_compatibility_required",)
        return merge_values(pointers)
    if method == "clone":
        return merge_values(pointers)
    return merge_values([Value("wrapper", "rust:raw_pointer", item.base) for item in pointers])
