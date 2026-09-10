"""Prepare Grafana, current product and index; never execute an LLM."""
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


def prepare_grafana(source: Path, emit=print) -> None:
    if source.exists():
        validate_grafana(source)
        emit(f"기존 Grafana 복제본을 재사용합니다: {source}")
        return

    source.parent.mkdir(parents=True, exist_ok=True)
    repository_URL = f"https://github.com/{SPEC['repository']}.git"
    emit(f"Grafana 고정 커밋을 가져옵니다: {SPEC['source_commit']}")
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
    import argparse
    from . import resources, settings
    parser = argparse.ArgumentParser(description="Grafana · 현재 제품 release 빌드 · 색인 준비 (모델 실행 없음)")
    parser.parse_args()
    try:
        observed = resources.environment()
        print(f"Codex: {observed['codex_version']} (버전 고정 없음)", flush=True)
        config = settings.load_config()
        prepared = resources.prepare_targets(config, ["current"])
        from .v2.core import write_json
        write_json(settings.CACHE / "ready.json", {"environment": observed, **prepared})
        print("벤치 준비 완료. pnpm bench:start에서 유형과 비교 대상을 선택하세요.")
        return 0
    except (ContractError, OSError, subprocess.SubprocessError) as exc:
        print(f"벤치 준비 실패: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("준비를 취소했습니다. 완료된 캐시와 미완료 로그를 보존합니다.", file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
