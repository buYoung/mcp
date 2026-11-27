from typing import Optional


class Gauge:
    def level(self) -> int:
        return 1


# The receivers are declared as optional.
def safe(gauge: Optional[Gauge]) -> int:
    return gauge.level() if gauge else 0


def union(gauge: Gauge | None) -> int:
    return gauge.level() if gauge else 0
