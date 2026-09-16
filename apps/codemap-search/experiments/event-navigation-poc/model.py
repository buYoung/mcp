"""Language-neutral symbolic storage facts. This module knows no event APIs."""

from dataclasses import dataclass, field


@dataclass(frozen=True, order=True)
class Location:
    path: str
    line: int
    column: int = 1


@dataclass(frozen=True)
class Value:
    kind: str
    name: str = ""
    base: "Value | None" = None
    key: "Value | None" = None

    def display(self) -> str:
        if self.kind == "slot":
            return f"{self.base.display()}[{self.key.display()}]"
        if self.kind == "iterator":
            return f"iterator:{self.name}({self.base.display()})"
        if self.kind == "choice":
            return f"choice({self.base.display()}|{self.key.display()})"
        if self.kind == "tuple":
            return "tuple(" + ",".join(value.display() for value in tuple_values(self)) + ")"
        if self.kind in {"wrapper", "bound_method", "closure", "object_copy"}:
            suffix = f",{self.key.display()}" if self.key is not None else ""
            return f"{self.kind}:{self.name}({self.base.display()}{suffix})"
        if self.kind == "capture_binding":
            return f"capture:{self.name}={self.base.display()};{self.key.display()}"
        return f"{self.kind}:{self.name}"


UNKNOWN = Value("unknown")
ELEMENT = Value("element", "*")
MAP_ENTRIES = Value("container", "entries")
MAP_KEYS = Value("container", "keys")
CALL_RELATIONS = {"storage_to_invocation", "stored_object_method_candidate"}
DATA_RELATIONS = {"stored_value_return"}
CONSUMPTION_RELATIONS = {"stored_object_write", "stored_object_read", "stored_key_lookup"}


def slot(base: Value, key: Value | str) -> Value:
    if base.kind == "choice":
        return merge_values([slot(option, key) for option in value_options(base)])
    return Value("slot", base=base, key=Value("key", key) if isinstance(key, str) else key)


def value_options(value: Value) -> list[Value]:
    if value.kind == "choice":
        return value_options(value.base) + value_options(value.key)
    return [value]


def merge_values(values: list[Value]) -> Value:
    options = list(dict.fromkeys(option for value in values for option in value_options(value)))
    if not options:
        return UNKNOWN
    if len(options) > 8:
        return Value("unknown", "value_alternative_cap")
    result = options[0]
    for option in options[1:]:
        result = Value("choice", base=result, key=option)
    return result


def tuple_value(values: list[Value]) -> Value:
    if len(values) > 8:
        return Value("unknown", "tuple_arity_cap")
    result = Value("tuple_end")
    for value in reversed(values):
        result = Value("tuple", base=value, key=result)
    return result


def tuple_values(value: Value) -> list[Value]:
    values = []
    while value.kind == "tuple":
        values.append(value.base)
        value = value.key
    return values


def split_path(value: Value) -> tuple[Value, tuple[Value, ...]]:
    keys = []
    while value.kind == "slot":
        keys.append(value.key)
        value = value.base
    return value, tuple(reversed(keys))


def substitute(value: Value, replacements: dict[Value, Value]) -> Value:
    if value in replacements:
        return replacements[value]
    if value.kind == "slot":
        return slot(substitute(value.base, replacements), substitute(value.key, replacements))
    if value.kind == "iterator":
        return Value("iterator", value.name, substitute(value.base, replacements))
    if value.kind == "choice":
        return merge_values([substitute(option, replacements) for option in value_options(value)])
    if value.kind == "tuple":
        return tuple_value([substitute(item, replacements) for item in tuple_values(value)])
    if value.kind in {"wrapper", "bound_method", "closure", "object_copy", "capture_binding"}:
        return Value(value.kind, value.name, substitute(value.base, replacements),
                     substitute(value.key, replacements) if value.key is not None else None)
    return value


def referenced_values(value: Value):
    yield value
    if value.base is not None:
        yield from referenced_values(value.base)
    if value.key is not None:
        yield from referenced_values(value.key)


@dataclass(frozen=True)
class Consumer:
    location: Location
    target: Value
    kind: str
    argument_index: int | None = None


