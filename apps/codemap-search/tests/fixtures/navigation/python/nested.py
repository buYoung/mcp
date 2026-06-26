from dataclasses import dataclass
from decimal import Decimal


@dataclass(frozen=True)
class OrderDto:
    total: Decimal


def traced(name: str):
    def decorate(fn):
        return fn

    return decorate


class CheckoutService:
    def __init__(self, repo, inventory, policy):
        self.repo = repo
        self.inventory = inventory
        self.policy = policy

    @traced("checkout.submit")
    def submit(self, lines: list[dict[str, Decimal]]) -> OrderDto:
        def line_total(line: dict[str, Decimal]) -> Decimal:
            return line["price"] * line["quantity"]

        total = sum(line_total(line) for line in lines)
        dto = OrderDto(total=total)

        self.policy.validate(dto)
        self.inventory.reserve(lines)
        self.repo.persist(dto)
        return dto
