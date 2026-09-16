"""Bounded native generator suspension, separate from framework effect execution.

Only straight-line yield sites and delegation to another known native generator
are interpreted. Unsupported control flow remains an explicit boundary.
"""

from dataclasses import dataclass
import hashlib

from model import UNKNOWN, Fact, Value, slot, value_options
from syntax import FUNCTIONS, child, walk


GENERATOR_NODES = {"generator_function", "generator_function_declaration"}
CONDITIONS = ("generator_resume_order_required", "generator_execution_required")
MAX_STEPS = 64


def create_generator(caller, nested, function, node):
    if any(part.type == "async" for part in function.node.children):
        caller.analyzer.notices.add(("async_generator_unsupported", function.identifier))
        return UNKNOWN
    names = {nested.text(part) for part in walk(function.body)
             if part.type in {"identifier", "this"}}
    globals = caller.analyzer.globals.get(function.source.path, {})
    captures = {name: nested.env[name] for name in sorted(names) if name in nested.env and nested.env[name] != globals.get(name)}
    if len(captures) > 32:
        caller.analyzer.notices.add(("generator_capture_bound", function.identifier))
        return UNKNOWN
    identity = Value("allocation", caller.allocation_name(node) + ":generator")
    caller.analyzer.allocation_owners[identity] = caller.identifier
    captures["__source_callable_identity__"] = identity
    bindings = Value("capture_end")
    for name, value in reversed(list(captures.items())):
        bindings = Value("capture_binding", name, value, bindings)
    return Value("wrapper", "javascript:generator", Value("closure", function.identifier, bindings))


@dataclass
class Frame:
    interpreter: object
    statements: list
    index: int = 0
    pending: tuple | None = None
    delegate: Value | None = None
    delegate_site: object = None
    is_closed: bool = False
    steps: int = 0


def result(value, is_done):
    return Value("wrapper", "javascript:iterator_result", value, Value("literal", str(is_done).lower()))


def yield_site(statement):
    """Return (yield, binding, is_return); nested callable bodies do not suspend us."""
    current = statement
    if current.type in {"lexical_declaration", "variable_declaration"}:
        declarations = [part for part in current.named_children if part.type == "variable_declarator"]
        if len(declarations) != 1:
            return None
        current = declarations[0]
    if current.type == "variable_declarator":
        expression = child(current, "value")
        return (expression, child(current, "name"), False) if expression is not None and expression.type == "yield_expression" else None
    if current.type in {"return_statement", "expression_statement"}:
        expression = current.named_children[0] if current.named_children else None
        return (expression, None, current.type == "return_statement") if expression is not None and expression.type == "yield_expression" else None
    return None


def resume(interpreter, generator, sent, node, depth=0):
    from engine import Interpreter, closure_captures
    if depth >= 6:
        interpreter.analyzer.notices.add(("generator_delegation_bound", interpreter.identifier))
        return UNKNOWN
    frame = interpreter.analyzer.generator_frames.get(generator)
    if frame is None:
        function = interpreter.program.functions.get(generator.base.name)
        if function is None:
            return UNKNOWN
        nested = Interpreter(interpreter.analyzer, function.source, function,
                             conditions=interpreter.conditions + CONDITIONS, depth=interpreter.depth + 1)
        nested.env.update(closure_captures(generator.base))
        nested.instance_context = "~generator:" + hashlib.sha256(generator.display().encode()).hexdigest()[:16]
        statements = list(function.body.named_children) if function.body is not None else []
        frame = Frame(nested, statements)
        interpreter.analyzer.generator_frames[generator] = frame
    if frame.is_closed:
        return result(Value("literal", "undefined"), True)
    nested = frame.interpreter
    start = len(nested.facts)

    def publish(value):
        for fact in nested.facts[start:]:
            interpreter.append_fact(Fact(fact.kind, fact.target, fact.value, fact.location, fact.function,
                                         tuple(sorted(set(fact.conditions + interpreter.conditions + CONDITIONS))),
                                         tuple(dict.fromkeys(fact.via + (interpreter.source.location(node),))), fact.argument_index, fact.consumer))
        interpreter.conditions += CONDITIONS
        return value

    while frame.steps < MAX_STEPS:
        frame.steps += 1
        if frame.delegate is not None:
            delegated = resume(nested, frame.delegate, sent, frame.delegate_site, depth + 1)
            if delegated.kind != "wrapper" or delegated.name != "javascript:iterator_result":
                frame.is_closed = True
                return publish(UNKNOWN)
            if delegated.key == Value("literal", "false"):
                return publish(delegated)
            sent, frame.delegate = delegated.base, None
        if frame.pending is not None:
            pattern, is_return = frame.pending
            frame.pending = None
            if is_return:
                frame.is_closed = True
                return publish(result(sent, True))
            if pattern is not None:
                nested.bind(pattern, sent)
        if frame.index == len(frame.statements):
            frame.is_closed = True
            return publish(result(Value("literal", "undefined"), True))
        statement = frame.statements[frame.index]
        frame.index += 1
        site = yield_site(statement)
        if site is not None:
            expression, pattern, is_return = site
            value = nested.expression(expression.named_children[-1] if expression.named_children else None)
            frame.pending = (pattern, is_return)
            if any(part.type == "*" for part in expression.children):
                actuals = [value] + nested.analyzer.read_values(value)
                generators = list(dict.fromkeys(item for item in actuals if item.kind == "wrapper" and item.name == "javascript:generator"))
                if len(generators) != 1:
                    nested.analyzer.notices.add(("generator_delegate_unresolved", nested.identifier))
                    frame.is_closed = True
                    return publish(UNKNOWN)
                frame.delegate, frame.delegate_site = generators[0], expression
                sent = Value("literal", "undefined")
                continue
            return publish(result(value, False))
        if statement.type not in FUNCTIONS and any(part.type in {"yield_expression", "throw_statement"}
               or (part.type == "return_statement" and part != statement)
               for part in walk(statement, stop_functions=True)):
            nested.analyzer.notices.add(("generator_control_flow_unsupported", nested.identifier))
            frame.is_closed = True
            return publish(UNKNOWN)
        if statement.type == "return_statement":
            value = nested.expression(statement.named_children[0] if statement.named_children else None)
            frame.is_closed = True
            return publish(result(value, True))
        nested.statement(statement)
    nested.analyzer.notices.add(("generator_step_bound", nested.identifier))
    frame.is_closed = True
    return publish(UNKNOWN)


def generator_call(interpreter, receiver, method, arguments, node):
    if method != "next" or len(arguments) > 1:
        return None
    if interpreter.analyzer.heap_values.get(slot(receiver, "next")):
        return None
    actuals = value_options(receiver) + [option for value in interpreter.analyzer.read_values(receiver) for option in value_options(value)]
    generators = list(dict.fromkeys(value for value in actuals if value.kind == "wrapper" and value.name == "javascript:generator"))
    if len(generators) != 1:
        return None
    return resume(interpreter, generators[0], arguments[0] if arguments else Value("literal", "undefined"), node)