@dataclass(frozen=True)
class Fact:
    kind: str
    target: Value
    value: Value
    location: Location
    function: str
    conditions: tuple[str, ...] = ()
    # One representative provenance path is retained; the same semantic fact
    # reached through another call chain must not consume another fact slot.
    via: tuple[Location, ...] = field(default=(), compare=False, hash=False)
    argument_index: int | None = None
    # Demand-derived property projections are valid only for the exact source
    # consumer that requested them, never as unconditional heap writes.
    consumer: Consumer | None = None


@dataclass
class Summary:
    name: str
    path: str
    owner: str
    receiver: Value
    parameters: tuple[Value, ...]
    facts: list[Fact] = field(default_factory=list)
    returns: list[Value] = field(default_factory=list)


def match_storage(stored: Value, called: Value) -> tuple[str, ...] | None:
    """Match a symbolic schema, never merge receivers by a field/key spelling."""
    left_root, left_keys = split_path(stored)
    right_root, right_keys = split_path(called)
    if left_root != right_root or left_root.kind not in {"receiver", "allocation", "global", "parameter", "binding"}:
        return None
    if not left_keys or len(left_keys) > len(right_keys):
        return None
    conditions = []
    if left_root.kind == "receiver":
        conditions.append("same_receiver_required")
    if left_root.kind == "allocation":
        conditions.append("same_allocation_instance_required")
    if left_root.kind == "global":
        conditions.append("same_runtime_context_required")
    if left_root.kind == "parameter":
        conditions.append("same_parameter_binding_required")
    if left_root.kind == "binding":
        conditions.extend(("same_lexical_value_binding_required", "initializer_value_unresolved"))
    for index, (left, right) in enumerate(zip(left_keys, right_keys)):
        if left.kind == "opaque_key" or right.kind == "opaque_key":
            return None
        if left == right:
            if left.kind == "element":
                conditions.append("collection_membership_required")
            if left.kind not in {"key", "literal", "tuple_index", "element", "container"}:
                conditions.append(f"key_equality_required:{left.display()}={right.display()}")
            continue
        if left.kind == "container" or right.kind == "container":
            return None
        if left.kind == "tuple_index" or right.kind == "tuple_index":
            return None
        if left.kind == "element" or right.kind == "element":
            other = right if left.kind == "element" else left
            is_map_entry = index > 0 and left_keys[index - 1] == MAP_ENTRIES
            if other.kind == "key" and not is_map_entry:
                return None
            conditions.append("collection_membership_required")
            continue
        if left.kind == right.kind and left.kind in {"key", "literal"}:
            return None
        # A dynamic key is an explicit equality obligation, not a resolved event key.
        conditions.append(f"key_equality_required:{left.display()}={right.display()}")
    return tuple(conditions)


