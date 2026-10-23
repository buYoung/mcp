"""Blind check of how coding agents use the sense lookup CLI: do they split an identifier into its words and pass
each as a term (`appVersion` -> `synonyms app version`), and do they use the related words it prints?

Usage (from the repository root):
  python3 apps/codemap-search/experiments/related-terms-poc/sense-eval/agent_usage_eval.py \\
      [--models claude-bridge/claude-haiku-5-5,openai-codex/gpt-6-luna] [--usage agent-usage/usage-short.md ...] \\
      [--count 12] [--identifiers appVersion,UsePasswordManager] [--jobs 4] [--output-dir DIR]
  python3 .../agent_usage_eval.py --rescore DIR

Each run is one `pi -p --mode json` session with the bash tool only, in an empty directory. The usage text is
appended to the system prompt and the CLI is reachable as `synonyms` (a wrapper around `sense_lookup.py`). The task
only asks for grep keywords for an identifier, so nothing tells the agent that its CLI use is measured.

Identifiers are `appVersion` and evaluation-set identifiers of two or more words (`syntax_parts.subparts`, the rule
the lookup splits by), shown with their code lines. Per run:

  called      the agent ran `synonyms` (or `sense_lookup.py`)
  coverage    share of the identifier's words, accessor prefixes and numbers aside, passed as separate terms
              (singular and plural count as the same word)
  split       coverage is complete
  raw/phrase  the identifier as one term / its words as one quoted term
  from_tool   share of answer keywords found in the CLI output (identifier words aside)

The usage text's own token cost is the prompt-token difference between a one-line request with and without it.
Raw session events go to DIR/runs/, scores to DIR/results.json; DIR defaults to
.codemap/poc/related-terms/agent-usage/<time> (outside git). Agents run in empty directories under the system
temporary directory, away from the repository.
"""

import argparse
import concurrent.futures
import json
import os
import random
import re
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
POC = HERE.parent
REPO = POC.parents[3]
sys.path.insert(0, str(POC))
from syntax_parts import subparts  # noqa: E402

DEFAULT_MODELS = "claude-bridge/claude-haiku-5-5,openai-codex/gpt-6-luna"
DEFAULT_USAGE = HERE / "agent-usage/usage-short.md"
SEED = 20261017
COMMAND = "synonyms"
# Accessor and glue words an agent may fairly leave out of the terms.
OPTIONAL_WORDS = frozenset("get set is has use to of on in by for with the a an new do can should".split())
VALUE_OPTIONS = frozenset("--format --output --sense-dataset --dataset --terms-file --rtpoc3".split())
SEPARATORS = frozenset("| || && ; & ( )".split())
# A redirection ends the terms of a call (`synonyms a b > out`).
REDIRECTIONS = frozenset("> >> < &> >& 2>".split())
PI_FLAGS = ["-p", "--mode", "json", "--tools", "bash", "--no-session", "-nc", "-ns", "--no-mcp", "-np", "--no-themes"]
TASK = """I'm going to grep a large codebase for code about the same concept as `{identifier}`, including code that names it differently.
{context}Reply with up to 10 grep keywords, one per line, nothing else."""


def pick_identifiers(count, chosen):
    if chosen:
        return [{"identifier": name} for name in chosen]
    items = json.loads((HERE / "eval-set.json").read_text())["items"]
    seen, pool = {"appVersion"}, []
    for item in items:
        name = item.get("identifier") or ""
        if len(subparts(name)) >= 2 and name not in seen:
            seen.add(name)
            pool.append(item)
    picked = random.Random(SEED).sample(pool, max(0, count - 1))
    return [{"identifier": "appVersion"}] + [{key: item[key] for key in ("identifier", "language", "file", "code")} for item in picked]


def task_prompt(case):
    context = ""
    if case.get("code"):
        code = "\n".join(line.split(":", 1)[1] if ":" in line else line for line in case["code"])
        context = f"It appears in {case['file']} ({case['language']}):\n```\n{code}\n```\n"
    return TASK.format(identifier=case["identifier"], context=context)


