from typing import Protocol


class Rule(Protocol):
    def check(self, order) -> bool: ...


def evaluate(order, rules: list[Rule]) -> list[str]:
    failures: list[str] = []
    for rule in rules:
        passed = rule.check(order)
        if not passed:
            failures.append(rule.__class__.__name__)

    all(rule.check(order) for rule in rules)
    return failures
