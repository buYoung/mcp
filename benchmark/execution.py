"""Current benchmark selection and solver input, without a campaign launch path."""
from __future__ import annotations

from . import runner, settings
from .core import require


def answer_prompt(spec: dict, question: dict) -> str:
    prefix = runner.ANSWER_PROMPT.split("실행 제한은")[0]
    limits = spec["limits"]
    return (prefix + f"실행 제한은 {limits['elapsed_seconds']}초, 실제 탐색 호출 {limits['exploration_calls']}회, "
            f"누적 입력+출력 토큰 {limits['total_tokens']}개다.\n\n" + question["prompt"])


def catalog() -> dict:
    return {"profiles": settings.profiles(), "route_targets": settings.targets()}


def prepare_plan(profile: str, selected: list[str], emit=print) -> dict:
    from . import route_contract as routes, route_prepare
    require(profile == routes.PROFILE, "지원하는 벤치마크는 grafana-routes뿐입니다")
    require(len(selected) == len(set(selected)) == 6 and set(selected) == set(routes.ARM_IDS),
            "고정된 6개 비교군을 모두 준비합니다")
    return route_prepare.prepare_all(emit)