def write_wrapper(bin_dir):
    bin_dir.mkdir(parents=True, exist_ok=True)
    wrapper = bin_dir / COMMAND
    wrapper.write_text(f'#!/bin/sh\nexec python3 "{POC / "sense_lookup.py"}" "$@"\n')
    wrapper.chmod(0o755)


def run_pi(model, prompt, usage, workdir, env, thinking, timeout_s, log_path):
    command = ["pi", *PI_FLAGS, "--model", model]
    if usage:
        command += ["--append-system-prompt", str(usage)]
    if thinking:
        command += ["--thinking", thinking]
    command += ["--", prompt]
    workdir.mkdir(parents=True, exist_ok=True)
    try:
        completed = subprocess.run(command, cwd=workdir, env=env, stdin=subprocess.DEVNULL, capture_output=True,
                                   text=True, timeout=timeout_s)
        log_path.write_text(completed.stdout)
        return completed.returncode, completed.stderr[-2000:]
    except subprocess.TimeoutExpired as error:
        log_path.write_text(error.stdout.decode() if isinstance(error.stdout, bytes) else (error.stdout or ""))
        return "timeout", ""


def read_session(log_path):
    """Commands with their outputs and exit codes, the final answer, token use and the system prompt text."""
    calls, answer, prompt_tokens, output_tokens, first_prompt, system = {}, "", 0, 0, None, ""
    for line in log_path.read_text().splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        kind = event.get("type")
        if kind == "tool_execution_start" and event.get("toolName") == "bash":
            calls[event["toolCallId"]] = {"command": event["args"].get("command", ""), "output": "", "exit_code": None}
        elif kind == "tool_execution_end" and event.get("toolCallId") in calls:
            result = event.get("result") or {}
            calls[event["toolCallId"]]["output"] = "".join(c.get("text", "") for c in result.get("content", []))
            calls[event["toolCallId"]]["exit_code"] = (result.get("structuredContent") or {}).get("exit_code")
        elif kind == "message_start" and event["message"].get("role") == "system":
            sections = event["message"].get("sections") or {}
            system = event["message"].get("content", "") + "\n".join(str(v) for v in sections.values())
        elif kind == "message_end" and event["message"].get("role") == "assistant":
            message = event["message"]
            usage = message.get("usage") or {}
            prompt = usage.get("input", 0) + usage.get("cacheRead", 0) + usage.get("cacheWrite", 0)
            first_prompt = prompt if first_prompt is None else first_prompt
            prompt_tokens += prompt
            output_tokens += usage.get("output", 0)
            text = "".join(part.get("text", "") for part in message.get("content", []) if part.get("type") == "text")
            if text.strip():
                answer = text
    return list(calls.values()), answer, prompt_tokens, output_tokens, first_prompt, system


