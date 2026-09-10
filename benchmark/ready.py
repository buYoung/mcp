"""Prepare the pinned Grafana checkout without changing the benchmark harness."""
from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

from .v2.core import SPEC, ContractError, command, require

GIT_TIMEOUT_SECONDS = 1800


def validate_grafana(source: Path) -> None:
    require((source / ".git").exists(), f"Git 복제본이 아닌 기존 경로입니다: {source}")
    repository_root = Path(command(["git", "rev-parse", "--show-toplevel"], source).strip())
    require(repository_root.resolve() == source.resolve(), f"독립 Git 복제본이 아닙니다: {source}")
    commit = command(["git", "rev-parse", "HEAD"], source).strip()
    require(commit == SPEC["source_commit"],
            f"Grafana 커밋이 다릅니다: {commit} (필요: {SPEC['source_commit']}). 기존 경로를 보존한 채 중단합니다: {source}")
    require(not command(["git", "status", "--porcelain", "--untracked-files=no"], source).strip(),
            f"Grafana 추적 파일에 로컬 변경이 있어 중단합니다. 변경을 보존하거나 정리한 뒤 다시 실행하세요: {source}")


def prepare_grafana(source: Path) -> None:
    if source.exists():
        validate_grafana(source)
        print(f"기존 Grafana 복제본을 재사용합니다: {source}")
        return

    source.parent.mkdir(parents=True, exist_ok=True)
    repository_URL = f"https://github.com/{SPEC['repository']}.git"
    print(f"Grafana 고정 커밋을 가져옵니다: {SPEC['source_commit']}", flush=True)
    with tempfile.TemporaryDirectory(prefix=".grafana-ready-", dir=source.parent) as folder:
        checkout = Path(folder) / "source"
        subprocess.run(["git", "init", "--quiet", str(checkout)], check=True, timeout=GIT_TIMEOUT_SECONDS)
        subprocess.run(["git", "remote", "add", "origin", repository_URL], cwd=checkout,
                       check=True, timeout=GIT_TIMEOUT_SECONDS)
        subprocess.run(["git", "fetch", "--progress", "--no-tags", "--depth=1", "origin", SPEC["source_commit"]],
                       cwd=checkout, check=True, timeout=GIT_TIMEOUT_SECONDS)
        subprocess.run(["git", "checkout", "--quiet", "--detach", SPEC["source_commit"]], cwd=checkout,
                       check=True, timeout=GIT_TIMEOUT_SECONDS)
        validate_grafana(checkout)
        require(not source.exists(), f"준비 중 대상 경로가 생성되어 중단합니다: {source}")
        checkout.rename(source)


def main() -> int:
    source = Path(__file__).resolve().parent / "v2" / "artifacts" / "grafana"
    try:
        prepare_grafana(source)
        print(f"Grafana 준비 완료: {source}\n커밋: {SPEC['source_commit']}")
        return 0
    except (ContractError, OSError, subprocess.SubprocessError) as exc:
        print(f"Grafana 준비 실패: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
