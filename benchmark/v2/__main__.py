"""python3 -m benchmark.v2 {collect,build-dataset,verify,run,grade,report}."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

from .core import ContractError, SPEC, digest, read_json, write_json


def main():
    parser = argparse.ArgumentParser(description="codemap-search V2 재현 가능한 A/B 하네스")
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("collect", help="공개 PR 후보와 원문 출처 수집")
    collect.add_argument("--output", type=Path, required=True)
    collect.add_argument("--max-candidates", type=int, default=400)
    build = commands.add_parser("build-dataset", help="검토한 질문·정답 계약을 고정 코드와 연결")
    for name in ["selection", "collection", "source", "output"]:
        build.add_argument("--" + name, type=Path, required=True)
    verify = commands.add_parser("verify", help="V2 자동검사 및 선택적 실제 런타임 검사")
    verify.add_argument("--dataset", type=Path)
    verify.add_argument("--source", type=Path)
    verify.add_argument("--runtime-probe", type=Path, help="새 런타임 검사 결과 디렉터리")
    verify.add_argument("--product", type=Path)
    verify.add_argument("--build-product", type=Path, help="고정 소스를 새 디렉터리에 추출해 빌드하고 출처 기록")
    verify.add_argument("--product-build", type=Path, help="기존 고정 소스 빌드 기록 재검증")
    verify.add_argument("--runtime-evidence", type=Path, help="같은 실행 옵션의 기존 런타임 원시 로그 재판독")
    run = commands.add_parser("run", help="예정 분모를 먼저 기록한 뒤 고정 모델 실행")
    for name in ["dataset", "source", "product", "experiment"]:
        run.add_argument("--" + name, type=Path, required=True)
    run.add_argument("--phase", choices=SPEC["phases"], required=True)
    run.add_argument("--preparation-experiment", type=Path)
    for name in ["grade", "report"]:
        command = commands.add_parser(name)
        command.add_argument("--dataset", type=Path, required=True)
        command.add_argument("--experiment", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "collect":
            from .dataset import collect
            result = collect(args.output, args.max_candidates)
            print(f"후보 {result['reviewed_candidates']}개 기록")
        elif args.command == "build-dataset":
            from .dataset import build_dataset
            result = build_dataset(args.selection, args.collection, args.source, args.output)
            print(f"고정 문제 {len(result['questions'])}개 기록")
        elif args.command == "verify":
            from .checks import verify_offline, verify_runtime
            from .runner import harness_digest
            result = verify_offline()
            print(result["output"], end="")
            result["harness_sha256"] = harness_digest()
            result["spec_sha256"] = digest(SPEC)
            if not result["passed"]:
                write_json(Path(__file__).parent / "artifacts" / "verification.json", result)
                return 1
            if args.dataset:
                from .dataset import validate_dataset
                validate_dataset(read_json(args.dataset), args.source)
                result["dataset_sha256"] = digest(read_json(args.dataset))
            result["runtime_probe_passed"] = False
            if args.product_build:
                from .runner import verify_product_build
                build = read_json(args.product_build)
                verify_product_build(build)
                result["product_build"] = build
                args.product = Path(build["binary"])
            if args.build_product:
                from .runner import build_product, PACKAGE_ROOT
                build = build_product(PACKAGE_ROOT, args.build_product.resolve())
                result["product_build"] = build
                args.product = Path(build["binary"])
            if args.runtime_probe:
                if not args.product:
                    parser.error("--runtime-probe requires --product")
                runtime = verify_runtime(args.runtime_probe, args.product)
                result["runtime_probe_passed"] = runtime["passed"]
                write_json(args.runtime_probe / "verification.json", runtime)
                result["runtime_probe"] = str(args.runtime_probe.resolve())
            if args.runtime_evidence:
                from .checks import reinspect_runtime
                runtime = reinspect_runtime(args.runtime_evidence)
                result["runtime_probe_passed"] = runtime["passed"]
                result["runtime_probe"] = str(args.runtime_evidence.resolve())
                write_json(Path(__file__).parent / "artifacts" / "runtime-reinspection.json", runtime)
            write_json(Path(__file__).parent / "artifacts" / "verification.json", result)
            return 0 if result["passed"] and (not (args.runtime_probe or args.runtime_evidence) or result["runtime_probe_passed"]) else 1
        elif args.command == "run":
            from .runner import run_experiment
            run_experiment(read_json(args.dataset), args.experiment.resolve(), args.source.resolve(), args.product.resolve(), args.phase, args.preparation_experiment)
        elif args.command == "grade":
            from .grading import grade
            result = grade(read_json(args.dataset), args.experiment.resolve())
            print(f"채점 묶음 {len(result['batches'])}개 처리")
        elif args.command == "report":
            from .reporting import report
            result = report(read_json(args.dataset), args.experiment.resolve())
            print(result["verdict"]["decision"])
        return 0
    except (ContractError, OSError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f"불완전: {exc}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