def lookup_terms(command):
    """Terms passed to the CLI in one bash command, per call."""
    lexer = shlex.shlex(command.replace("\n", " ; "), posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        tokens = list(lexer)
    except ValueError:
        tokens = command.split()
    found, segment = [], []
    for token in tokens + [";"]:
        if token not in SEPARATORS:
            segment.append(token)
            continue
        if any(t in REDIRECTIONS for t in segment):
            segment = segment[:min(i for i, t in enumerate(segment) if t in REDIRECTIONS)]
        starts = [i for i, t in enumerate(segment) if t == COMMAND or t.endswith("/" + COMMAND) or t.endswith("sense_lookup.py")]
        if starts:
            terms, skip = [], False
            for token_ in segment[starts[0] + 1:]:
                if skip:
                    skip = False
                elif token_.startswith("-"):
                    skip = token_.split("=", 1)[0] in VALUE_OPTIONS and "=" not in token_
                else:
                    terms.append(token_)
            found.append(terms)
        segment = []
    return found


def keywords(answer):
    lines = (re.sub(r"^\s*(?:[-*•]|\d+[.)])\s*", "", line).strip().strip("`'\"") for line in answer.splitlines())
    return [line for line in lines if line and not line.startswith("```")]


def singular(word):
    """`swaps` and `swap` count as the same word."""
    word = word.lower()
    return word[:-1] if len(word) > 3 and word.endswith("s") and not word.endswith("ss") else word


def score(case, model, usage, log_path, status):
    identifier = case["identifier"]
    words = list(subparts(identifier))
    required = [w for w in words if w not in OPTIONAL_WORDS and not w.isdigit()] or words
    calls, answer, prompt_tokens, output_tokens, first_prompt, system = read_session(log_path)
    lookups = [(call, terms) for call in calls for terms in lookup_terms(call["command"])]
    terms = [t for _, terms in lookups for t in terms]
    lowered = {t.lower() for t in terms}
    passed = {singular(t) for t in terms}
    covered = [w for w in required if singular(w) in passed]
    own = set(words) | {identifier.lower(), " ".join(words)}
    tool_text = "\n".join(call["output"] for call, _ in lookups).lower()
    answer_words = keywords(answer)
    counted = [k for k in answer_words if k.lower() not in own]
    from_tool = [k for k in counted if re.search(rf"(?<![\w-]){re.escape(k.lower())}(?![\w-])", tool_text)]
    return {
        "model": model, "usage": usage, "identifier": identifier, "words": words, "required": required,
        "status": status, "usage_loaded": bool(usage) and usage_text(usage).splitlines()[0][:40] in system,
        "called": bool(lookups), "calls": len(lookups), "terms": terms,
        "coverage": round(len(covered) / len(required), 3), "split": len(covered) == len(required),
        "raw": identifier.lower() in lowered, "phrase": " ".join(words) in lowered or " ".join(required) in lowered,
        "extra_terms": [t for t in terms if t.lower() not in own],
        "exit_codes": [call["exit_code"] for call, _ in lookups],
        "other_commands": [call["command"] for call in calls if not lookup_terms(call["command"])],
        "answer": answer_words, "from_tool": round(len(from_tool) / len(counted), 3) if counted else None,
        "tool_output_chars": len(tool_text), "prompt_tokens": prompt_tokens, "output_tokens": output_tokens,
        "first_prompt_tokens": first_prompt,
    }


def usage_text(usage):
    return Path(usage).read_text() if usage else ""


def summarize(rows):
    def mean(values):
        values = [v for v in values if v is not None]
        return round(sum(values) / len(values), 3) if values else None
    groups = {}
    for row in rows:
        groups.setdefault((row["model"], row["usage"]), []).append(row)
    return [{"model": model, "usage": Path(usage).name if usage else None, "runs": len(group),
             "called": mean([r["called"] for r in group]), "split": mean([r["split"] for r in group]),
             "coverage": mean([r["coverage"] for r in group]), "raw": mean([r["raw"] for r in group]),
             "phrase": mean([r["phrase"] for r in group]), "from_tool": mean([r["from_tool"] for r in group]),
             "calls": mean([r["calls"] for r in group]), "tool_output_chars": mean([r["tool_output_chars"] for r in group]),
             "prompt_tokens": mean([r["prompt_tokens"] for r in group]),
             "failed": sum(r["status"] != 0 for r in group), "usage_not_loaded": sum(bool(usage) and not r["usage_loaded"] for r in group)}
            for (model, usage), group in sorted(groups.items())]


def print_summary(summary, usage_tokens):
    print("model | usage | runs | called | split | coverage | raw | phrase | from_tool | calls | tool chars | prompt tokens | usage tokens | failed")
    for row in summary:
        tokens = usage_tokens.get(f"{row['model']}|{row['usage']}")
        print(" | ".join(str(v) for v in (row["model"], row["usage"], row["runs"], row["called"], row["split"], row["coverage"],
                                          row["raw"], row["phrase"], row["from_tool"], row["calls"], row["tool_output_chars"],
                                          row["prompt_tokens"], tokens, row["failed"] + row["usage_not_loaded"])))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--models", default=DEFAULT_MODELS, help="comma-separated pi model ids")
    parser.add_argument("--usage", nargs="+", type=Path, default=[DEFAULT_USAGE], help="usage text files to compare")
    parser.add_argument("--count", type=int, default=12, help="identifiers, appVersion included")
    parser.add_argument("--identifiers", help="comma-separated identifiers instead of the evaluation-set sample")
    parser.add_argument("--thinking", help="pi thinking level (default: the model's)")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=600, help="seconds per run")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--rescore", type=Path, help="score the runs of an earlier output directory again")
    arguments = parser.parse_args()

    if arguments.rescore:
        plan = json.loads((arguments.rescore / "plan.json").read_text())
        rows = [score(item["case"], item["model"], item["usage"], arguments.rescore / item["log"], item["status"]) for item in plan["runs"]]
        out, usage_tokens = arguments.rescore, plan.get("usage_tokens", {})
    else:
        out = (arguments.output_dir or REPO / ".codemap/poc/related-terms/agent-usage" / time.strftime("%Y%m%d-%H%M%S")).resolve()
        write_wrapper(out / "bin")
        env = {**os.environ, "PATH": f"{out / 'bin'}{os.pathsep}{os.environ['PATH']}", "PYTHONDONTWRITEBYTECODE": "1"}
        models = [m.strip() for m in arguments.models.split(",") if m.strip()]
        usages = [str(u.resolve()) for u in arguments.usage]
        cases = pick_identifiers(arguments.count, [i.strip() for i in arguments.identifiers.split(",")] if arguments.identifiers else None)
        jobs = []
        for model in models:
            slug = re.sub(r"[^A-Za-z0-9.-]+", "_", model)
            for usage in [None, *usages]:
                name = f"{slug}/probe-{Path(usage).stem if usage else 'none'}"
                jobs.append({"probe": True, "model": model, "usage": usage, "case": None, "log": f"runs/{name}.jsonl", "prompt": "Reply with OK."})
            for usage in usages:
                for number, case in enumerate(cases, 1):
                    name = f"{slug}/{Path(usage).stem}/{number:02d}-{re.sub(r'[^A-Za-z0-9]+', '_', case['identifier'])}"
                    jobs.append({"probe": False, "model": model, "usage": usage, "case": case, "log": f"runs/{name}.jsonl", "prompt": task_prompt(case)})

        def execute(job):
            log_path = out / job["log"]
            log_path.parent.mkdir(parents=True, exist_ok=True)
            status, stderr = run_pi(job["model"], job["prompt"], job["usage"], work_root / job["log"].removesuffix(".jsonl"),
                                    env, arguments.thinking, arguments.timeout, log_path)
            job["status"] = status
            if status != 0:
                (log_path.with_suffix(".stderr")).write_text(stderr)
            print(f"{'ok' if status == 0 else status}: {job['log']}", file=sys.stderr, flush=True)
            return job

        work_root = Path(tempfile.mkdtemp(prefix="agent-usage-"))
        with concurrent.futures.ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
            done = list(pool.map(execute, jobs))
        probes = {(j["model"], j["usage"]): read_session(out / j["log"])[4] for j in done if j["probe"]}
        usage_tokens = {f"{model}|{Path(usage).name}": (probes.get((model, usage)) or 0) - (probes.get((model, None)) or 0)
                        for model in models for usage in usages if probes.get((model, usage)) and probes.get((model, None))}
        runs = [{key: j[key] for key in ("model", "usage", "case", "log", "status")} for j in done if not j["probe"]]
        (out / "plan.json").write_text(json.dumps({"models": models, "usages": usages, "usage_tokens": usage_tokens, "runs": runs},
                                                  ensure_ascii=False, indent=1))
        rows = [score(r["case"], r["model"], r["usage"], out / r["log"], r["status"]) for r in runs]
    summary = summarize(rows)
    (out / "results.json").write_text(json.dumps({"usage_tokens": usage_tokens, "summary": summary, "runs": rows},
                                                 ensure_ascii=False, indent=1))
    print_summary(summary, usage_tokens)
    print(f"-> {out / 'results.json'}")


if __name__ == "__main__":
    main()
