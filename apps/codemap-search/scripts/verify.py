#!/usr/bin/env python3
"""현재 코드로 로컬·공개 저장소 검증을 실행하고 결과를 한곳에 요약한다."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import time

APP = Path(__file__).resolve().parents[1]
SCRIPTS = APP / "scripts"
DATA = APP / "validation"
RUST_GO = {"rust", "go"}


def repeated(option, values):
    return [part for value in dict.fromkeys(values or []) for part in (option, value)]


def parse_args():
    manifest = json.loads((DATA / "development-languages.json").read_text())
    parser = argparse.ArgumentParser(prog="verify", description=__doc__)
    parser.add_argument("suite", nargs="?", choices=("quick", "test", "public"), default="quick",
        help="quick: 작은 언어별 검증(기본), test: cargo check/test, public: 공개 저장소 검증")
    parser.add_argument("--language", action="append", choices=[row["language"] for row in manifest["languages"]],
        help="대상 언어. 반복 지정 가능, 생략하면 전체")
    parser.add_argument("--profile", action="append", choices=("default", "structural"),
        help="검증 설정. 생략하면 두 설정 모두")
    parser.add_argument("--repository", action="append", help="public 전용 owner/repository. 반복 지정 가능")
    parser.add_argument("--binary", type=Path, help="이 바이너리로 검사하며 자동 빌드는 생략")
    parser.add_argument("--cache", type=Path,
        default=Path(os.environ.get("CODEMAP_VALIDATION_CACHE") or Path.home() / ".cache/codemap-public-validation"),
        help="저장소·결과 캐시. CODEMAP_VALIDATION_CACHE로 기본값 지정 가능")
    parser.add_argument("--jobs", type=int, choices=range(1, 5), help="public 준비·검증 동시 작업 수(기본 2)")
    parser.add_argument("--skip-prepare", action="store_true", help="public에서 기존 저장소 측정·파서 캐시 사용")
    parser.add_argument("--dry-run", action="store_true", help="실행 명령만 표시. 빌드·다운로드·파일 생성 없음")
    args = parser.parse_args()
    if args.suite == "test" and (args.language or args.profile or args.binary):
        parser.error("test에는 --language, --profile, --binary를 사용할 수 없습니다")
    if args.suite != "public" and (args.repository or args.jobs or args.skip_prepare):
        parser.error("--repository, --jobs, --skip-prepare는 public 전용입니다")
    selected = [row for row in manifest["languages"] if not args.language or row["language"] in args.language]
    pairs = [(row["language"], slug) for row in selected for slug in row.get("selected_candidates", row["candidates"])]
    if args.repository:
        unmatched = set(args.repository) - {slug for _, slug in pairs}
        if unmatched:
            parser.error(f"선택한 언어에 없는 저장소: {', '.join(sorted(unmatched))}")
        pairs = [(language, slug) for language, slug in pairs if slug in args.repository]
    args.languages = list(dict.fromkeys(language for language, _ in pairs))
    args.pairs = pairs
    args.cache = args.cache.expanduser().resolve()
    if args.binary:
        candidate = shutil.which(str(args.binary)) if not args.binary.exists() else str(args.binary)
        args.binary = Path(candidate or args.binary).expanduser().resolve()
        if not args.dry_run and (not args.binary.is_file() or not os.access(args.binary, os.X_OK)):
            parser.error(f"실행할 바이너리를 찾을 수 없습니다: {args.binary}")
    return args


def required_tools(args):
    required = set()
    if args.suite == "test" or not args.binary:
        required.add("cargo")
    languages = set(args.languages)
    if args.suite != "test":
        required.add("git")
    if args.suite == "public":
        required.add("tokei")
        if languages & RUST_GO:
            required.update(("go", "rust-analyzer"))
        if languages & {"typescript", "javascript", "vue", "astro", "svelte"}:
            required.add("node")
        if "swift" in languages:
            required.add("swiftc")
        native = {"dart", "scala", "groovy"}
        if languages & native and not args.skip_prepare:
            # The existing native helper prepares these three parsers together.
            required.update(("java", "javac", "dart"))
        if languages & {"scala", "groovy"}:
            required.add("java")
        if languages - RUST_GO - native - {"python", "swift", "typescript", "javascript", "vue", "astro", "svelte"}:
            required.add("ctags")
    missing = sorted(name for name in required if shutil.which(name) is None)
    if missing:
        raise RuntimeError(f"필요한 실행 파일이 PATH에 없습니다: {', '.join(missing)}")


def run_command(command, log_path, on_message=None):
    command = [str(part) for part in command]
    print(f"\n$ {shlex.join(command)}", flush=True)
    with log_path.open("w") as log, subprocess.Popen(command, cwd=APP, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, text=True, errors="replace") as process:
        for line in process.stdout:
            log.write(line)
            log.flush()
            if on_message:
                on_message(line)
            else:
                print(line, end="", flush=True)
        return process.wait()


def build_binary(output):
    executable = None

    def cargo_message(line):
        nonlocal executable
        try:
            message = json.loads(line)
        except ValueError:
            print(line, end="", flush=True)
            return
        if message.get("reason") == "compiler-artifact" and message.get("target", {}).get("name") == "codemap-search":
            executable = message.get("executable") or executable

    command = ["cargo", "build", "--release", "--locked", "--bin", "codemap-search", "--message-format=json-render-diagnostics"]
    if run_command(command, output / "build.log", cargo_message):
        raise RuntimeError(f"빌드 실패: {output / 'build.log'}")
    if not executable:
        raise RuntimeError("Cargo 출력에서 codemap-search 실행 파일을 찾지 못했습니다")
    return Path(executable).resolve()


def command_plan(args, binary, output):
    steps = []

    def add(name, command, summary=None, depends=()):
        steps.append({"name": name, "command": [str(part) for part in command],
            "summary": str(summary) if summary else None, "depends": list(depends)})

    if args.suite == "test":
        add("cargo-check", ["cargo", "check", "--locked"])
        add("cargo-test", ["cargo", "test", "--locked"], depends=("cargo-check",))
        return steps
    profiles = repeated("--profile", args.profile)
    jobs = ["--jobs", str(args.jobs or 2)]
    languages = set(args.languages)
    public = [sys.executable, "-B", SCRIPTS / "public_validation.py", "--cache", args.cache, "--binary", binary]
    if languages & RUST_GO:
        run_id = output.name + "-rust-go"
        if args.suite == "quick":
            add("rust-go", public + ["regressions", "--run-id", run_id] + profiles
                + repeated("--language", sorted(languages & RUST_GO)), args.cache / "runs" / run_id / "summary.json")
        else:
            specs = json.loads((DATA / "repositories.json").read_text())["repositories"]
            repos = repeated("--repo", [row["name"] for row in specs
                if (row["language"].lower(), row["url"].removeprefix("https://github.com/").removesuffix(".git")) in args.pairs])
            dependencies = []
            if not args.skip_prepare:
                add("prepare-rust-go", public + ["prepare"] + repos + jobs)
                dependencies.append("prepare-rust-go")
            add("public-rust-go", public + ["run", "--run-id", run_id] + repos + profiles
                + repeated("--language", sorted(languages & RUST_GO)) + jobs,
                args.cache / "runs" / run_id / "summary.json", dependencies)
    expanded = [language for language in args.languages if language not in RUST_GO]
    if expanded:
        language_options = repeated("--language", expanded)
        if args.suite == "quick":
            add("languages", [sys.executable, "-B", SCRIPTS / "probe_development_languages.py", "--binary", binary,
                "--output", output / "languages"] + language_options + profiles, output / "languages/summary.json")
        else:
            repositories = [slug for language, slug in args.pairs if language in expanded]
            dependencies = []
            if not args.skip_prepare:
                add("prepare-languages", [sys.executable, "-B", SCRIPTS / "qualify_development_languages.py", "--cache", args.cache]
                    + language_options + repeated("--candidate", repositories) + jobs)
                dependencies.append("prepare-languages")
                if languages & {"dart", "scala", "groovy"}:
                    add("prepare-oracles", [sys.executable, "-B", SCRIPTS / "prepare_validation_oracles.py", "--cache", args.cache],
                        depends=("prepare-languages",))
                    dependencies.append("prepare-oracles")
            run_id = output.name + "-languages"
            add("public-languages", [sys.executable, "-B", SCRIPTS / "validate_development_languages.py", "--cache", args.cache,
                "--binary", binary, "--run-id", run_id] + language_options + repeated("--repository", repositories) + profiles + jobs,
                args.cache / "runs" / run_id / "summary.json", dependencies)
    return steps


def collect_results(path):
    data = json.loads(path.read_text())
    records = data.get("results", []) + data.get("regressions", [])
    checks = [check for record in records for check in record.get("checks", [])]
    errors = list(data.get("errors", [])) + [record["error"] for record in records if record.get("error")]
    if not checks and not errors:
        raise RuntimeError(f"검사 항목이 없는 결과입니다: {path}")
    totals = Counter(check["status"] for check in checks)
    failures = [{"target": record.get("language", record.get("repository")), "profile": record.get("profile"),
        "id": check["id"], "status": check["status"]}
        for record in records for check in record.get("checks", []) if check["status"] != "pass"]
    errors.extend(f"추적 파일 변경: {record.get('repository')}" for record in records if record.get("tracked_changes"))
    return {"counts": {status: totals[status] for status in ("pass", "fail", "unverified")}, "errors": errors, "failures": failures}


def save_summary(output, summary):
    (output / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")


def main():
    args = parse_args()
    run_id = "verify-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%f") + f"-{os.getpid()}"
    output = args.cache / "runs" / run_id
    if args.dry_run:
        print(f"작업 디렉터리: {APP}")
        if args.suite != "test" and not args.binary:
            print("$ cargo build --release --locked --bin codemap-search --message-format=json-render-diagnostics")
        for step in command_plan(args, args.binary or "<Cargo 빌드 결과>", output):
            print(f"[{step['name']}] $ {shlex.join(step['command'])}")
        return 0
    required_tools(args)
    output.mkdir(parents=True, exist_ok=False)
    summary = {"suite": args.suite, "started_utc": datetime.now(timezone.utc).isoformat(), "languages": args.languages,
        "profiles": list(dict.fromkeys(args.profile or ("default", "structural"))), "stages": []}
    save_summary(output, summary)
    try:
        binary = None if args.suite == "test" else args.binary or build_binary(output)
        if binary:
            source_binary = binary
            binary = output / ("tested-codemap-search.exe" if os.name == "nt" else "tested-codemap-search")
            shutil.copy2(source_binary, binary)
            summary.update(binary=str(binary), binary_source=str(source_binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
        outcomes = {}
        for step in command_plan(args, binary, output):
            record = {**step, "log": str(output / (step["name"] + ".log")), "status": "running", "exit_code": None}
            summary["stages"].append(record)
            if any(outcomes[name] != 0 for name in step["depends"]):
                record.update(status="skipped", exit_code=2)
            else:
                started = time.monotonic()
                code = run_command(step["command"], Path(record["log"]))
                record.update(exit_code=code, elapsed_seconds=round(time.monotonic() - started, 3), status="passed" if code == 0 else "failed")
                if step["summary"]:
                    try:
                        record.update(collect_results(Path(step["summary"])))
                        if record["errors"] or record["counts"]["fail"] or record["counts"]["unverified"]:
                            record.update(status="failed", exit_code=code or 1)
                    except (OSError, ValueError, RuntimeError) as error:
                        record.update(status="failed", exit_code=code or 2, errors=[str(error)])
            outcomes[step["name"]] = record["exit_code"]
            save_summary(output, summary)
        summary["exit_code"] = int(any(code != 0 for code in outcomes.values()))
    except KeyboardInterrupt:
        summary.update(exit_code=130, error="사용자가 실행을 중단했습니다")
    except (OSError, ValueError, RuntimeError) as error:
        summary.update(exit_code=2, error=str(error))
        print(f"검증 실행 오류: {error}", file=sys.stderr)
    finally:
        if summary["stages"] and summary["stages"][-1]["status"] == "running":
            summary["stages"][-1].update(status="interrupted" if summary.get("exit_code") == 130 else "failed", exit_code=summary.get("exit_code", 2))
        summary["finished_utc"] = datetime.now(timezone.utc).isoformat()
        save_summary(output, summary)
    totals = Counter()
    for stage in summary["stages"]:
        totals.update(stage.get("counts", {}))
    print(f"\n검증 {'완료' if summary['exit_code'] == 0 else '실패 또는 미완료'}")
    if args.suite == "test":
        passed = sum(stage["status"] == "passed" for stage in summary["stages"])
        print(f"기존 검사 명령: {passed}/{len(summary['stages'])}개 통과 (테스트 개수는 cargo 로그 참조)")
    else:
        print(f"통과 {totals['pass']}, 실패 {totals['fail']}, 보류 {totals['unverified']}")
    for stage in summary["stages"]:
        print(f"  {stage['name']}: {stage['status']}")
        for failure in stage.get("failures", [])[:12]:
            print(f"    {failure['status']}: {failure['target']}/{failure['profile']} {failure['id']}")
        if len(stage.get("failures", [])) > 12:
            print(f"    나머지 {len(stage['failures']) - 12}개는 결과 JSON 참조")
    print(f"결과: {output / 'summary.json'}")
    return summary["exit_code"]


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(f"검증 실행 오류: {error}", file=sys.stderr)
        sys.exit(2)