def connect(facts: list[Fact], max_relations: int = 2048) -> tuple[list[dict], int]:
    stores = [fact for fact in facts if fact.kind == "store"]
    calls = [fact for fact in facts if fact.kind in {"invoke", "member_invoke", "argument", "return", "key_lookup"}]
    calls.extend(fact for fact in stores if MAP_ENTRIES in split_path(fact.target)[1])
    priorities = {"invoke": 0, "member_invoke": 1, "argument": 2, "return": 3, "key_lookup": 4, "store": 4}
    calls.sort(key=lambda fact: (priorities[fact.kind], fact.location, fact.argument_index or 0))
    mutations = [fact for fact in facts if fact.kind == "remove"]
    by_root: dict[Value, list[Fact]] = {}
    for fact in stores:
        by_root.setdefault(split_path(fact.target)[0], []).append(fact)
    relations = []
    relation_buckets = {}
    argument_groups = {}
    seen = set()
    omitted = 0
    for call in calls:
        for store in by_root.get(split_path(call.target)[0], ()):
            if store.consumer is not None and store.consumer != Consumer(call.location, call.target, call.kind, call.argument_index):
                continue
            if call.kind == "invoke" and store.value.kind in {"literal", "key"}:
                continue
            _, stored_keys = split_path(store.target)
            _, called_keys = split_path(call.target)
            # A container/config initializer is not evidence that one particular
            # nested callback was stored. Explicit field projection supplies the
            # more precise fact when that nested value is visible in source.
            distance = len(called_keys) - len(stored_keys)
            if call.kind == "store":
                if distance < 1 or distance > 6 or store.location == call.location:
                    continue
                if stored_keys[-1] == MAP_ENTRIES or MAP_ENTRIES not in stored_keys:
                    continue
            elif call.kind == "return" and distance > 0 and MAP_ENTRIES in stored_keys:
                if distance > 6:
                    continue
            elif distance != 0 and not (call.kind == "member_invoke" and distance == 1):
                continue
            conditions = match_storage(store.target, call.target)
            if conditions is None:
                continue
            key = (store.location, call.location, store.target, store.value, call.target, call.kind, call.argument_index)
            if key in seen:
                continue
            seen.add(key)
            argument_group = None
            if call.kind == "argument":
                def shape(keys):
                    return tuple(value.display() if value.kind in {"key", "literal", "tuple_index", "element", "container"}
                                 else "<dynamic>" for value in keys)
                argument_group = (store.location, call.location, split_path(store.target)[0],
                                  shape(stored_keys), shape(called_keys), call.argument_index, call.value)
                existing = argument_groups.get(argument_group)
                if existing is not None:
                    existing["alternative_count"] += 1
                    # These are alternate symbolic keys/contexts for the same
                    # source-level argument transfer, not different endpoints.
                    if len(existing["alternative_examples"]) < 3:
                        existing["alternative_examples"].append({"storage_target": store.target.display(),
                                                                 "argument_target": call.target.display(),
                                                                 "conditions": list(conditions + store.conditions + call.conditions)})
                    continue
            related_mutations = [fact for fact in mutations if match_storage(fact.target, call.target) is not None]
            related_stores = [fact for fact in by_root.get(split_path(call.target)[0], ())
                              if match_storage(fact.target, call.target) is not None]
            obligations = set(conditions + store.conditions + call.conditions)
            obligations.add("registration_and_call_order_unproven")
            if call.kind == "argument":
                obligations.add("callee_consumption_unproven")
            if call.kind == "return":
                obligations.add("returned_value_only_not_callback_execution")
            if call.kind == "store":
                obligations.add("object_write_only_not_callback_execution")
            if call.kind == "key_lookup":
                obligations.add("lookup_key_only_not_callback_execution")
            if store.value.kind in {"unknown", "result", "unresolved", "binding"}:
                obligations.add("stored_value_unresolved")
            if related_mutations:
                obligations.add("removal_may_prevent_call")
            if len({fact.location for fact in related_stores}) > 1:
                obligations.add("multiple_storage_writes_present")
            category = "call" if call.kind in {"invoke", "member_invoke"} else "argument" if call.kind == "argument" else "data"
            bucket = relation_buckets.setdefault((category, call.location.path), [])
            if len(bucket) >= min(512, max_relations):
                omitted += 1
                continue
            relation_kind = {"invoke": "storage_to_invocation", "member_invoke": "stored_object_method_candidate",
                             "argument": "stored_value_argument", "return": "stored_value_return",
                             "store": "stored_object_write", "key_lookup": "stored_key_lookup"}[call.kind]
            if call.kind == "return" and distance > 0:
                relation_kind = "stored_object_read"
                obligations.add("object_read_only_not_callback_execution")
            root, _ = split_path(store.target)
            relation = {
                "kind": relation_kind,
                "certainty": "conditional_source_relation",
                "identity_scope": "receiver_schema" if root.kind == "receiver" else
                                  ("parameter_binding" if root.kind == "parameter" else "allocation_or_global_context"),
                "concrete_instance_proven": False,
                "event_classification": "not_inferred",
                "storage": store,
                "invocation": call,
                "conditions": sorted(obligations),
                "mutations": [fact.location for fact in related_mutations],
            }
            if argument_group is not None:
                relation["alternative_count"] = 1
                relation["alternative_examples"] = [{"storage_target": store.target.display(),
                                                       "argument_target": call.target.display(),
                                                       "conditions": sorted(obligations)}]
                relation["conditions_are_representative"] = True
                argument_groups[argument_group] = relation
            relations.append(relation)
            bucket.append(relation)
    if len(relations) <= max_relations:
        return relations, omitted
    selected = []
    offsets = {key: 0 for key in relation_buckets}
    while len(selected) < max_relations:
        progressed = False
        for key, bucket in relation_buckets.items():
            offset = offsets[key]
            if offset < len(bucket) and len(selected) < max_relations:
                selected.append(bucket[offset])
                offsets[key] += 1
                progressed = True
        if not progressed:
            break
    return selected, omitted + len(relations) - len(selected)
