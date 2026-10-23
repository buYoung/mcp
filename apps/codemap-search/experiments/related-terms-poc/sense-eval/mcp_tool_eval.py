"""Measure expected-field hits before lookup caps and in actual MCP tool responses.

This driver starts only the codemap-search server, never a model or an agent.
Pass a branch-built binary and the uncapped JSONL from related_terms_dump.
All configuration, indexes and reports are written under --output-dir.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import sys
import threading
import time

HERE = Path(__file__).resolve().parent
POC = HERE.parent


class McpSession:
    def __init__(self, binary, dataset, output, cwd=None, home=None, env_overrides=None,
                 is_watch_enabled=False, evidence_timeout_seconds=900):
        self.evidence_timeout_seconds = evidence_timeout_seconds
        output = output.resolve()
        output.mkdir(parents=True, exist_ok=True)
        self.home = (home or output / "home").resolve()
        self.home.mkdir(parents=True, exist_ok=True)
        self.cwd = (cwd or output / "workspace").resolve()
        self.cwd.mkdir(parents=True, exist_ok=True)
        self.scope = None
        config = ("[index]\npath = " + json.dumps(str(output / "index")) + "\n"
                  "[index.refresh]\nwatch = " + str(is_watch_enabled).lower() + "\nindex_staleness_ms = 1000\n"
                  "[related_terms]\nenabled = true\ndataset_path = " + json.dumps(str(dataset)) + "\n"
                  "[update]\nconfig_auto_update = false\n")
        (self.home / "config.toml").write_text(config)
        self.log = (output / "server.stderr.log").open("w")
        env = dict(os.environ, CODEMAP_HOME=str(self.home))
        env.pop("TYPESAFE_API_KEY", None)
        env.update(env_overrides or {})
        self.process = subprocess.Popen([str(binary), "mcp", "--no-call-log"], cwd=self.cwd, env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
                                        text=True, bufsize=1)
        self.messages = queue.Queue()
        self.reader = threading.Thread(target=self._read, daemon=True)
        self.reader.start()
        self.request_id = 0

    def _read(self):
        try:
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
        except Exception as error:
            self.messages.put(error)
        finally:
            self.messages.put(EOFError("MCP server closed stdout"))

    def call(self, method, params):
        self.request_id += 1
        self.process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.request_id,
                                            "method": method, "params": params}) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + 60
        while True:
            result = self.messages.get(timeout=max(0.01, deadline - time.monotonic()))
            if isinstance(result, Exception):
                raise result
            if result.get("id") == self.request_id:
                if "error" in result:
                    raise RuntimeError(result["error"])
                return result["result"]
            if time.monotonic() >= deadline:
                raise TimeoutError("MCP response timeout")

    def initialize(self):
        self.call("initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                 "clientInfo": {"name": "rterms-evaluation", "version": "1"}})
        tools = self.call("tools/list", {})["tools"]
        return next(tool for tool in tools if tool["name"] == "synonyms")

    def configure(self, scope, mode, score=None):
        """`score` is sent only when given. The specificity-ordering code that accepted it was removed,
        so the current server accepts only None (level ordering)."""
        params = {"scope": scope, "mode": mode}
        if score is not None:
            params["score"] = score
        self.call("related_terms/evaluate", params)
        self.scope = scope

    def lookup(self, terms):
        result = self.lookup_result(terms)
        return "\n".join(item["text"] for item in result["content"] if item["type"] == "text")

    def lookup_result(self, terms, should_wait_for_evidence=False):
        timeout_seconds = self.evidence_timeout_seconds if should_wait_for_evidence else 120
        deadline = time.monotonic() + timeout_seconds
        while True:
            result = self.call("tools/call", {"name": "synonyms", "arguments": {"terms": terms}})
            text = "\n".join(item["text"] for item in result["content"] if item["type"] == "text")
            if result.get("isError"):
                raise RuntimeError(text)
            metadata = result.get("_meta", {}).get("related_terms_eval") or {}
            is_current = (metadata.get("status") == "ready" and
                          metadata.get("index_generation") == metadata.get("evidence_generation") and
                          metadata.get("scope") == self.scope)
            if text.startswith("## ") and (not should_wait_for_evidence or is_current):
                return result
            if text.startswith("Related terms: unavailable:") or metadata.get("status") == "unavailable":
                raise RuntimeError(text)
            if time.monotonic() > deadline:
                raise TimeoutError(f"Readiness timeout after {timeout_seconds}s: {text[:250]}")
            time.sleep(0.25)

    def close(self):
        try:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                print("MCP shutdown exceeded 30s; terminating the owned server process", file=sys.stderr, flush=True)
                self.process.terminate()
                try:
                    self.process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=10)
        finally:
            self.reader.join(timeout=1)
            self.process.stdout.close()
            self.log.close()


def evaluate(arguments):
    output = arguments.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=True)
    dataset = arguments.dataset.resolve()
    before_sha256 = hashlib.sha256(dataset.read_bytes()).hexdigest()
    items = [item for item in json.loads(arguments.eval_set.read_text())["items"] if item["expected_fields"]]
    uncapped = {row["input"]: row for line in arguments.uncapped_jsonl.read_text().splitlines() if (row := json.loads(line))}
    missing = [item["word"] for item in items if item["word"] not in uncapped]
    if missing:
        raise ValueError(f"uncapped answers missing inputs: {missing}")
    client = McpSession(arguments.binary.resolve(), dataset, output)
    rows = []
    try:
        schema = client.initialize()
        with (output / "responses.jsonl").open("w") as responses:
            for item in items:
                text = client.lookup([item["word"]])
                before_fields = [sense["field"] for sense in uncapped[item["word"]]["senses"]]
                after_fields = re.findall(r"^### .* \[([^\]]+)\] — ", text, re.MULTILINE)
                expected = set(item["expected_fields"])
                row = {"id": item["id"], "word": item["word"], "expected_fields": item["expected_fields"],
                       "before_fields": before_fields, "after_fields": after_fields,
                       "before_hit": bool(expected.intersection(before_fields)),
                       "after_hit": bool(expected.intersection(after_fields)),
                       "output_bytes": len(text.encode()), "output_characters": len(text)}
                rows.append(row)
                responses.write(json.dumps({**row, "text": text}, ensure_ascii=False) + "\n")
    finally:
        client.close()
    if hashlib.sha256(dataset.read_bytes()).hexdigest() != before_sha256:
        raise ValueError("source dataset changed")
    summary = {"words": len(rows), "before_hit": sum(row["before_hit"] for row in rows),
               "after_hit": sum(row["after_hit"] for row in rows),
               "lost_words": [row["word"] for row in rows if row["before_hit"] and not row["after_hit"]],
               "output_bytes": sum(row["output_bytes"] for row in rows),
               "output_characters": sum(row["output_characters"] for row in rows),
               "max_output_bytes": max(row["output_bytes"] for row in rows)}
    report = {"binary": str(arguments.binary.resolve()), "dataset_sha256": before_sha256,
              "uncapped_jsonl": str(arguments.uncapped_jsonl.resolve()), "tool_schema": schema,
              "summary": summary, "words": rows, "model_evaluation": "not run; this driver calls no agents"}
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, default=POC / "datasets/rts5-75e02862daa29308/dataset.sqlite3")
    parser.add_argument("--eval-set", type=Path, default=HERE / "eval-set.json")
    parser.add_argument("--uncapped-jsonl", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    print(json.dumps(evaluate(parser.parse_args()), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
