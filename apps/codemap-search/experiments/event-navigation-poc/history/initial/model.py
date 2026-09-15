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
        return f"{self.kind}:{self.name}"


UNKNOWN = Value("unknown")
ELEMENT = Value("element", "*")
MAP_ENTRIES = Value("container", "entries")
MAP_KEYS = Value("container", "keys")


def slot(base: Value, key: Value | str) -> Value:
    return Value("slot", base=base, key=Value("key", key) if isinstance(key, str) else key)


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
    return value


@dataclass(frozen=True)
class Fact:
    kind: str
    target: Value
    value: Value
    location: Location
    function: str
    conditions: tuple[str, ...] = ()
    via: tuple[Location, ...] = ()


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
    if left_root != right_root or left_root.kind not in {"receiver", "allocation", "global"}:
        return None
    if not left_keys or len(left_keys) > len(right_keys):
        return None
    conditions = []
    if left_root.kind == "receiver":
        conditions.append("same_receiver_required")
    if left_root.kind == "allocation":
        conditions.append("same_allocation_instance_required")
    for index, (left, right) in enumerate(zip(left_keys, right_keys)):
        if left == right:
            if left.kind not in {"key", "literal", "element", "container"}:
                conditions.append(f"key_equality_required:{left.display()}={right.display()}")
            continue
        if left.kind == "container" or right.kind == "container":
            return None
        if left.kind == "element" or right.kind == "element":
            other = right if left.kind == "element" else left
            is_map_entry = index > 0 and left_keys[index - 1] == MAP_ENTRIES
            if other.kind == "key" and not is_map_entry:
                return None
            conditions.append("collection_membership_required")
            continue
        if left.kind == "key" and right.kind == "key":
            return None
        # A dynamic key is an explicit equality obligation, not a resolved event key.
        conditions.append(f"key_equality_required:{left.display()}={right.display()}")
    return tuple(conditions)


def connect(facts: list[Fact], max_relations: int = 2048) -> tuple[list[dict], int]:
    stores = [fact for fact in facts if fact.kind == "store"]
    calls = [fact for fact in facts if fact.kind in {"invoke", "member_invoke"}]
    mutations = [fact for fact in facts if fact.kind == "remove"]
    by_root: dict[Value, list[Fact]] = {}
    for fact in stores:
        by_root.setdefault(split_path(fact.target)[0], []).append(fact)
    relations = []
    seen = set()
    omitted = 0
    for call in calls:
        for store in by_root.get(split_path(call.target)[0], ()):
            conditions = match_storage(store.target, call.target)
            if conditions is None:
                continue
            key = (store.location, call.location, store.target, call.target)
            if key in seen:
                continue
            seen.add(key)
            related_mutations = [fact for fact in mutations if match_storage(fact.target, call.target) is not None]
            related_stores = [fact for fact in by_root.get(split_path(call.target)[0], ())
                              if match_storage(fact.target, call.target) is not None]
            obligations = set(conditions + store.conditions + call.conditions)
            obligations.add("registration_and_call_order_unproven")
            if store.value.kind in {"unknown", "result", "unresolved"}:
                obligations.add("stored_value_unresolved")
            if related_mutations:
                obligations.add("removal_may_prevent_call")
            if len({fact.location for fact in related_stores}) > 1:
                obligations.add("multiple_writes_may_replace_value")
            if len(relations) >= max_relations:
                omitted += 1
                continue
            relations.append({
                "kind": "storage_to_invocation" if call.kind == "invoke" else "stored_object_method_candidate",
                "certainty": "conditional_source_relation",
                "event_classification": "not_inferred",
                "storage": store,
                "invocation": call,
                "conditions": sorted(obligations),
                "mutations": [fact.location for fact in related_mutations],
            })
    return relations, omitted
