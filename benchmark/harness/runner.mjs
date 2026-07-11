#!/usr/bin/env node

/**
 * CMS Official Benchmark 20260619-02 — durable run-local runner
 *
 * 설계 원칙:
 *   - episode-list-driven: 슬라이스(3 episode)와 full run(180 episode) 모두 동일 진입점 사용
 *   - 동시성 제어: claude_serial = semaphore(1), non_claude_capped = semaphore(3),
 *       per-codebase 상호배제 = codebase당 in-flight ≤ 1 (글로벌, claude 포함)
 *   - per-episode 1800s wall-time timeout (인자 --timeout-s로 조정; 측정 max 896.9s 근거)
 *   - claude = stream-json 추출 / codex = --output-last-message 추출 / opencode = stdout 추출
 *   - target-root mutation guard: find mtime+size manifest 비교 (pre/post episode)
 *   - --skip-scorer 지정 시 외부 judge를 호출하지 않고 풀이 기록만 보존
 *
 * 사용법:
 *   node runner.mjs --episodes <json-file-or-inline-json>
 *                   --arm-config <path>
 *                   --manifest <path>
 *                   --readiness <path>
 *                   --scorer <path>
 *                   --out-root <dir>
 *                   [--timeout-s 1800]
 *                   --codemap-bin <absolute-path>
 *                   [--workspace-root <dir>]
 *                   [--judge-model opus] [--skip-scorer]
 *                   [--print-cmd]
 *
 * episodes 형식: [{arm_id, codebase, round}]
 */

import { spawn, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { appendFileSync, closeSync, copyFileSync, existsSync, fsyncSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ============================================================
// 상수 / 경로
// ============================================================

const REPO_ROOT_PLACEHOLDER = "<REPO_ROOT>";
const DEFAULT_WORKSPACE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CLAUDE_BIN = "claude";

// ============================================================
// 인자 파싱
// ============================================================

function parseArgs(argv) {
    const out = {};
    for (let i = 0; i < argv.length; i++) {
        const a = argv[i];
        if (a.startsWith("--")) {
            const key = a.slice(2);
            const next = argv[i + 1];
            if (next === undefined || next.startsWith("--")) {
                out[key] = true;
            } else {
                out[key] = next;
                i++;
            }
        }
    }
    return out;
}

// ============================================================
// 유틸리티
// ============================================================

function readJson(filePath) {
    return JSON.parse(readFileSync(filePath, "utf8"));
}

function resolveManifestPlaceholders(value, workspaceRoot) {
    if (typeof value === "string") return value.replaceAll(REPO_ROOT_PLACEHOLDER, workspaceRoot);
    if (Array.isArray(value)) return value.map((item) => resolveManifestPlaceholders(item, workspaceRoot));
    if (value && typeof value === "object") {
        return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, resolveManifestPlaceholders(item, workspaceRoot)]));
    }
    return value;
}

function resolveWorkspacePath(value, workspaceRoot) {
    if (typeof value !== "string" || value.length === 0) {
        throw new Error(`[runner:path] expected a non-empty path, received: ${JSON.stringify(value)}`);
    }
    const placeholderResolved = value.replaceAll(REPO_ROOT_PLACEHOLDER, workspaceRoot);
    return path.isAbsolute(placeholderResolved) ? path.normalize(placeholderResolved) : path.resolve(workspaceRoot, placeholderResolved);
}

function requireDirectory(label, resolvedPath) {
    if (!existsSync(resolvedPath) || !statSync(resolvedPath).isDirectory()) {
        throw new Error(`[runner:path] ${label} directory does not exist: ${resolvedPath}`);
    }
}

function requireFile(label, resolvedPath) {
    if (!existsSync(resolvedPath) || !statSync(resolvedPath).isFile()) {
        throw new Error(`[runner:path] ${label} file does not exist: ${resolvedPath}`);
    }
}

function writeJson(filePath, value) {
    mkdirSync(path.dirname(filePath), { recursive: true });
    writeFileSync(filePath, JSON.stringify(value, null, 2) + "\n");
}

function writeText(filePath, text) {
    mkdirSync(path.dirname(filePath), { recursive: true });
    writeFileSync(filePath, String(text ?? ""));
}

function appendAttemptLedger(outRoot, event) {
    const ledgerPath = path.join(outRoot, "attempt-ledger.jsonl");
    mkdirSync(path.dirname(ledgerPath), { recursive: true });
    // One JSONL record is appended with O_APPEND and flushed before a solver can start.
    // Never put command lines, environments, prompts, or model output in this ledger.
    const fd = openSync(ledgerPath, "a");
    try {
        appendFileSync(fd, JSON.stringify(event) + "\n", { encoding: "utf8" });
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
    return ledgerPath;
}

function episodeClaimPath(outRoot, identitySha256) {
    return path.join(outRoot, "claims", `${identitySha256}.claim`);
}

function acquireEpisodeClaim(outRoot, identitySha256) {
    const claimPath = episodeClaimPath(outRoot, identitySha256);
    mkdirSync(path.dirname(claimPath), { recursive: true });
    let fd;
    try {
        fd = openSync(claimPath, "wx");
        writeFileSync(fd, JSON.stringify({ pid: process.pid, claimed_at: new Date().toISOString(), identity_sha256: identitySha256 }) + "\n");
        fsyncSync(fd);
    } catch (error) {
        if (error?.code === "EEXIST") throw new Error(`[runner:claim] identity already claimed; refusing automatic retry: ${identitySha256}`);
        throw error;
    } finally {
        if (fd != null) closeSync(fd);
    }
    return claimPath;
}

function releaseEpisodeClaim(claimPath) {
    try { unlinkSync(claimPath); } catch (error) { if (error?.code !== "ENOENT") throw error; }
}

const TERMINAL_LEDGER_EVENTS = new Set(["completed", "terminal_success", "terminal_failure"]);
const SOLVER_REUSABLE_ARTIFACTS = ["stdout.txt", "stderr.txt", "raw_answer.txt", "tool_events.json", "process_result.json", "mutation_guard_before.json", "mutation_guard_after.json", "mutation_guard.json"];
const SUCCESSFUL_TERMINAL_EVENTS = new Set(["completed", "terminal_success"]);

function artifactCorruption(message) {
    throw new Error(`[runner:artifact_corruption] ${message}`);
}

function ledgerCorruption(message) {
    throw new Error(`[runner:ledger_corruption] ${message}`);
}

function ledgerEventsForIdentity(outRoot, solverIdentitySha256) {
    const ledgerPath = path.join(outRoot, "attempt-ledger.jsonl");
    if (!existsSync(ledgerPath)) return [];
    return readFileSync(ledgerPath, "utf8").split("\n").flatMap((line, index) => {
        if (!line.trim()) return [];
        let value;
        try { value = JSON.parse(line); } catch { ledgerCorruption(`invalid JSONL at ${ledgerPath}:${index + 1}`); }
        if (!value || typeof value !== "object") ledgerCorruption(`non-object event at ${ledgerPath}:${index + 1}`);
        return value.solver_identity_sha256 === solverIdentitySha256 ? [value] : [];
    });
}

function validatedTerminalEvents(outRoot, solverIdentitySha256) {
    const events = ledgerEventsForIdentity(outRoot, solverIdentitySha256);
    const eventsByAttempt = new Map();
    const terminalsByAttempt = new Map();
    for (const event of events) {
        if (typeof event.attempt_id === "string" && event.attempt_id.length > 0) {
            const attemptEvents = eventsByAttempt.get(event.attempt_id) ?? [];
            attemptEvents.push(event);
            eventsByAttempt.set(event.attempt_id, attemptEvents);
        }
        if (!TERMINAL_LEDGER_EVENTS.has(event.event)) continue;
        if (typeof event.attempt_id !== "string" || event.attempt_id.length === 0) {
            ledgerCorruption(`terminal event without attempt_id for solver identity: ${solverIdentitySha256}`);
        }
        const terminals = terminalsByAttempt.get(event.attempt_id) ?? [];
        terminals.push(event);
        terminalsByAttempt.set(event.attempt_id, terminals);
    }
    for (const [attemptId, terminals] of terminalsByAttempt) {
        if (terminals.length > 1) {
            ledgerCorruption(`multiple terminal events for attempt ${attemptId}; refusing ambiguous ledger`);
        }
    }
    const successful = [...terminalsByAttempt.values()].flat().filter((event) => SUCCESSFUL_TERMINAL_EVENTS.has(event.event));
    const solverSuccesses = successful.filter((event) => !(eventsByAttempt.get(event.attempt_id) ?? []).some((attemptEvent) => attemptEvent.event === "scorer_started"));
    if (solverSuccesses.length > 1) {
        ledgerCorruption(`multiple successful attempts for solver identity; refusing arbitrary reuse: ${solverIdentitySha256}`);
    }
    const reusable = events.filter((event) => event.event === "solver_reusable");
    if (reusable.length > 1) ledgerCorruption(`multiple reusable solver records for solver identity; refusing arbitrary reuse: ${solverIdentitySha256}`);
    return { successful, solverSuccesses, reusable };
}

function hasPriorTerminalOrStartedAttempt(outRoot, solverIdentitySha256) {
    const events = ledgerEventsForIdentity(outRoot, solverIdentitySha256);
    const byAttempt = new Map();
    for (const event of events) byAttempt.set(event.attempt_id, event.event);
    return [...byAttempt.values()].some((event) => event === "started" || event === "terminal_failure");
}

function sha256(value) {
    return createHash("sha256").update(value).digest("hex");
}

function canonicalJson(value) {
    if (Array.isArray(value)) return `[${value.map((item) => canonicalJson(item)).join(",")}]`;
    if (value && typeof value === "object") {
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
    }
    return JSON.stringify(value);
}

function evaluationContractSha256(evaluationContract) {
    return evaluationContract ? sha256(canonicalJson(evaluationContract)) : null;
}

const IDENTITY_SCHEMA_VERSION = 2;

function canonicalRound(round) {
    if (typeof round === "number" && Number.isSafeInteger(round) && round >= 0) return round;
    if (typeof round === "string" && /^(?:0|[1-9]\d*)$/.test(round)) {
        const value = Number(round);
        if (Number.isSafeInteger(value)) return value;
    }
    throw new Error(`[runner:identity] round must be a non-negative safe integer or its canonical decimal string: ${JSON.stringify(round)}`);
}

function assertLedgerIdentitySchema(outRoot) {
    const ledgerPath = path.join(outRoot, "attempt-ledger.jsonl");
    if (!existsSync(ledgerPath)) return;
    for (const [index, line] of readFileSync(ledgerPath, "utf8").split("\n").entries()) {
        if (!line.trim()) continue;
        let event;
        try { event = JSON.parse(line); } catch { ledgerCorruption(`invalid JSONL at ${ledgerPath}:${index + 1}`); }
        if (event?.solver_identity_sha256 && event.identity_schema_version !== IDENTITY_SCHEMA_VERSION) {
            throw new Error(`[runner:identity] legacy ledger identity at ${ledgerPath}:${index + 1} cannot be safely compared with schema v${IDENTITY_SCHEMA_VERSION}; use a fresh --out-root`);
        }
    }
}

function fileDigest(filePath) {
    if (!existsSync(filePath)) return { exists: false, sha256: null, bytes: null };
    const content = readFileSync(filePath);
    return { exists: true, sha256: sha256(content), bytes: content.length };
}

function realpathInside(root, candidate, label) {
    const rootReal = realpathSync(root);
    const candidateReal = realpathSync(candidate);
    if (candidateReal !== rootReal && !candidateReal.startsWith(rootReal + path.sep)) {
        throw new Error(`[fixture] ${label} escapes target root: ${candidate}`);
    }
    return { rootReal, candidateReal };
}

function gitOutput(targetRoot, args) {
    // Large fixtures such as ClickHouse have more than 1 MiB of tracked paths. Node's default
    // spawnSync buffer otherwise terminates `git ls-files` with ENOBUFS and makes preflight
    // misclassify a valid fixture as having zero source files.
    const result = spawnSync("git", ["-C", targetRoot, ...args], { encoding: "utf8", timeout: 15_000, maxBuffer: 16 * 1024 * 1024 });
    return result.status === 0 ? String(result.stdout).trim() : null;
}

function targetIdentity(targetRoot) {
    const realpath = realpathSync(targetRoot);
    const gitRoot = gitOutput(realpath, ["rev-parse", "--show-toplevel"]);
    const head = gitOutput(realpath, ["rev-parse", "HEAD"]);
    const tree = gitOutput(realpath, ["ls-tree", "-r", "--full-tree", "HEAD"]);
    const dirty = gitOutput(realpath, ["diff", "--no-ext-diff", "--binary", "HEAD"]);
    return { realpath, git_root: gitRoot ? path.resolve(gitRoot) : null, head, tree_sha256: tree == null ? null : sha256(tree), dirty_tracked_sha256: dirty == null ? null : sha256(dirty), source_file_count: sourceFileCount(realpath) };
}

function runtimeIdentity(runtime) {
    const command = runtime === "claude-sonnet" ? "claude" : runtime === "codex-gpt54" ? "codex" : "opencode";
    const located = spawnSync("which", [command], { encoding: "utf8", timeout: 5_000 });
    const executable = located.status === 0 ? String(located.stdout).trim() : null;
    if (!executable || !existsSync(executable)) throw new Error(`[runner:runtime] executable not found for ${runtime}: ${command}`);
    const canonicalPath = realpathSync(executable);
    return { command, path: canonicalPath, version: commandVersion(canonicalPath), file: fileDigest(canonicalPath) };
}

function sourceFileCount(targetRoot) {
    const tracked = gitOutput(targetRoot, ["ls-files"]);
    if (tracked == null) return 0;
    const sourcePattern = /\.(?:[cm]?[jt]sx?|rs|cpp|cc|cxx|h|hpp|py|go|java|kt|swift|cs|php|rb|scala|sql)$/i;
    // git index membership is the intended source inventory. Avoid per-file stat fan-out;
    // sentinel and representative-read checks below establish on-disk availability.
    return tracked.split("\n").filter((relativePath) => relativePath && !relativePath.startsWith(".codemap/") && sourcePattern.test(relativePath)).length;
}

async function callCodemapMcp(codemapBin, targetRoot, request) {
    return await new Promise((resolve, reject) => {
        const child = spawn(codemapBin, ["mcp"], { cwd: targetRoot, stdio: ["pipe", "pipe", "pipe"] });
        let stderr = ""; let buffer = ""; let settled = false;
        const finish = (error, value) => {
            if (settled) return; settled = true; clearTimeout(timer);
            try { child.kill("SIGTERM"); } catch {}
            error ? reject(error) : resolve(value);
        };
        const timer = setTimeout(() => finish(new Error("[fixture] codemap MCP probe timed out")), 30_000);
        child.on("error", (error) => finish(new Error(`[fixture] codemap MCP spawn failed: ${error.message}`)));
        child.stderr.on("data", (chunk) => { stderr += String(chunk).slice(0, 2000); });
        child.stdout.on("data", (chunk) => {
            buffer += String(chunk);
            let newline;
            while ((newline = buffer.indexOf("\n")) >= 0) {
                const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
                let response; try { response = JSON.parse(line); } catch { continue; }
                if (response.id === request.id) {
                    if (response.error) finish(new Error(`[fixture] codemap MCP ${request.method} failed: ${response.error.message}`));
                    else finish(null, response.result);
                }
            }
        });
        child.on("close", (code) => { if (!settled) finish(new Error(`[fixture] codemap MCP exited before response: ${code}; ${stderr}`)); });
        const initialize = { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "benchmark-preflight", version: "1" } } };
        child.stdin.write(JSON.stringify(initialize) + "\n");
        child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized", params: {} }) + "\n");
        child.stdin.write(JSON.stringify(request) + "\n");
    });
}

function mcpText(result) {
    return result?.content?.filter((item) => item?.type === "text").map((item) => item.text).join("\n") ?? "";
}

async function preflightFixture({ taskDef, taskId, targetRoot, armDef, codemapBin, identity: suppliedIdentity = null }) {
    const fixture = taskDef.fixture;
    if (!fixture) throw new Error(`[fixture] missing fixture declaration for ${taskId}`);
    const identity = suppliedIdentity ?? targetIdentity(targetRoot);
    if (identity.git_root !== identity.realpath) throw new Error(`[fixture] target root is not an independent Git worktree: ${targetRoot}; git_root=${identity.git_root}`);
    if (identity.head !== fixture.expected_git_commit) throw new Error(`[fixture] git identity mismatch: expected ${fixture.expected_git_commit}, observed ${identity.head}; tree=${identity.tree_sha256}`);
    const count = identity.source_file_count;
    if (count < 1) throw new Error(`[fixture] no non-backend source files under ${identity.realpath}`);
    const sentinels = [];
    for (const relativePath of fixture.sentinel_paths || []) {
        const candidate = path.join(identity.realpath, relativePath);
        if (!existsSync(candidate) || !lstatSync(candidate).isFile()) throw new Error(`[fixture] sentinel missing for ${taskId}: ${relativePath}`);
        sentinels.push({ path: realpathInside(identity.realpath, candidate, "sentinel").candidateReal, sha256: fileDigest(candidate).sha256 });
    }
    const readRelativePath = fixture.representative_read_path || fixture.sentinel_paths?.[0];
    const readPath = path.join(identity.realpath, readRelativePath || "");
    if (!readRelativePath || !existsSync(readPath)) throw new Error(`[fixture] representative read missing: ${readPath}`);
    const readRealpath = realpathInside(identity.realpath, readPath, "representative read").candidateReal;
    const proof = { target_root_realpath: identity.realpath, source_file_count: count, sentinel_paths: sentinels, git_head: identity.head, git_tree_sha256: identity.tree_sha256, dirty_tracked_sha256: identity.dirty_tracked_sha256, read_path: readRealpath, read_sha256: fileDigest(readPath).sha256, backend: armDef.backend, arm_id: armDef.arm_id };
    if (armDef.backend === "codemap") {
        const indexPath = path.join(identity.realpath, ".codemap");
        // A symlinked index can make an unrelated stale cache look valid. The root itself and
        // every visible index entry must resolve under the target worktree.
        if (!existsSync(indexPath) || !lstatSync(indexPath).isDirectory()) throw new Error(`[fixture] codemap index not ready: ${indexPath}`);
        if (lstatSync(indexPath).isSymbolicLink()) throw new Error(`[fixture] codemap index symlink is not allowed: ${indexPath}`);
        const indexEntries = readdirSync(indexPath);
        if (indexEntries.length === 0) throw new Error(`[fixture] codemap index not ready: ${indexPath}`);
        for (const entry of indexEntries) {
            const entryPath = path.join(indexPath, entry);
            if (lstatSync(entryPath).isSymbolicLink()) throw new Error(`[fixture] codemap index symlink is not allowed: ${entryPath}`);
            realpathInside(identity.realpath, entryPath, "codemap index entry");
        }
        const searchResult = await callCodemapMcp(codemapBin, identity.realpath, { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "search", arguments: { query: fixture.codemap_query, limit: 10 } } });
        const searchText = mcpText(searchResult);
        const match = searchText.match(/^### File: (.+?) \(\d+ lines\)$/m);
        if (!match || /No indexed matches/i.test(searchText)) throw new Error(`[fixture] codemap MCP search produced no in-root match: query=${fixture.codemap_query}`);
        const matchPath = realpathInside(identity.realpath, path.join(identity.realpath, match[1]), "codemap MCP search match").candidateReal;
        const readResult = await callCodemapMcp(codemapBin, identity.realpath, { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "read", arguments: { file_path: readRelativePath, offset: 1, limit: 40 } } });
        const readText = mcpText(readResult);
        if (!readText.trim()) throw new Error(`[fixture] codemap MCP read returned empty content: ${readRelativePath}`);
        proof.index_path = realpathInside(identity.realpath, indexPath, "codemap index").candidateReal;
        proof.search_query = fixture.codemap_query;
        proof.search_match_path = matchPath;
        proof.search_response_sha256 = sha256(searchText);
        proof.read_response_sha256 = sha256(readText);
        proof.mcp_read_path = readRealpath;
    }
    return { identity, proof };
}

function commandVersion(command) {
    const result = spawnSync(command, ["--version"], { encoding: "utf8", timeout: 5_000 });
    return result.status === 0 ? String(result.stdout).trim() || String(result.stderr).trim() : null;
}

function codemapBinaryIdentity(codemapBin) {
    const digest = fileDigest(codemapBin);
    if (!digest.exists || !statSync(codemapBin).isFile()) {
        throw new Error(`--codemap-bin must name an existing file: ${codemapBin}`);
    }
    const version = commandVersion(codemapBin); if (!version) throw new Error(`--codemap-bin must support --version: ${codemapBin}`);
    return { path: codemapBin, ...digest, version };
}

function scoringContract({ scorerPath, schemaPath, privateAnswerKeyPath, judgeModel, printCmd }) {
    return {
        scorer: { path: scorerPath, ...fileDigest(scorerPath) },
        schema: { path: schemaPath, ...fileDigest(schemaPath) },
        private_answer_key: { path: privateAnswerKeyPath, ...fileDigest(privateAnswerKeyPath) },
        judge_model: judgeModel, print_command: printCmd,
    };
}

function fixtureContract(taskDef, taskId, armDef, codemapIdentity) {
    const fixture = taskDef.fixture ?? {};
    return {
        task_id: taskId,
        arm_id: armDef.arm_id,
        backend: armDef.backend,
        effective_codemap_usage: armDef.backend === "codemap",
        codemap_binary: armDef.backend === "codemap" ? codemapIdentity : null,
        expected_git_commit: fixture.expected_git_commit ?? null,
        sentinel_paths: [...(fixture.sentinel_paths ?? [])].sort(),
        representative_read_path: fixture.representative_read_path ?? fixture.sentinel_paths?.[0] ?? null,
        codemap_query: armDef.backend === "codemap" ? fixture.codemap_query ?? null : null,
    };
}

function cachedIdentity(config, cacheName, key, compute) {
    const cache = config[cacheName] ?? (config[cacheName] = new Map());
    let promise = cache.get(key);
    if (!promise) {
        promise = Promise.resolve().then(compute);
        cache.set(key, promise);
        promise.catch(() => { if (cache.get(key) === promise) cache.delete(key); });
    }
    return promise;
}

function buildResumeIdentity({ armDef, promptPath, prompt, config, contract, fixtureIdentity, evaluationContract, runtime, episode }) {
    const solverValue = {
        identity_schema_version: IDENTITY_SCHEMA_VERSION,
        episode,
        task: {
            code_root: fixtureIdentity,
            prompt: { path: promptPath, ...fileDigest(promptPath), sha256: sha256(prompt) },
            evaluation_contract_sha256: evaluationContractSha256(evaluationContract),
        },
        solver: { runtime, command: armDef.command, model: armDef.model, backend: armDef.backend, permissions: { shell_policy: armDef.shell_policy, builtin_read_policy: armDef.builtin_read_policy, mcp_config_policy: armDef.mcp_config_policy }, codemap_binary: armDef.backend === "codemap" ? config.codemapBinary : null, timeout_ms: config.timeoutMs },
        runner: fileDigest(config.paths.runnerPath),
    };
    const finalValue = {
        identity_schema_version: IDENTITY_SCHEMA_VERSION,
        solver_identity: sha256(JSON.stringify(solverValue)),
        skip_scorer: config.skipScorer,
        scoring_contract: contract,
    };
    return { sha256: sha256(JSON.stringify(finalValue)), value: finalValue, solver_identity: { sha256: finalValue.solver_identity, value: solverValue } };
}

function buildArtifactSeal(episodeDir, artifactNames) {
    return Object.fromEntries(artifactNames.map((name) => [name, fileDigest(path.join(episodeDir, name))]));
}

function validScorerOutput(value, schema = null, taskId = null, rawAnswerSha256 = null) {
    if (!value || typeof value !== "object" || !Number.isFinite(value.score) || value.score < 0 || value.score > 1) return false;
    if (!schema) return true;
    if (
        typeof value.schema_version !== "string" ||
        !value.scorer_output ||
        typeof value.scorer_output !== "object" ||
        typeof value.scorer_output.schema_version !== "string" ||
        value.task_id !== taskId ||
        value.candidate_id !== schema.candidate_id ||
        value.schema_version !== schema.schema_version ||
        value.scorer_output.schema_version !== schema.schema_version ||
        value.answer_sha256 !== rawAnswerSha256
    ) return false;
    if (!Array.isArray(value.per_fact_score) || value.per_fact_score.length !== schema.facts.length) return false;
    const facts = new Map(schema.facts.map((fact) => [fact.fact_id, fact]));
    let numerator = 0; let denominator = 0;
    for (const item of value.per_fact_score) {
        const fact = facts.get(item?.fact_id);
        const verdictValue = { present: 1, partial: 0.5, absent: 0 }[item?.verdict];
        if (!fact || verdictValue === undefined || item.value !== verdictValue) return false;
        facts.delete(item.fact_id); numerator += fact.weight * verdictValue; denominator += fact.weight;
    }
    return facts.size === 0 && denominator > 0 && value.score === Number((numerator / denominator).toFixed(6)) && value.fact_count_F === schema.facts.length && (value.verdict === "pass" || value.verdict === "fail");
}

const REQUIRED_ARTIFACTS = ["stdout.txt", "stderr.txt", "raw_answer.txt", "tool_events.json", "process_result.json", "mutation_guard_before.json", "mutation_guard_after.json", "mutation_guard.json", "harness_judgment.json", "result_metrics.json", "scorer_output.json"];

function readCanonicalTerminal(outRoot, solverIdentitySha256, finalIdentitySha256 = null) {
    const terminals = validatedTerminalEvents(outRoot, solverIdentitySha256).successful.filter((event) => !finalIdentitySha256 || event.final_identity_sha256 === finalIdentitySha256);
    if (terminals.length > 1) ledgerCorruption(`multiple successful terminal events for final identity; refusing arbitrary selection: ${solverIdentitySha256}`);
    const terminal = terminals[0] ?? null;
    if (terminal && (!terminal.canonical_artifacts || !terminal.attempt_dir)) {
        artifactCorruption(`successful terminal event lacks canonical artifacts: ${terminal.attempt_id}`);
    }
    return terminal;
}

function validateCompletedArtifacts(attemptDir, terminal, resumeIdentity, skipScorer, schemaPath, taskId, completedEpisodeDir = attemptDir) {
    if (!terminal || terminal.final_identity_sha256 !== resumeIdentity.sha256) return null;
    for (const artifactDir of new Set([attemptDir, completedEpisodeDir])) {
        for (const name of REQUIRED_ARTIFACTS) {
            const expected = terminal.canonical_artifacts?.[name]; const observed = fileDigest(path.join(artifactDir, name));
            if (!expected?.exists || expected.sha256 !== observed.sha256 || expected.bytes !== observed.bytes) artifactCorruption(`canonical artifact mismatch for ${terminal.attempt_id}: ${name}`);
        }
    }
    try {
        if (terminal.artifact_manifest_sha256 !== fileDigest(path.join(attemptDir, "artifact_manifest.json")).sha256 || terminal.metadata_sha256 !== fileDigest(path.join(attemptDir, "episode_metadata.json")).sha256 || terminal.artifact_manifest_sha256 !== fileDigest(path.join(completedEpisodeDir, "artifact_manifest.json")).sha256 || terminal.metadata_sha256 !== fileDigest(path.join(completedEpisodeDir, "episode_metadata.json")).sha256) {
            artifactCorruption(`canonical manifest or metadata mismatch for ${terminal.attempt_id}`);
        }
        const metrics = readJson(path.join(attemptDir, "result_metrics.json"));
        const judgment = readJson(path.join(attemptDir, "harness_judgment.json"));
        const metadata = readJson(path.join(attemptDir, "episode_metadata.json"));
        const answerHash = fileDigest(path.join(attemptDir, "raw_answer.txt")).sha256;
        if (metrics.harness_valid !== true || judgment.harness_valid !== true || metrics.answer_sha256 !== answerHash || judgment.answer_sha256 !== answerHash || metrics.episode_id !== terminal.episode_id || judgment.episode_id !== terminal.episode_id || metadata?.resume_identity?.sha256 !== resumeIdentity.sha256) artifactCorruption(`answer or episode cross-check failed for ${terminal.attempt_id}`);
        for (const name of REQUIRED_ARTIFACTS) {
            const expected = metadata?.artifact_seal?.[name]; const observed = fileDigest(path.join(attemptDir, name));
            if (!expected?.exists || expected.sha256 !== observed.sha256 || expected.bytes !== observed.bytes) artifactCorruption(`episode seal mismatch for ${terminal.attempt_id}: ${name}`);
        }
        const scorer = readJson(path.join(attemptDir, "scorer_output.json"));
        if (skipScorer ? scorer.status !== "skipped" : !validScorerOutput(scorer, readJson(schemaPath), taskId, answerHash)) artifactCorruption(`scorer cross-check failed for ${terminal.attempt_id}`);
        return metrics;
    } catch (error) {
        if (String(error?.message ?? error).includes("[runner:artifact_corruption]")) throw error;
        artifactCorruption(`unreadable canonical artifact set for ${terminal.attempt_id}`);
    }
}

function findReusableSolverAttempt(outRoot, solverIdentitySha256) {
    const candidates = validatedTerminalEvents(outRoot, solverIdentitySha256).reusable;
    if (candidates.length === 0) return null;
    const candidate = candidates[0];
    if (!candidate.canonical_solver_artifacts || !candidate.attempt_dir) artifactCorruption(`reusable solver artifact inventory is incomplete: ${candidate.attempt_id}`);
    try {
        const metrics = readJson(path.join(candidate.attempt_dir, "result_metrics.json"));
        const judgment = readJson(path.join(candidate.attempt_dir, "harness_judgment.json"));
        const answerHash = fileDigest(path.join(candidate.attempt_dir, "raw_answer.txt")).sha256;
        if (candidate.solver_identity_sha256 !== solverIdentitySha256 || metrics.harness_valid !== true || judgment.harness_valid !== true || metrics.answer_sha256 !== answerHash || judgment.answer_sha256 !== answerHash) artifactCorruption(`solver answer cross-check failed for ${candidate.attempt_id}`);
        for (const name of SOLVER_REUSABLE_ARTIFACTS) {
            const expected = candidate.canonical_solver_artifacts[name]; const observed = fileDigest(path.join(candidate.attempt_dir, name));
            if (!expected?.exists || expected.sha256 !== observed.sha256 || expected.bytes !== observed.bytes) artifactCorruption(`solver artifact seal mismatch for ${candidate.attempt_id}: ${name}`);
        }
        return candidate;
    } catch (error) {
        if (String(error?.message ?? error).includes("[runner:artifact_corruption]")) throw error;
        artifactCorruption(`unreadable reusable solver artifact set for ${candidate.attempt_id}`);
    }
}

function shQuote(value) {
    const text = String(value);
    if (text === "") return "''";
    if (/^[A-Za-z0-9_./:=@%+,-]+$/.test(text)) return text;
    return "'" + text.replace(/'/g, "'\\''") + "'";
}

function commandLine(command, args) {
    return [command, ...args].map(shQuote).join(" ");
}

function maybeJson(text) {
    try {
        return JSON.parse(text);
    } catch {
        return null;
    }
}

function jsonLines(text) {
    const lines = [];
    if (!text) return lines;
    for (const rawLine of String(text).split(/\r?\n/)) {
        const line = rawLine.trim();
        if (!line) continue;
        const parsed = maybeJson(line);
        if (parsed !== null) lines.push(parsed);
    }
    return lines;
}

function byteLength(value) {
    if (value === undefined || value === null) return 0;
    const text = typeof value === "string" ? value : JSON.stringify(value);
    return Buffer.byteLength(text, "utf8");
}

// ============================================================
// mutation guard: target-root 스냅샷 (mtime + size)
// ============================================================

/**
 * target root 하위 모든 파일의 상대경로, size, mtime_ms를 수집한다.
 * 디렉터리 수도 포함 (type 구분).
 */
function snapshotTargetRoot(root) {
    const entries = [];
    const stack = [root];
    while (stack.length > 0) {
        const current = stack.pop();
        let names = [];
        try {
            names = readdirSync(current);
        } catch {
            continue;
        }
        for (const name of names) {
            const filePath = path.join(current, name);
            let stat;
            try {
                stat = statSync(filePath);
            } catch {
                continue;
            }
            const relPath = path.relative(root, filePath);
            entries.push({
                path: relPath,
                type: stat.isDirectory() ? "dir" : "file",
                size: stat.isFile() ? stat.size : null,
                mtime_ms: stat.mtimeMs,
            });
            if (stat.isDirectory()) stack.push(filePath);
        }
    }
    entries.sort((a, b) => a.path.localeCompare(b.path));
    return entries;
}

/**
 * 백엔드 산출물 경로 prefix — belt-and-suspenders로 violation에서 제외한다.
 * 1차 분류는 git-tracked 여부지만, .git 메타나 git이 없는 경우를 대비해 유지.
 */
const BACKEND_ALLOWED_PREFIXES = [".codemap/", ".serena/", ".codegraph/", ".git/"];
const BACKEND_ALLOWED_ROOTS = [".codemap", ".serena", ".codegraph", ".git"];

function isBackendArtifactPath(relPath) {
    const posix = relPath.split(path.sep).join("/");
    if (BACKEND_ALLOWED_ROOTS.includes(posix)) return true;
    return BACKEND_ALLOWED_PREFIXES.some((prefix) => posix.startsWith(prefix));
}

/**
 * 주어진 상대경로 집합 중 git-ignored(추적되지 않는 빌드 산출물 등)를 batch로 판정한다.
 * `git -C <root> check-ignore --stdin`에 경로를 흘려보내 ignored 경로 집합을 받는다.
 * git이 없거나 실패하면 빈 집합 반환(보수적: 그땐 backend-prefix 규칙으로만 판정).
 *
 * 근거(P7b 실측): target/ 는 deno-main에서 git-ignored 빌드 트리다. rust-analyzer(serena)나
 *   동시 실행 episode가 target/ fingerprint를 touch하지만 이는 추적 소스가 아니다 —
 *   "소스 트리 변경"의 충실한 해석은 "git-tracked 소스 변경"이다. P7의 over-broad 규칙
 *   (backend allowlist 밖 모든 것=violation)이 target/를 false-positive로 swept-in 했다.
 */
function gitIgnoredPaths(root, relPaths) {
    const ignored = new Set();
    if (relPaths.length === 0) return ignored;
    // git 작업트리 여부 확인
    const isRepo = spawnSync("git", ["-C", root, "rev-parse", "--is-inside-work-tree"], {
        encoding: "utf8",
    });
    if (isRepo.status !== 0 || String(isRepo.stdout).trim() !== "true") return ignored;
    // POSIX 경로로 stdin 전달
    const posixPaths = relPaths.map((p) => p.split(path.sep).join("/"));
    const res = spawnSync("git", ["-C", root, "check-ignore", "--stdin"], {
        input: posixPaths.join("\n") + "\n",
        encoding: "utf8",
        maxBuffer: 256 * 1024 * 1024,
    });
    // check-ignore: ignored 경로만 stdout에 한 줄씩. exit 0=일부 ignored, 1=없음, >1=에러.
    if (res.status === 0 || res.status === 1) {
        for (const line of String(res.stdout || "").split(/\r?\n/)) {
            const t = line.trim();
            if (t) ignored.add(t);
        }
    }
    return ignored;
}

/**
 * baseline(readiness 이후)과 after(episode 이후) 스냅샷을 git-tracked 기반으로 비교한다.
 *
 * 분류:
 *   - source_mutation(violation) = git-tracked 소스의 신규/크기/ mtime-only 변경. → harness_invalid 게이트.
 *   - artifact_churn(허용, 기록) = git-ignored 경로(target/, node_modules/ 등) +
 *       backend 산출물(.codemap/.serena/.codegraph/.git). solver/백엔드 정상 부수효과이며
 *       동시 실행 episode의 cross-contamination도 여기로 분류된다(false harness_invalid 방지).
 *   - violations 배열에는 source_mutation만. artifact_churn은 별도 배열에 기록(투명성).
 *
 * @param {string} root target-root 절대경로 (git check-ignore 기준)
 */
function diffSnapshots(baseline, after, root) {
    const baselineMap = new Map(baseline.map((e) => [e.path, e]));
    const afterMap = new Map(after.map((e) => [e.path, e]));

    function classifyChange(relPath, afterEntry, baselineEntry) {
        if (!baselineEntry) return { reason: "new_file_or_dir", type: afterEntry.type };
        if (afterEntry.type !== "file") return null;
        if (afterEntry.size !== baselineEntry.size) {
            return { reason: "size_changed", size_before: baselineEntry.size, size_after: afterEntry.size };
        }
        if (afterEntry.mtime_ms > baselineEntry.mtime_ms + 1000) {
            return {
                reason: "mtime_only_changed",
                size: afterEntry.size,
                mtime_before: baselineEntry.mtime_ms,
                mtime_after: afterEntry.mtime_ms,
            };
        }
        return null;
    }

    // 1단계: 모든 변경 수집
    const changes = [];
    for (const [relPath, afterEntry] of afterMap) {
        const baselineEntry = baselineMap.get(relPath);
        const change = classifyChange(relPath, afterEntry, baselineEntry);
        if (change) changes.push({ path: relPath, ...change });
    }

    // 2단계: git-ignored 판정 (root가 주어지면). backend-prefix는 항상 artifact로.
    const changedPaths = changes.map((c) => c.path);
    const ignoredSet = root ? gitIgnoredPaths(root, changedPaths) : new Set();

    const violations = [];
    const allowedBackendWrites = [];
    for (const c of changes) {
        const posix = c.path.split(path.sep).join("/");
        const isArtifact = isBackendArtifactPath(c.path) || ignoredSet.has(posix);
        if (isArtifact) {
            allowedBackendWrites.push({ ...c, kind: ignoredSet.has(posix) ? "git_ignored" : "backend_artifact" });
        } else {
            violations.push({ ...c, kind: "source_mutation" });
        }
    }
    return { violations, allowedBackendWrites };
}

// ============================================================
// claude stream-json 파싱
// ============================================================

function extractClaudeStreamJson(stdoutText) {
    let resultText = null;
    let usageObj = null;
    const assistantTexts = [];
    const toolCallsByCallId = {};
    const toolEvents = [];

    for (const event of jsonLines(stdoutText)) {
        // result 이벤트에서 최종 답변 + 사용량 추출
        if (event.type === "result") {
            if (typeof event.result === "string") resultText = event.result;
            if (event.usage) usageObj = event.usage;
        }

        // assistant 메시지에서 텍스트 + 도구 이벤트 추출
        const msg = event.message || (event.type === "assistant" ? event : null);
        if (!msg) continue;
        const content = msg.content || (msg.message && msg.message.content);
        if (!Array.isArray(content)) continue;

        for (const block of content) {
            if (!block) continue;
            if (block.type === "text" && typeof block.text === "string") {
                assistantTexts.push(block.text);
            } else if (block.type === "tool_use") {
                if (block.id) toolCallsByCallId[block.id] = block.name;
                toolEvents.push({
                    phase: "call",
                    tool_name: block.name,
                    call_id: block.id,
                    response_size_bytes: 0,
                });
            } else if (block.type === "tool_result") {
                const bytes = byteLength(block.content);
                const callId = block.tool_use_id || block.id || null;
                const resolvedName = block.tool_name || block.name || (callId && toolCallsByCallId[callId]) || null;
                toolEvents.push({
                    phase: "result",
                    tool_name: resolvedName,
                    call_id: callId,
                    response_size_bytes: bytes,
                });
            }
        }
    }

    const rawAnswer = resultText ?? assistantTexts.join("\n");
    return { rawAnswer, toolEvents, usageObj };
}

function extractTokensFromClaudeUsage(usageObj) {
    if (!usageObj) return null;
    return {
        input_tokens: usageObj.input_tokens ?? null,
        output_tokens: usageObj.output_tokens ?? null,
        cache_read_input_tokens: usageObj.cache_read_input_tokens ?? null,
        cache_creation_input_tokens: usageObj.cache_creation_input_tokens ?? null,
    };
}

// ============================================================
// codex JSON 파싱 (--json JSONL events + --output-last-message)
// ============================================================

function extractCodexOutput(stdoutText, lastMessagePath) {
    // raw answer는 --output-last-message 파일에서 읽음
    let rawAnswer = null;
    if (lastMessagePath && existsSync(lastMessagePath)) {
        rawAnswer = readFileSync(lastMessagePath, "utf8");
    }

    // 토큰 사용량 + 도구 이벤트를 단일 패스로 추출.
    // [버그 수정 2026-06-20] 이전 구현은 toolEvents:[]를 하드코딩 반환해 codex의 MCP/쉘
    //   도구 호출이 전수 0으로 오기록됐다. codex stdout JSONL은 도구 호출을
    //   item.started/item.completed 쌍(item.type = "mcp_tool_call" | "command_execution")으로
    //   내보낸다. claude/opencode 파서와 동일한 call/result 이벤트로 기록한다.
    //   네이밍은 claude와 동일한 mcp__<server>__<tool> (calcBackendExercised가 서버명으로 판정).
    let tokens = null;
    const toolEvents = [];

    for (const event of jsonLines(stdoutText)) {
        if (event.type === "turn.completed" && event.usage) {
            // codex usage: input_tokens는 캐시 포함 합산, cached_input_tokens는 그 부분집합.
            tokens = {
                input_tokens: event.usage.input_tokens ?? null,
                output_tokens: event.usage.output_tokens ?? null,
                cached_input_tokens: event.usage.cached_input_tokens ?? null,
            };
        }

        const item = event.item;
        if (!item) continue;

        // fallback raw answer: stdout JSONL의 마지막 텍스트 item
        if (rawAnswer === null && event.type === "item.completed" && typeof item.text === "string") {
            rawAnswer = item.text;
        }

        if (item.type === "mcp_tool_call" || item.type === "command_execution") {
            const callId = item.id || null;
            const toolName =
                item.type === "mcp_tool_call"
                    ? `mcp__${item.server || "unknown"}__${item.tool || "unknown"}`
                    : "command_execution";
            if (event.type === "item.started") {
                // call 이벤트 (started/completed 쌍 중 started에서 1회만 카운트)
                toolEvents.push({ phase: "call", tool_name: toolName, call_id: callId, response_size_bytes: 0 });
            } else if (event.type === "item.completed") {
                // result 이벤트 (출력 바이트: mcp는 result, 쉘은 aggregated_output)
                const out = item.result ?? item.aggregated_output ?? item.output ?? "";
                toolEvents.push({
                    phase: "result",
                    tool_name: toolName,
                    call_id: callId,
                    response_size_bytes: byteLength(out),
                });
            }
        }
    }

    return { rawAnswer: rawAnswer ?? "", tokens, toolEvents };
}

// ============================================================
// opencode 출력 파싱
// ============================================================

/**
 * opencode --format json JSONL 이벤트를 파싱한다.
 * 이벤트 형태(probe로 실측, opencode 1.17.7):
 *   {type:"tool_use", part:{tool:"<server>_<tool>", callID, state:{status, output, time}}}
 *   {type:"text", part:{text}}
 *   {type:"step_finish", part:{tokens:{total,input,output,reasoning,cache:{read,write}}, cost}}
 * 중요: opencode MCP 도구 네이밍은 `<server>_<tool>`(예: codemap-search_overview, serena_find_symbol)
 *   — claude의 `mcp__<server>__<tool>`와 다르다. backend_exercised 검출은 calcBackendExercised에서
 *   서버명 substring으로 runtime-agnostic하게 처리한다.
 */
function extractOpencodeJsonOutput(stdoutText) {
    const textParts = [];
    const toolEvents = [];
    const stepTokens = [];
    let parsedEventIndex = 0;
    for (const [rawLineIndex, rawLine] of String(stdoutText).split(/\r?\n/).entries()) {
        const event = maybeJson(rawLine.trim());
        if (event === null) continue;
        const eventPosition = { raw_line_number: rawLineIndex + 1, parsed_event_index: parsedEventIndex++ };
        const type = event.type;
        const part = event.part || {};
        if (type === "text" && typeof part.text === "string") {
            textParts.push(part.text);
        } else if (type === "tool_use" || type === "tool") {
            const name = part.tool || part.name || "unknown";
            const callId = part.callID || part.id || null;
            const state = part.state || {};
            const time = state.time || {};
            const toolInput = state.input && typeof state.input === "object" ? state.input : null;
            // call 이벤트
            toolEvents.push({
                phase: "call",
                tool_name: name,
                call_id: callId,
                response_size_bytes: 0,
                input: toolInput,
                started_at_epoch_ms: typeof time.start === "number" ? time.start : null,
                ...eventPosition,
            });
            // result 이벤트 (output 바이트 — truncated면 outputPath 파일 크기 우선)
            let bytes = 0;
            const meta = state.metadata || {};
            if (meta.truncated && meta.outputPath && existsSync(meta.outputPath)) {
                try {
                    bytes = statSync(meta.outputPath).size;
                } catch {
                    bytes = byteLength(state.output);
                }
            } else {
                bytes = byteLength(state.output);
            }
            toolEvents.push({
                phase: "result",
                tool_name: name,
                call_id: callId,
                response_size_bytes: bytes,
                input: toolInput,
                status: typeof state.status === "string" ? state.status : null,
                started_at_epoch_ms: typeof time.start === "number" ? time.start : null,
                finished_at_epoch_ms: typeof time.end === "number" ? time.end : null,
                duration_ms: typeof time.start === "number" && typeof time.end === "number" ? Math.max(0, time.end - time.start) : null,
                output_truncated: meta.truncated === true,
                ranked_results: /codemap[-_]search_search/i.test(name) ? extractCodemapRankedResults(state.output) : null,
                ...eventPosition,
            });
        } else if (type === "step_finish" && part.tokens) {
            const t = part.tokens;
            stepTokens.push({
                ...eventPosition,
                input_tokens: t.input ?? null,
                output_tokens: t.output ?? null,
                cache_read_input_tokens: (t.cache && t.cache.read) ?? null,
                cache_creation_input_tokens: (t.cache && t.cache.write) ?? null,
            });
        }
    }
    const sum = (field) => {
        const values = stepTokens.map((tokens) => tokens[field]).filter((value) => typeof value === "number");
        return values.length === 0 ? null : values.reduce((total, value) => total + value, 0);
    };
    return {
        rawAnswer: textParts.join("\n").trim(),
        tokens: stepTokens.length === 0 ? null : {
            input_tokens: sum("input_tokens"),
            output_tokens: sum("output_tokens"),
            cache_read_input_tokens: sum("cache_read_input_tokens"),
            cache_creation_input_tokens: sum("cache_creation_input_tokens"),
        },
        stepTokens,
        toolEvents,
    };
}

function extractCodemapRankedResults(output) {
    const ranked = [];
    const seen = new Set();
    for (const line of String(output ?? "").split(/\r?\n/)) {
        const detail = line.match(/^### File: (.+?) \(\d+ lines\)$/);
        const tail = line.match(/^- (?!read )([^`].+?) \(\d+ lines\)(?: —|$)/);
        const filePath = (detail?.[1] ?? tail?.[1] ?? "").replace(/^\.\//, "");
        if (!filePath || seen.has(filePath)) continue;
        seen.add(filePath);
        ranked.push({ rank: ranked.length + 1, path: filePath });
    }
    return ranked;
}

function percentile95(values) {
    const sorted = values.filter((value) => typeof value === "number").sort((a, b) => a - b);
    return sorted.length === 0 ? null : sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)];
}

function buildEvaluationObservation({ evaluation, toolEvents, solveStartedAt, backendExercised, processResult, judgeStatus, scorerScore, scorerOutput, schemaPath }) {
    if (!evaluation) return { schema_version: "1.0", status: "unconfigured", excluded_reason: "missing_evaluation_contract" };
    const canonicalPaths = new Set((evaluation.canonical_paths || []).map((value) => String(value).replace(/^\.\//, "")));
    const searchResults = toolEvents.filter((event) => event.phase === "result" && Array.isArray(event.ranked_results));
    const observations = searchResults.map((event, index) => {
        const rankedResults = event.ranked_results.map((result) => ({ ...result, relevant: canonicalPaths.has(result.path) }));
        const firstRelevant = rankedResults.find((result) => result.relevant) ?? null;
        return {
            observation_id: `${evaluation.query_id}:search-${index + 1}`,
            query: typeof event.input?.query === "string" ? event.input.query : null,
            duration_ms: event.duration_ms,
            finished_at_epoch_ms: event.finished_at_epoch_ms,
            output_truncated: event.output_truncated,
            ranked_results: rankedResults,
            first_relevant_rank: firstRelevant?.rank ?? null,
            recall_at_5: firstRelevant && firstRelevant.rank <= 5 ? 1 : 0,
        };
    });
    const searchHits = observations
        .filter((item) => item.first_relevant_rank !== null && typeof item.finished_at_epoch_ms === "number")
        .map((item) => ({ finished_at_epoch_ms: item.finished_at_epoch_ms, source: "search_result" }));
    const readHits = toolEvents
        .filter((event) => event.phase === "result" && /codemap[-_]search_read/i.test(event.tool_name) && typeof event.finished_at_epoch_ms === "number")
        .filter((event) => canonicalPaths.has(String(event.input?.file_path ?? event.input?.path ?? "").replace(/^\.\//, "")))
        .map((event) => ({ finished_at_epoch_ms: event.finished_at_epoch_ms, source: "read_result" }));
    const firstHit = [...searchHits, ...readHits].sort((a, b) => a.finished_at_epoch_ms - b.finished_at_epoch_ms)[0] ?? null;
    const recallValues = observations.map((item) => item.recall_at_5);
    const schema = readJson(schemaPath);
    const perFact = Array.isArray(scorerOutput?.per_fact_score) ? scorerOutput.per_fact_score : [];
    return {
        schema_version: "1.0",
        status: processResult.timedOut || processResult.exitCode !== 0 ? "failed" : backendExercised ? "observed" : "unobserved",
        query_id: evaluation.query_id,
        difficulty_tier: evaluation.difficulty_tier,
        relevance_contract_sha256: evaluationContractSha256(evaluation),
        canonical_path_count: canonicalPaths.size,
        search_observations: observations,
        search_observation_count: observations.length,
        recall_at_5: recallValues.length === 0 ? null : recallValues.reduce((sum, value) => sum + value, 0) / recallValues.length,
        first_correct_evidence_ms: firstHit ? Math.max(0, firstHit.finished_at_epoch_ms - solveStartedAt.getTime()) : null,
        first_correct_evidence_status: firstHit?.source ?? (observations.length === 0 ? "unobservable" : "not_found"),
        search_latency_samples_ms: observations.map((item) => item.duration_ms).filter((value) => typeof value === "number"),
        search_latency_p95_ms: percentile95(observations.map((item) => item.duration_ms)),
        scorer_quality: {
            status: judgeStatus,
            contract_valid: judgeStatus === "completed",
            score: scorerScore,
            fact_count_expected: schema.facts.length,
            fact_count_observed: perFact.length,
            scorer_output_sha256: scorerOutput ? sha256(JSON.stringify(scorerOutput)) : null,
        },
    };
}

// ============================================================
// 도구 메트릭 집계
// ============================================================

function summarizeToolMetrics(toolEvents) {
    const toolCallDistribution = {};
    const toolResultBytesByTool = {};
    for (const ev of toolEvents) {
        const name = ev.tool_name || "unknown";
        if (ev.phase === "call") {
            toolCallDistribution[name] = (toolCallDistribution[name] || 0) + 1;
        } else if (ev.phase === "result") {
            toolResultBytesByTool[name] = (toolResultBytesByTool[name] || 0) + (ev.response_size_bytes || 0);
        }
    }
    return { toolCallDistribution, toolResultBytesByTool };
}

/**
 * backend → MCP 서버명. 도구 네이밍이 runtime별로 다르다:
 *   claude/codex: mcp__<server>__<tool>   (예: mcp__serena__find_symbol)
 *   opencode:     <server>_<tool>          (예: serena_find_symbol, codemap-search_overview)
 * 둘 다 잡으려면 서버명을 substring으로 매칭하되, codemap은 서버명이 'codemap-search'다.
 */
const BACKEND_SERVER_NAME = {
    codemap: "codemap-search",
    serena: "serena",
    codegraph: "codegraph",
};

/**
 * 도구 이름이 주어진 backend의 MCP 도구인지 runtime-agnostic하게 판정.
 * codemap: 'codemap-search' 또는 'codemap_search' 또는 'codemap[-_]search' 형태 모두 허용.
 * serena/codegraph: 서버명으로 시작하는 mcp__ prefix 또는 <server>_ prefix.
 */
function isBackendToolName(backend, name) {
    if (!name) return false;
    const lower = String(name).toLowerCase();
    if (backend === "codemap") {
        return /codemap[-_]search/.test(lower);
    }
    const server = BACKEND_SERVER_NAME[backend];
    if (!server) return false;
    // claude/codex: mcp__serena__... ; opencode: serena_... 또는 serena-...
    return (
        lower.startsWith(`mcp__${server}`) ||
        lower.startsWith(`${server}_`) ||
        lower.startsWith(`${server}-`) ||
        lower === server
    );
}

function calcAssignedBackendToolBytes(backend, toolResultBytesByTool) {
    if (backend === "no-mcp") return 0;
    return Object.entries(toolResultBytesByTool).reduce(
        (sum, [name, bytes]) => (isBackendToolName(backend, name) ? sum + bytes : sum),
        0,
    );
}

function calcBackendExercised(backend, toolCallDistribution) {
    if (backend === "no-mcp") return true; // 내장 도구 기반이므로 항상 true로 취급
    return Object.keys(toolCallDistribution).some((n) => isBackendToolName(backend, n));
}

// ============================================================
// MCP config 파일 생성 (claude codemap arm용)
// ============================================================

function buildMcpConfigForArm(arm, targetRoot, episodeDir, codemapBin) {
    const mcpConfigPath = path.join(episodeDir, "mcp_config.json");
    if (arm.backend === "codemap") {
        writeJson(mcpConfigPath, {
            mcpServers: {
                "codemap-search": {
                    command: codemapBin,
                    args: ["mcp"],
                    cwd: targetRoot,
                },
            },
        });
    } else if (arm.backend === "serena") {
        // serena: start-mcp-server --project <root> --context ide
        writeJson(mcpConfigPath, {
            mcpServers: {
                serena: {
                    command: "serena",
                    args: ["start-mcp-server", "--project", targetRoot, "--context", "ide"],
                },
            },
        });
    } else if (arm.backend === "codegraph") {
        writeJson(mcpConfigPath, {
            mcpServers: {
                codegraph: {
                    command: "codegraph",
                    args: ["serve", "--mcp", "-p", targetRoot, "--no-watch"],
                },
            },
        });
    }
    return mcpConfigPath;
}

// ============================================================
// opencode XDG_CONFIG_HOME config 생성
// ============================================================

function buildOpencodeXdgConfig(arm, targetRoot, episodeDir, codemapBin) {
    const xdgHome = path.join(episodeDir, "opencode-xdg");
    const opencodeConfigDir = path.join(xdgHome, "opencode");
    mkdirSync(opencodeConfigDir, { recursive: true });

    // read-only 정책 (opencode 1.17.7 실측 기반):
    //   no-mcp arm = bash 완전 차단. grep/glob/read/list로 탐색 보존.
    //   MCP arm    = bash 완전 차단(MCP 도구로만 탐색).
    //   edit/write/patch는 모든 arm에서 차단(소스 mutation 금지).
    //
    // 설계 근거(P7b probe로 실증): permission.bash를 command-glob 맵(default-deny + read-only
    //   allow-list)으로 주려 했으나, opencode 1.17.7은 permission.bash가 object면 bash 도구를
    //   "unavailable tool"로 통째 비활성화한다(per-command 게이팅 미지원). 따라서 fragile한
    //   command-pattern 화이트리스트 대신 tools.bash:false로 결정론적으로 차단한다.
    //   probe 결과: `touch`(mutating) 차단으로 파일 미생성 확인, grep/glob/read는 보존됨.
    //   이는 P7의 `cargo build`(target/ mutation) 재발을 원천 차단한다.
    const toolsConfig = {
        bash: false,
        read: true,
        grep: arm.backend === "no-mcp", // no-mcp는 grep/glob 탐색 허용, MCP arm은 MCP 도구만
        glob: arm.backend === "no-mcp",
        list: arm.backend === "no-mcp",
        edit: false,
        write: false,
        patch: false,
    };

    // permission: bash:false와 중복이지만 belt-and-suspenders로 edit/web 차단을 명시.
    const permissionConfig = {
        edit: "deny",
        webfetch: "deny",
        websearch: "deny",
    };
    // run07: the Ollama Cloud codemap arm keeps builtin read, but must not resolve a
    // parent-workspace path when OpenCode receives an absolute file path.
    if (arm.arm_id === "opencode-ollama-cloud-deepseek-codemap") {
        permissionConfig.external_directory = "deny";
    }

    const mcpSection = {};
    if (arm.backend === "codemap") {
        mcpSection["codemap-search"] = {
            type: "local",
            command: [codemapBin, "mcp"],
            enabled: true,
            cwd: targetRoot,
        };
    } else if (arm.backend === "serena") {
        mcpSection["serena"] = {
            type: "local",
            command: ["serena", "start-mcp-server", "--project", targetRoot, "--context", "ide"],
            enabled: true,
        };
    } else if (arm.backend === "codegraph") {
        mcpSection["codegraph"] = {
            type: "local",
            command: ["codegraph", "serve", "--mcp", "-p", targetRoot, "--no-watch"],
            enabled: true,
        };
    }

    const config = {
        $schema: "https://opencode.ai/config.json",
        tools: toolsConfig,
        permission: permissionConfig,
        mcp: mcpSection,
    };
    const configPath = path.join(opencodeConfigDir, "opencode.jsonc");
    writeFileSync(configPath, JSON.stringify(config, null, 2));
    return { xdgHome, configPath };
}

// ============================================================
// 명령 빌드 + 실행
// ============================================================

/**
 * arm 설정과 episode 정보로 실행 명령을 빌드한다.
 * Returns: { command, args, env, cwd, lastMessagePath? }
 */
function buildEpisodeCommand(arm, targetRoot, prompt, episodeDir, codemapBin) {
    const runtime = arm.runtime;

    if (runtime === "claude-sonnet") {
        const args = ["-p", "--model", arm.model, "--setting-sources", "", "--strict-mcp-config"];

        if (arm.backend === "no-mcp") {
            args.push(
                "--allowedTools",
                "Bash,Read,Glob,Grep",
                "--disallowedTools",
                "Edit,Write,WebFetch,WebSearch,Task,NotebookEdit,TodoWrite,Workflow,Agent,Skill",
            );
        } else {
            const mcpConfigPath = buildMcpConfigForArm(arm, targetRoot, episodeDir, codemapBin);
            args.push("--mcp-config", mcpConfigPath);
            // arm config의 allowedTools/disallowedTools를 그대로 사용
            // arm.command에서 추출하거나 arm_config의 직접 필드를 사용
            const allowed = arm.allowedTools || deriveClaudeAllowedTools(arm);
            const disallowed = arm.disallowedTools || deriveClaudeDisallowedTools(arm);
            args.push("--allowedTools", allowed, "--disallowedTools", disallowed);
        }

        args.push("--output-format", "stream-json", "--verbose");
        // prompt는 stdin으로 전달 (args에 포함 안함)

        return {
            command: CLAUDE_BIN,
            args,
            env: { ...process.env },
            cwd: targetRoot,
            stdin: prompt,
            lastMessagePath: null,
        };
    }

    if (runtime === "codex-gpt54") {
        const lastMessagePath = path.join(episodeDir, "codex_last_message.txt");
        const args = [
            "exec",
            "-C",
            targetRoot,
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ephemeral",
            "-s",
            "read-only",
            "-m",
            arm.model,
            "-c",
            "model_reasoning_effort=medium",
            "-c",
            "approval_policy=never",
        ];

        if (arm.backend !== "no-mcp") {
            args.push("--disable", "shell_tool");
            if (arm.backend === "codemap") {
                args.push(
                    "-c",
                    `mcp_servers.codemap-search.command=${codemapBin}`,
                    "-c",
                    'mcp_servers.codemap-search.args=["mcp"]',
                    "-c",
                    `mcp_servers.codemap-search.cwd=${targetRoot}`,
                );
            } else if (arm.backend === "serena") {
                args.push(
                    "-c",
                    "mcp_servers.serena.command=serena",
                    "-c",
                    `mcp_servers.serena.args=["start-mcp-server","--project","${targetRoot}","--context","ide"]`,
                );
            } else if (arm.backend === "codegraph") {
                args.push(
                    "-c",
                    "mcp_servers.codegraph.command=codegraph",
                    "-c",
                    `mcp_servers.codegraph.args=["serve","--mcp","-p","${targetRoot}","--no-watch"]`,
                    "-c",
                    "mcp_servers.codegraph.default_tools_approval_mode=approve",
                );
            }
        }

        args.push("--json", "--output-last-message", lastMessagePath);

        return {
            command: "codex",
            args,
            env: { ...process.env },
            cwd: targetRoot,
            stdin: prompt,
            lastMessagePath,
        };
    }

    if (runtime.startsWith("opencode-")) {
        // OpenCode 1.17.18 uses --dir as the client/session directory.  The runner's
        // spawn cwd alone previously allowed a parent-workspace session to be selected.
        // Restrict this run07 repair to the affected Ollama Cloud codemap arm.
        const isRun07OllamaCodemapArm = arm.arm_id === "opencode-ollama-cloud-deepseek-codemap";
        const canonicalTargetRoot = isRun07OllamaCodemapArm ? realpathSync(targetRoot) : targetRoot;
        const { xdgHome, configPath } = buildOpencodeXdgConfig(arm, canonicalTargetRoot, episodeDir, codemapBin);
        // --format json: JSONL events(tool_use/text/step_finish) → 견고한 answer/tool/token 추출.
        // 기존 ANSI stdout 파싱은 fragile했고 tool_events/tokens를 못 얻었음.
        const args = isRun07OllamaCodemapArm
            ? ["run", "--dir", canonicalTargetRoot, "--model", arm.model, "--format", "json"]
            : ["run", "--model", arm.model, "--format", "json"];

        return {
            command: "opencode",
            args,
            env: { ...process.env, XDG_CONFIG_HOME: xdgHome },
            cwd: canonicalTargetRoot,
            stdin: prompt,
            lastMessagePath: null,
            opencodeConfigPath: configPath,
        };
    }

    throw new Error(`unsupported runtime: ${runtime}`);
}

function deriveClaudeAllowedTools(arm) {
    if (arm.backend === "codemap") {
        return "mcp__codemap-search__search,mcp__codemap-search__overview,mcp__codemap-search__read,mcp__codemap-search__find,mcp__codemap-search__grep,Read,ToolSearch";
    }
    if (arm.backend === "serena") {
        return "mcp__serena__find_symbol,mcp__serena__find_referencing_symbols,mcp__serena__find_implementations,mcp__serena__find_declaration,mcp__serena__get_symbols_overview,mcp__serena__search_for_pattern,mcp__serena__list_dir,mcp__serena__find_file,Read,ToolSearch";
    }
    if (arm.backend === "codegraph") {
        return "mcp__codegraph__codegraph_search,mcp__codegraph__codegraph_callers,mcp__codegraph__codegraph_node,mcp__codegraph__codegraph_explore,Read,ToolSearch";
    }
    return "Bash,Read,Glob,Grep";
}

function deriveClaudeDisallowedTools(arm) {
    const base = [
        "Bash",
        "Glob",
        "Grep",
        "Edit",
        "Write",
        "WebFetch",
        "WebSearch",
        "Task",
        "NotebookEdit",
        "TodoWrite",
        "Workflow",
        "Agent",
        "Skill",
    ];
    return base.join(",");
}

// ============================================================
// 프로세스 실행 (process group kill 포함)
// ============================================================

/**
 * episode 런타임(claude/codex/opencode)을 띄우고, 그 런타임이 spawn한 MCP/언어서버
 * (serena rust-analyzer, ClickHouse clangd, codegraph 데몬)까지 **프로세스 그룹** 단위로
 * 정리한다.
 *
 * 핵심: 백엔드 언어서버는 우리가 직접 spawn한 child가 아니라 런타임이 spawn한 **손자(grandchild)**다.
 *   `child.kill()`은 런타임만 reap하고 언어서버는 유휴로 남아 누적된다(warmup RESULT.md 발견 #1).
 *   따라서 `detached:true`로 새 프로세스 그룹을 만들고, 종료 시 `process.kill(-pgid, ...)`로
 *   그룹 전체를 보낸다. darwin에서 동작.
 *
 * cleanup은 timeout뿐 아니라 **정상 종료에서도** 실행한다 — 유휴 언어서버 누적은 clean exit에서도
 *   발생하기 때문(부모가 죽어도 detached 손자는 살아남을 수 있음).
 *
 * @returns process 결과 + groupKilled(그룹 정리 시도 여부)
 */
function runProcess(plan, timeoutMs, streamPaths = null, abortSignal = null) {
    return new Promise((resolve, reject) => {
        const startedAt = Date.now();
        let timedOut = false;
        const stdoutChunks = [];
        const stderrChunks = [];
        let spawnError = null;
        let settled = false;
        let stdoutFd = null;
        let stderrFd = null;

        // Streaming destinations are opened before the solver starts. A write failure is
        // therefore reported through the same promise boundary as a spawn/timeout failure.
        try {
            if (streamPaths?.stdout) stdoutFd = openSync(streamPaths.stdout, "a");
            if (streamPaths?.stderr) stderrFd = openSync(streamPaths.stderr, "a");
        } catch (error) {
            for (const fd of [stdoutFd, stderrFd]) {
                if (fd != null) try { closeSync(fd); } catch {}
            }
            reject(error);
            return;
        }

        let child;
        try {
            child = spawn(plan.command, plan.args, {
                cwd: plan.cwd,
                env: plan.env,
                stdio: ["pipe", "pipe", "pipe"],
                // detached:true → 자식이 새 프로세스 그룹의 리더가 됨(pgid == child.pid).
                // 런타임이 spawn한 MCP/언어서버 손자도 같은 그룹에 들어가 그룹 kill로 함께 정리된다.
                detached: true,
            });
        } catch (error) {
            for (const fd of [stdoutFd, stderrFd]) {
                if (fd != null) try { closeSync(fd); } catch {}
            }
            reject(error);
            return;
        }

        const childPid = child.pid;

        // 백엔드 프로세스 그룹 정리: 그룹 전체에 신호. 멱등(여러 번 호출 안전).
        let cleanedUp = false;
        function killProcessGroup(signal) {
            if (childPid == null) return false;
            try {
                // 음수 pid = 프로세스 그룹 전체. detached:true라 child.pid == pgid.
                process.kill(-childPid, signal);
                return true;
            } catch {
                // 그룹이 이미 사라졌거나(ESRCH) 권한 문제면 단일 프로세스로 폴백.
                try {
                    child.kill(signal);
                } catch {}
                return false;
            }
        }
        function cleanupBackendProcesses() {
            if (cleanedUp) return;
            cleanedUp = true;
            // 정상 종료 후에도 detached 손자(언어서버)가 남을 수 있어 그룹에 SIGTERM→SIGKILL.
            killProcessGroup("SIGTERM");
            setTimeout(() => {
                killProcessGroup("SIGKILL");
            }, 2000).unref();
        }

        function closeStreamFds() {
            for (const fd of [stdoutFd, stderrFd]) {
                if (fd == null) continue;
                try { fsyncSync(fd); } catch {}
                try { closeSync(fd); } catch {}
            }
            stdoutFd = null;
            stderrFd = null;
        }
        function settle(error, value) {
            if (settled) return;
            settled = true;
            clearTimeout(timer);
            abortSignal?.removeEventListener("abort", abortProcess);
            child.stdout.off("data", onStdout);
            child.stderr.off("data", onStderr);
            cleanupBackendProcesses();
            closeStreamFds();
            if (error) reject(error); else resolve(value);
        }
        function failStreamWrite(error) {
            cleanupBackendProcesses();
            settle(new Error(`[runner:stream_write] ${error?.message ?? error}`));
        }
        function abortProcess() {
            killProcessGroup("SIGTERM");
            setTimeout(() => killProcessGroup("SIGKILL"), 2000).unref();
        }
        function onStdout(chunk) {
            const bytes = Buffer.from(chunk);
            stdoutChunks.push(bytes);
            try { if (stdoutFd != null) writeFileSync(stdoutFd, bytes); } catch (error) { failStreamWrite(error); }
        }
        function onStderr(chunk) {
            const bytes = Buffer.from(chunk);
            stderrChunks.push(bytes);
            try { if (stderrFd != null) writeFileSync(stderrFd, bytes); } catch (error) { failStreamWrite(error); }
        }

        if (plan.stdin) {
            child.stdin.write(plan.stdin, "utf8");
            child.stdin.end();
        } else {
            child.stdin.end();
        }

        child.on("error", (err) => { spawnError = err; });
        child.stdout.on("data", onStdout);
        child.stderr.on("data", onStderr);

        const timer = setTimeout(() => {
            timedOut = true;
            // timeout: 그룹 전체에 SIGTERM, 5초 후 SIGKILL.
            killProcessGroup("SIGTERM");
            setTimeout(() => {
                killProcessGroup("SIGKILL");
            }, 5000).unref();
        }, timeoutMs);
        if (abortSignal) {
            if (abortSignal.aborted) abortProcess();
            else abortSignal.addEventListener("abort", abortProcess, { once: true });
        }

        child.on("close", (code, signal) => {
            const stdoutBytes = Buffer.concat(stdoutChunks);
            const stderrBytes = Buffer.concat(stderrChunks);
            settle(null, {
                exitCode: spawnError ? null : code,
                signal: signal ?? null,
                spawn_attempted: true,
                stdout: stdoutBytes.toString("utf8"),
                stderr: stderrBytes.toString("utf8"),
                stdoutBytes,
                stderrBytes,
                timedOut,
                spawnError: spawnError ? String(spawnError) : null,
                elapsedMs: Date.now() - startedAt,
                backend_process_group_cleaned: childPid != null,
            });
        });
    });
}

// ============================================================
// 세마포어 (concurrency 제어)
// ============================================================

class Semaphore {
    constructor(capacity) {
        this.capacity = capacity;
        this.current = 0;
        this.queue = [];
    }

    acquire() {
        return new Promise((resolve) => {
            if (this.current < this.capacity) {
                this.current++;
                resolve();
            } else {
                this.queue.push(resolve);
            }
        });
    }

    release() {
        this.current--;
        if (this.queue.length > 0) {
            const next = this.queue.shift();
            this.current++;
            next();
        }
    }
}

// ============================================================
// 실시간 메모리 가드 (serena 같은 무거운 episode 신규 실행 전 체크)
// ============================================================

const MEMORY_GUARD_FREE_FLOOR_BYTES = 8 * 1024 * 1024 * 1024; // 8GB (free+inactive)
const MEMORY_GUARD_PAGE_SIZE = 16384; // darwin Apple Silicon page size (bytes)

/**
 * darwin에서 가용 메모리(free+inactive)와 압박 상태를 경량 셸 호출로 읽는다.
 *   - `memory_pressure`: 시스템 압박 레벨(normal/warn/critical) 파싱.
 *   - `vm_stat`: Pages free + Pages inactive → bytes 환산.
 * 실패(명령 없음/파싱 실패)하면 보수적으로 "통과 허용"(가드가 정상 실행을 막지 않도록).
 *
 * @returns {{ freeInactiveBytes:number|null, pressureLevel:string, ok:boolean }}
 */
function readMemoryStatus() {
    let freeInactiveBytes = null;
    let pressureLevel = "unknown";

    // vm_stat: free + inactive page 수 → bytes
    try {
        const vm = spawnSync("vm_stat", [], { encoding: "utf8", timeout: 5000 });
        if (vm.status === 0 && vm.stdout) {
            const freeMatch = vm.stdout.match(/Pages free:\s+(\d+)/);
            const inactiveMatch = vm.stdout.match(/Pages inactive:\s+(\d+)/);
            if (freeMatch && inactiveMatch) {
                const pages = Number(freeMatch[1]) + Number(inactiveMatch[1]);
                freeInactiveBytes = pages * MEMORY_GUARD_PAGE_SIZE;
            }
        }
    } catch {
        /* 보수적 통과 */
    }

    // memory_pressure -Q: 시스템 압박 레벨.
    //   darwin -Q 출력은 "System-wide memory free percentage: NN%" 형식(+ critical/warn 텍스트가
    //   뜰 수도 있음). 명시 텍스트가 있으면 우선, 없으면 free percentage로 환산:
    //   free% < 10 → critical, < 20 → warn, 그 외 normal. (단발성, 보수적 휴리스틱)
    try {
        const mp = spawnSync("memory_pressure", ["-Q"], { encoding: "utf8", timeout: 5000 });
        const text = String(mp.stdout || "") + String(mp.stderr || "");
        const lower = text.toLowerCase();
        if (lower.includes("critical")) {
            pressureLevel = "critical";
        } else if (lower.includes("warn")) {
            pressureLevel = "warn";
        } else {
            const pctMatch = text.match(/free percentage:\s*(\d+)\s*%/i);
            if (pctMatch) {
                const freePct = Number(pctMatch[1]);
                if (freePct < 10) pressureLevel = "critical";
                else if (freePct < 20) pressureLevel = "warn";
                else pressureLevel = "normal";
            } else if (lower.includes("normal")) {
                pressureLevel = "normal";
            }
        }
    } catch {
        /* 보수적 통과 */
    }

    // ok = free+inactive ≥ 8GB AND 압박이 warn/critical 아님.
    // 읽기 실패 시(null/unknown) ok=true로 보수적 통과(가드가 정상 실행을 차단하지 않게).
    const freeOk = freeInactiveBytes == null || freeInactiveBytes >= MEMORY_GUARD_FREE_FLOOR_BYTES;
    const pressureOk = pressureLevel !== "warn" && pressureLevel !== "critical";
    return { freeInactiveBytes, pressureLevel, ok: freeOk && pressureOk };
}

function sleepMs(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * serena(무거운) episode를 **새로 띄우기 전에** 메모리를 확인하고, 부족하면 회복까지 대기한다.
 * free+inactive < 8GB 이거나 압박이 warn/critical이면 polling으로 대기.
 * 이미 backend-sem(serena=3)이 메모리 1차 상한을 잡으므로, 이 가드는 clangd 최악 케이스용
 *   2차 안전장치다(check-then-act race에 대해 airtight하지는 않음 — 보수적 보호).
 *
 * @param {number} pollIntervalMs 폴 간격
 * @param {number} maxWaitMs 최대 대기(이 시간 지나면 강제 진행 — 영구 교착 방지)
 * @returns {{ waited:boolean, waitedMs:number, finalStatus:object }}
 */
async function waitForMemoryBeforeHeavyEpisode(pollIntervalMs = 5000, maxWaitMs = 300000) {
    const startedAt = Date.now();
    let status = readMemoryStatus();
    let waited = false;
    while (!status.ok && Date.now() - startedAt < maxWaitMs) {
        waited = true;
        console.log(
            `[memory-guard:wait] free+inactive=${status.freeInactiveBytes != null ? (status.freeInactiveBytes / 1e9).toFixed(1) + "GB" : "?"} pressure=${status.pressureLevel} → serena 신규 실행 대기`,
        );
        await sleepMs(pollIntervalMs);
        status = readMemoryStatus();
    }
    return { waited, waitedMs: Date.now() - startedAt, finalStatus: status };
}

// ============================================================
// resume-skip: 완료된 episode 판정
// ============================================================

/**
 * episode가 STRICT하게 완료되었는지 판정한다 (resume-skip 게이트).
 * 완료 기준: result_metrics.json 과 harness_judgment.json 이 모두 존재하고
 *   유효한 JSON으로 파싱되며, result_metrics.json에 핵심 필드(arm_id, extraction_status,
 *   harness_valid)가 존재한다. result_metrics.json은 episode의 **마지막**에 기록되므로
 *   done-marker로 적합하다. timeout/partial episode는 이 파일이 없거나 불완전 → 재실행.
 * @returns {object|null} 완료 시 기존 result_metrics(요약 필드), 아니면 null.
 */
function loadCompletedEpisode(episodeDir, resumeIdentity, skipScorer, outRoot = null, schemaPath = null, taskId = null) {
    if (outRoot && schemaPath && taskId) {
        // Even when a later scored attempt can be skipped, its underlying solver artifact
        // remains the canonical source. Verify it first so a corrupted prior solver result
        // cannot be hidden behind the later scorer-only terminal record.
        findReusableSolverAttempt(outRoot, resumeIdentity.solver_identity.sha256);
        const terminal = readCanonicalTerminal(outRoot, resumeIdentity.solver_identity.sha256, resumeIdentity.sha256);
        const trustedMetrics = terminal && validateCompletedArtifacts(terminal.attempt_dir, terminal, resumeIdentity, skipScorer, schemaPath, taskId, episodeDir);
        if (!trustedMetrics) return null;
        return trustedMetrics;
    }
    const metricsPath = path.join(episodeDir, "result_metrics.json");
    const judgmentPath = path.join(episodeDir, "harness_judgment.json");
    const metadataPath = path.join(episodeDir, "episode_metadata.json");
    if (!existsSync(metricsPath) || !existsSync(judgmentPath) || !existsSync(metadataPath)) return null;
    let metrics;
    let judgment;
    let metadata;
    try {
        metrics = readJson(metricsPath);
        judgment = readJson(judgmentPath);
        metadata = readJson(metadataPath);
    } catch {
        return null; // 파싱 실패 → 불완전 → 재실행
    }
    // 핵심 필드 존재 확인 (부분 기록 방어)
    if (
        metrics == null ||
        typeof metrics.arm_id !== "string" ||
        typeof metrics.extraction_status !== "string" ||
        metrics.harness_valid !== true ||
        metrics.extraction_status !== "success" ||
        metrics.mutation_guard_status !== "clean" ||
        judgment == null ||
        typeof judgment.episode_id !== "string" ||
        metadata?.resume_identity?.sha256 !== resumeIdentity?.sha256 ||
        metadata?.scoring?.skip_scorer !== skipScorer ||
        (skipScorer ? metadata?.scoring?.status !== "skipped" : metadata?.scoring?.status !== "completed") ||
        !metadata?.process?.stdout?.exists || !metadata?.process?.stderr?.exists ||
        fileDigest(path.join(episodeDir, "stdout.txt")).sha256 !== metadata.process.stdout.sha256 ||
        fileDigest(path.join(episodeDir, "stderr.txt")).sha256 !== metadata.process.stderr.sha256 ||
        !existsSync(path.join(episodeDir, "raw_answer.txt")) ||
        !existsSync(path.join(episodeDir, "tool_events.json")) ||
        !existsSync(path.join(episodeDir, "scorer_output.json")) ||
        !metadata?.artifact_seal
    ) {
        return null;
    }
    for (const [artifactName, expected] of Object.entries(metadata.artifact_seal)) {
        const observed = fileDigest(path.join(episodeDir, artifactName));
        if (!expected?.exists || observed.sha256 !== expected.sha256 || observed.bytes !== expected.bytes) return null;
    }
    const scorerOutput = readJson(path.join(episodeDir, "scorer_output.json"));
    const answerHash = fileDigest(path.join(episodeDir, "raw_answer.txt")).sha256;
    if (metrics.answer_sha256 !== answerHash || judgment.answer_sha256 !== answerHash || metrics.episode_id !== judgment.episode_id) return null;
    if (!skipScorer && (!validScorerOutput(scorerOutput, schemaPath ? readJson(schemaPath) : null, taskId, answerHash) || metrics.scorer_score !== scorerOutput.score || judgment.scorer_score !== scorerOutput.score)) return null;
    if (skipScorer && scorerOutput?.status !== "skipped") return null;
    return metrics;
}

async function prepareEpisodeForResume(ep, config) {
    const preflightCache = config.preflightCache ?? (config.preflightCache = new Map());
    const preflightSemaphore = config.preflightSemaphore ?? (config.preflightSemaphore = new Semaphore(4));
    const arm = ep.arm_id ?? ep.arm;
    const round = canonicalRound(ep.round);
    const armDef = config.armConfig.arms.find((candidate) => candidate.arm_id === arm);
    if (!armDef) throw new Error(`arm_id not found: ${arm}`);
    const taskDef = config.manifest.tasks[ep.codebase];
    if (!taskDef) throw new Error(`codebase not found in manifest: ${ep.codebase}`);
    const targetRoot = resolveWorkspacePath(taskDef.code_root, config.workspaceRoot);
    const publicQuestionPath = resolveWorkspacePath(taskDef.public_question, config.workspaceRoot);
    const privateAnswerKeyPath = resolveWorkspacePath(taskDef.private_answer_key, config.workspaceRoot);
    const schemaPath = path.join(config.schemaDir, `scoring_schema.${ep.codebase}.json`);
    requireDirectory("target root", targetRoot);
    requireFile("public question", publicQuestionPath);
    requireFile("private answer key", privateAnswerKeyPath);
    requireFile("scoring schema", schemaPath);
    if (!config.skipScorer) requireFile("scorer", config.scorerPath);
    const targetRootRealpath = realpathSync(targetRoot);
    const runtime = await cachedIdentity(config, "runtimeIdentityCache", armDef.runtime, () => runtimeIdentity(armDef.runtime));
    const fixtureIdentity = await cachedIdentity(config, "targetIdentityCache", targetRootRealpath, () => targetIdentity(targetRootRealpath));
    const codemapIdentity = armDef.backend === "codemap" ? config.codemapBinary : null;
    const fixtureContractValue = fixtureContract(taskDef, ep.codebase, armDef, codemapIdentity);
    const fixtureContractSha256 = sha256(JSON.stringify(fixtureContractValue));
    const preflightKey = JSON.stringify({ root: fixtureIdentity.realpath, runtime, codemap: codemapIdentity, fixture_contract_sha256: fixtureContractSha256 });
    let preflight = preflightCache.get(preflightKey);
    if (!preflight) {
        preflight = (async () => {
            await preflightSemaphore.acquire();
            try {
                return await preflightFixture({ taskDef, taskId: ep.codebase, targetRoot, armDef, codemapBin: config.codemapBin, identity: fixtureIdentity });
            } finally {
                preflightSemaphore.release();
            }
        })();
        preflightCache.set(preflightKey, preflight);
        preflight.catch(() => { if (preflightCache.get(preflightKey) === preflight) preflightCache.delete(preflightKey); });
    }
    const fixture = await preflight;
    const contract = scoringContract({ scorerPath: config.scorerPath, schemaPath, privateAnswerKeyPath, judgeModel: config.judgeModel, printCmd: config.printCmd });
    const prompt = readFileSync(publicQuestionPath, "utf8");
    const resumeIdentity = buildResumeIdentity({ armDef, promptPath: publicQuestionPath, prompt, config, contract, fixtureIdentity: fixture.identity, evaluationContract: taskDef.evaluation, runtime, episode: { arm_id: arm, codebase: ep.codebase, round } });
    const episodeDir = path.join(config.outRoot, arm, ep.codebase, `round-${round}`);
    return { armDef, episodeDir, resumeIdentity, fixture, runtime, targetRoot, publicQuestionPath, privateAnswerKeyPath, schemaPath };
}

// ============================================================
// episode 실행 (핵심)
// ============================================================

/**
 * @param {object} ep episode 정의
 * @param {object} config 런 설정
 * @param {object} [hooks] 스케줄러 훅
 * @param {() => void} [hooks.releaseSlots] solver 답변 추출 직후(scorer 실행 전) 호출되어
 *   codebase 락·백엔드 세마포어·전역 슬롯을 먼저 release한다. scorer는 그 밖에서 실행되어
 *   무거운 슬롯을 채점 동안 점유하지 않는다. **호출부에서 멱등 처리**(중복 release 방지).
 * @param {() => {count:number, backends:object}} [hooks.coTenancySnapshot] 이 episode 실행 시작
 *   시점의 동시 실행 episode 수 + 백엔드 구성 스냅샷을 반환.
 */
async function runEpisode(ep, config, hooks = {}) {
    // episodes.json은 arm_id 또는 arm 필드를 허용
    const { arm: armField, arm_id, codebase, round: roundInput, sequence_ordinal: sequenceOrdinal = null } = ep;
    const arm = arm_id ?? armField;
    const round = canonicalRound(roundInput);
    const releaseSlots = typeof hooks.releaseSlots === "function" ? hooks.releaseSlots : () => {};
    const coTenancySnapshot = typeof hooks.coTenancySnapshot === "function" ? hooks.coTenancySnapshot : () => null;
    const { armConfig, manifest, scorerPath, schemaDir, outRoot, timeoutMs, judgeModel, printCmd, skipScorer, codemapBin, workspaceRoot } = config;

    const armDef = armConfig.arms.find((a) => a.arm_id === arm);
    if (!armDef) throw new Error(`arm_id not found: ${arm}`);

    const taskDef = manifest.tasks[codebase];
    if (!taskDef) throw new Error(`codebase not found in manifest: ${codebase}`);

    const targetRoot = resolveWorkspacePath(taskDef.code_root, workspaceRoot);
    const publicQuestionPath = resolveWorkspacePath(taskDef.public_question, workspaceRoot);
    const privateAnswerKeyPath = resolveWorkspacePath(taskDef.private_answer_key, workspaceRoot);
    const schemaPath = path.join(schemaDir, `scoring_schema.${codebase}.json`);

    console.log(
        `[runner:paths] workspace_root=${workspaceRoot} target_root=${targetRoot} public_question=${publicQuestionPath} private_answer_key=${privateAnswerKeyPath} schema=${schemaPath}`,
    );
    requireDirectory("target root", targetRoot);
    requireFile("public question", publicQuestionPath);
    requireFile("private answer key", privateAnswerKeyPath);
    requireFile("scoring schema", schemaPath);
    if (!skipScorer) requireFile("scorer", scorerPath);

    const prompt = readFileSync(publicQuestionPath, "utf8");
    const episodeId = `${arm}__${codebase}__round-${round}`;
    const episodeDir = path.join(outRoot, arm, codebase, `round-${round}`);
    const contract = scoringContract({ scorerPath, schemaPath, privateAnswerKeyPath, judgeModel, printCmd });
    const prepared = hooks.prepared ?? await prepareEpisodeForResume(ep, config);
    const fixture = prepared.fixture;
    const solverRuntime = prepared.runtime;
    const resumeIdentity = prepared.resumeIdentity;

    // --- resume-skip: 완료된 episode면 재실행 없이 건너뛴다 (P9 6~10시간 중단·재개 대비) ---
    if (!config.force) {
        const completed = loadCompletedEpisode(episodeDir, resumeIdentity, skipScorer, outRoot, schemaPath, codebase);
        if (completed) {
            console.log(`[episode:skip] ${episodeId} (already complete; --force로 재실행)`);
            return {
                arm_id: arm,
                runtime: armDef.runtime,
                codebase,
                round,
                wall_time_s: completed.wall_time_s ?? null,
                extraction_status: completed.extraction_status,
                scorer_score: completed.scorer_score ?? null,
                mutation_guard_status: completed.mutation_guard_status ?? "unknown",
                harness_valid: completed.harness_valid,
                episode_dir: episodeDir,
                skipped: true,
            };
        }
    }

    // A sealed solver result is immutable even when its first scorer failed. Later scored
    // invocations must reuse it; retry only applies when no reusable solver exists.
    const reusableSolver = findReusableSolverAttempt(outRoot, resumeIdentity.solver_identity.sha256);
    if (config.force) {
        const completed = loadCompletedEpisode(episodeDir, resumeIdentity, skipScorer, outRoot, schemaPath, codebase);
        if (completed) {
            throw new Error(`[runner:force] ${episodeId} already has a valid successful final identity; choose a fresh --out-root/run id instead of appending duplicate evidence`);
        }
    }
    if (skipScorer && reusableSolver) {
        throw new Error(`[runner:reuse] ${episodeId} already has a sealed solver artifact; --skip-scorer cannot append a duplicate solver success`);
    }
    const reusableSolverAttempt = !skipScorer ? reusableSolver : null;
    // The solver identity is single-use by default: a prior started record (including a
    // crash with no terminal record) or a terminal failure must be explicitly investigated,
    // never silently retried by a resumed benchmark invocation.
    if (!reusableSolverAttempt && !config.allowRetry && hasPriorTerminalOrStartedAttempt(outRoot, resumeIdentity.solver_identity.sha256)) {
        throw new Error(`[runner:retry] solver identity already attempted; --allow-retry is required: ${resumeIdentity.solver_identity.sha256}`);
    }
    const claimPath = acquireEpisodeClaim(outRoot, resumeIdentity.solver_identity.sha256);
    let terminalAppended = false;
    let terminalLedgerBase = null;
    let terminalAttemptId = null;
    try {

    mkdirSync(episodeDir, { recursive: true });

    // --- co-tenancy: 이 episode 실행 시작 시점의 동시 실행 구성 스냅샷 ---
    // wall_time 부풀림 보정용. 동시 실행 episode 수 + 백엔드 구성을 기록한다.
    const coTenancyAtStart = coTenancySnapshot();

    console.log(`[episode:start] ${episodeId}`);

    // --- mutation guard: before snapshot ---
    const snapshotBefore = snapshotTargetRoot(targetRoot);
    writeJson(path.join(episodeDir, "mutation_guard_before.json"), snapshotBefore);

    // --- 명령 빌드 ---
    const plan = buildEpisodeCommand(armDef, targetRoot, prompt, episodeDir, codemapBin);
    const exactCommandStr = commandLine(plan.command, plan.args);
    writeText(path.join(episodeDir, "exact_command.txt"), exactCommandStr + "\n");
    writeJson(path.join(episodeDir, "exact_command.json"), {
        command: plan.command,
        args: plan.args,
        cwd: plan.cwd,
        command_line: exactCommandStr,
        contains_bare: plan.args.includes("--bare"),
        setting_sources_used: plan.args.includes("--setting-sources"),
        runtime: armDef.runtime,
        backend: armDef.backend,
    });

    if (printCmd) {
        console.log(`[episode:cmd] ${exactCommandStr}`);
    }

    // --- 실행 ---
    const attemptId = randomUUID();
    terminalAttemptId = attemptId;
    const attemptDir = path.join(outRoot, "attempts", attemptId);
    mkdirSync(attemptDir, { recursive: true });
    const attemptStdoutPath = path.join(attemptDir, "stdout.txt");
    const attemptStderrPath = path.join(attemptDir, "stderr.txt");
    writeFileSync(attemptStdoutPath, "");
    writeFileSync(attemptStderrPath, "");
    const ledgerBase = {
        identity_schema_version: IDENTITY_SCHEMA_VERSION,
        attempt_id: attemptId,
        attempt_dir: attemptDir,
        episode_id: episodeId,
        arm_id: arm,
        codebase,
        round,
        sequence_ordinal: sequenceOrdinal,
        provider: armDef.model?.split("/")[0] ?? null,
        model: armDef.model,
        command_sha256: sha256(exactCommandStr),
        runtime: solverRuntime,
        repository: fixture.identity,
        solver_identity_sha256: resumeIdentity.solver_identity.sha256,
        final_identity_sha256: resumeIdentity.sha256,
    };
    terminalLedgerBase = ledgerBase;
    appendAttemptLedger(outRoot, { ...ledgerBase, event: reusableSolverAttempt ? "scorer_started" : "started", timestamp: new Date().toISOString(), reused_solver_attempt_id: reusableSolverAttempt?.attempt_id ?? null });
    writeJson(path.join(attemptDir, "attempt_metadata.json"), { ...ledgerBase, fixture_proof: fixture.proof, resume_identity: resumeIdentity });
    const solveStartedAt = new Date();
    const processResult = reusableSolverAttempt
        ? (() => {
            const prior = readJson(path.join(reusableSolverAttempt.attempt_dir, "process_result.json"));
            copyFileSync(path.join(reusableSolverAttempt.attempt_dir, "stdout.txt"), attemptStdoutPath);
            copyFileSync(path.join(reusableSolverAttempt.attempt_dir, "stderr.txt"), attemptStderrPath);
            return { exitCode: prior.exitCode, signal: prior.signal, timedOut: prior.timedOut, spawnError: prior.spawnError, elapsedMs: prior.elapsedMs, wall_time_s: prior.wall_time_s, spawn_attempted: false, reused_solver_attempt_id: reusableSolverAttempt.attempt_id, stdout: readFileSync(attemptStdoutPath, "utf8"), stderr: readFileSync(attemptStderrPath, "utf8"), stdoutBytes: readFileSync(attemptStdoutPath), stderrBytes: readFileSync(attemptStderrPath), backend_process_group_cleaned: prior.backend_process_group_cleaned ?? false };
        })()
        : await runProcess(plan, timeoutMs, { stdout: attemptStdoutPath, stderr: attemptStderrPath }, hooks.abortSignal ?? null);
    const solveFinishedAt = new Date();
    const solveElapsedMs = processResult.elapsedMs;
    const wallTimeS = processResult.elapsedMs / 1000;

    console.log(
        `[episode:done] ${episodeId} exit=${processResult.exitCode} wall_time_s=${wallTimeS.toFixed(1)} timed_out=${processResult.timedOut}`,
    );

    copyFileSync(attemptStdoutPath, path.join(episodeDir, "stdout.txt"));
    copyFileSync(attemptStderrPath, path.join(episodeDir, "stderr.txt"));
    writeJson(path.join(episodeDir, "process_result.json"), {
        exitCode: processResult.exitCode,
        signal: processResult.signal,
        timedOut: processResult.timedOut,
        spawnError: processResult.spawnError,
        elapsedMs: processResult.elapsedMs,
        wall_time_s: wallTimeS,
    });

    // --- mutation guard: after snapshot ---
    const snapshotAfter = snapshotTargetRoot(targetRoot);
    writeJson(path.join(episodeDir, "mutation_guard_after.json"), snapshotAfter);

    // --- mutation 비교 (git-tracked 기반 rule) ---
    const { violations: mutationViolations, allowedBackendWrites } = diffSnapshots(
        snapshotBefore,
        snapshotAfter,
        targetRoot,
    );
    // violation = git-tracked 소스 변경(source_mutation). git-ignored(target/ 등) + backend 산출물은 artifact_churn(허용).
    const mutationGuardStatus = mutationViolations.length === 0 ? "clean" : "violation";
    const mutationGuard = {
        status: mutationGuardStatus,
        classification: "git_tracked_source_only",
        violations: mutationViolations,
        allowed_backend_writes: allowedBackendWrites,
        allowed_backend_write_count: allowedBackendWrites.length,
        before_count: snapshotBefore.length,
        after_count: snapshotAfter.length,
        allowed_prefixes: BACKEND_ALLOWED_PREFIXES,
        policy: "git-tracked source only: violation(source_mutation) = git-tracked 소스의 신규/크기/ mtime-only 변경. artifact_churn(허용·기록) = git-ignored 경로(target/ 빌드트리, node_modules 등) + backend 산출물(.codemap/.serena/.codegraph/.git, 예: .serena/cache, .codegraph/*.db-wal/-shm/pid). P7b 실측 근거: bash:false인데도 target/ 821건 변경 관측 → 동시 실행 serena episode의 rust-analyzer가 공유 target-root의 git-ignored target/ fingerprint를 touch한 cross-contamination. '소스 트리 변경'의 충실한 해석은 'git-tracked 소스 변경'. P7의 over-broad 규칙이 target/를 false harness_invalid로 swept-in 했던 것을 교정.",
    };
    writeJson(path.join(episodeDir, "mutation_guard.json"), mutationGuard);

    // --- runtime별 답변 추출 ---
    let rawAnswer = "";
    let tokens = null;
    let tokenSteps = [];
    let toolEvents = [];
    let extractionStatus = "unknown";

    const runtime = armDef.runtime;

    if (runtime === "claude-sonnet") {
        const extracted = extractClaudeStreamJson(processResult.stdout);
        rawAnswer = extracted.rawAnswer ?? "";
        toolEvents = extracted.toolEvents;
        tokens = extractTokensFromClaudeUsage(extracted.usageObj);
        extractionStatus = rawAnswer.length > 0 ? "success" : "empty";
    } else if (runtime === "codex-gpt54") {
        const extracted = extractCodexOutput(processResult.stdout, plan.lastMessagePath);
        rawAnswer = extracted.rawAnswer ?? "";
        toolEvents = extracted.toolEvents;
        tokens = extracted.tokens;
        extractionStatus = rawAnswer.length > 0 ? "success" : "empty";
    } else if (runtime.startsWith("opencode-")) {
        const extracted = extractOpencodeJsonOutput(processResult.stdout);
        rawAnswer = extracted.rawAnswer ?? "";
        toolEvents = extracted.toolEvents;
        tokens = extracted.tokens;
        tokenSteps = extracted.stepTokens;
        extractionStatus = rawAnswer.length > 0 ? "success" : "empty";
    } else {
        extractionStatus = "unsupported_runtime";
    }

    // process 에러/timeout 시 extraction_status를 명시
    if (processResult.timedOut) extractionStatus = "timeout";
    else if (processResult.exitCode !== 0 && extractionStatus !== "success") extractionStatus = "process_error";

    const rawAnswerPath = path.join(episodeDir, "raw_answer.txt");
    writeText(rawAnswerPath, rawAnswer);

    // --- 도구 메트릭 ---
    const { toolCallDistribution, toolResultBytesByTool } = summarizeToolMetrics(toolEvents);
    const assignedBackendToolBytes = calcAssignedBackendToolBytes(armDef.backend, toolResultBytesByTool);
    const backendExercised = calcBackendExercised(armDef.backend, toolCallDistribution);

    writeJson(path.join(episodeDir, "tool_events.json"), toolEvents);

    // Seal the solver stage before scoring. This record is intentionally independent of the
    // final terminal: scorer process/parse/contract failures can be recovered by scorer-only
    // resume without another solver spawn.
    const solverReusable = !reusableSolverAttempt &&
        !processResult.timedOut && processResult.exitCode === 0 &&
        mutationGuardStatus === "clean" && extractionStatus === "success" && rawAnswer.trim().length > 0;
    if (solverReusable) {
        const canonicalSolverArtifacts = buildArtifactSeal(episodeDir, SOLVER_REUSABLE_ARTIFACTS);
        appendAttemptLedger(outRoot, {
            ...ledgerBase,
            event: "solver_reusable",
            timestamp: new Date().toISOString(),
            answer_sha256: sha256(rawAnswer),
            canonical_solver_artifacts: canonicalSolverArtifacts,
        });
    }

    // --- 채점 잠금 밖 실행: solver 답변 추출이 끝났으므로 codebase 락·백엔드 세마포어·전역 슬롯을
    //     먼저 release한다. scorer(judge)는 target-root에 접근하지 않으므로 무거운 슬롯을
    //     점유하지 않고 이 밖에서 실행된다. releaseSlots는 호출부에서 멱등 처리됨(중복 release 방지). ---
    releaseSlots();

    // --- scorer.mjs 호출 (잠금 밖) ---
    let scorerOutput = null;
    let scorerScore = null;
    let judgeStatus = skipScorer ? "skipped" : "not_started";
    let judgeStartedAt = null; let judgeFinishedAt = null;
    let judgeElapsedMs = null;

    if (skipScorer) {
        scorerOutput = { status: judgeStatus, reason: existsSync(scorerPath) ? "skip_scorer" : "scorer_missing" };
        writeJson(path.join(episodeDir, "scorer_output.json"), scorerOutput);
    } else if (
        extractionStatus === "success" &&
        rawAnswer.trim().length > 0 &&
        existsSync(schemaPath) &&
        existsSync(privateAnswerKeyPath)
    ) {
        const scorerOutPath = path.join(episodeDir, "scorer_output.json");
        const scorerArgs = [
            scorerPath,
            "--raw-answer",
            rawAnswerPath,
            "--schema",
            schemaPath,
            "--answer-key",
            privateAnswerKeyPath,
            "--out",
            scorerOutPath,
            "--judge-model",
            judgeModel,
        ];
        if (printCmd) scorerArgs.push("--print-cmd");

        judgeStartedAt = new Date();
        console.log(`[scorer:start] ${episodeId}`);
        const scorerResult = spawnSync("node", scorerArgs, {
            encoding: "utf8",
            maxBuffer: 32 * 1024 * 1024,
            timeout: 300_000, // scorer(judge) 5분 상한
        });
        judgeFinishedAt = new Date();
        judgeElapsedMs = judgeFinishedAt.getTime() - judgeStartedAt.getTime();

        if (scorerResult.status === 0 && existsSync(scorerOutPath)) {
            try {
                scorerOutput = readJson(scorerOutPath);
                scorerScore = scorerOutput.score ?? null;
                if (!validScorerOutput(scorerOutput, readJson(schemaPath), codebase, sha256(rawAnswer))) {
                    judgeStatus = "result_contract_error";
                    scorerScore = null;
                    console.error(`[scorer:contract_error] ${episodeId}`);
                } else {
                    judgeStatus = "completed";
                    console.log(`[scorer:done] ${episodeId} score=${scorerScore}`);
                }
            } catch {
                console.error(`[scorer:parse_error] ${episodeId}`);
                judgeStatus = "result_parse_error";
            }
        } else {
            console.error(
                `[scorer:fail] ${episodeId} exit=${scorerResult.status} stderr=${(scorerResult.stderr || "").slice(0, 400)}`,
            );
            scorerOutput = {
                status: "scorer_failed",
                exit_code: scorerResult.status,
                stderr: (scorerResult.stderr || "").slice(0, 800),
            };
            judgeStatus = "scorer_failed";
            writeJson(path.join(episodeDir, "scorer_output.json"), scorerOutput);
        }
    } else {
        scorerOutput = {
            status: "not_scored",
            reason:
                extractionStatus !== "success"
                    ? `extraction_status=${extractionStatus}`
                    : "schema_or_answer_key_missing",
        };
        judgeStatus = "not_run_solver_or_inputs_unavailable";
        writeJson(path.join(episodeDir, "scorer_output.json"), scorerOutput);
    }

    // --- harness validity ---
    const harnessValid =
        !processResult.timedOut &&
        processResult.exitCode === 0 &&
        mutationGuardStatus === "clean" &&
        extractionStatus === "success";
    const backendStatus = processResult.timedOut || processResult.exitCode !== 0
        ? "failed"
        : backendExercised ? "exercised" : "unobserved";
    const evaluationObservation = buildEvaluationObservation({
        evaluation: taskDef.evaluation,
        toolEvents,
        solveStartedAt,
        backendExercised,
        processResult,
        judgeStatus,
        scorerScore,
        scorerOutput,
        schemaPath,
    });

    const harnessJudgment = {
        episode_id: episodeId,
        arm_id: arm,
        runtime: armDef.runtime,
        codebase,
        round,
        harness_valid: harnessValid,
        exit_code: processResult.exitCode,
        timed_out: processResult.timedOut,
        spawn_error: processResult.spawnError,
        wall_time_s: wallTimeS,
        judge_status: judgeStatus,
        extraction_status: extractionStatus,
        mutation_guard_status: mutationGuardStatus,
        mutation_violations_count: mutationViolations.length,
        scorer_score: scorerScore,
        answer_sha256: sha256(rawAnswer),
        backend_exercised: backendExercised,
        backend_status: backendStatus,
        assigned_backend_tool_bytes: assignedBackendToolBytes,
        contains_bare: plan.args.includes("--bare"),
        cwd: plan.cwd,
        cwd_is_target_root: realpathSync(plan.cwd) === fixture.identity.realpath,
    };
    writeJson(path.join(episodeDir, "harness_judgment.json"), harnessJudgment);

    // --- result_metrics.json ---
    const answerSha256 = sha256(rawAnswer);
    const resultMetrics = {
        episode_id: episodeId,
        arm_id: arm,
        runtime: armDef.runtime,
        model: armDef.model,
        model_label: armDef.model_label,
        backend: armDef.backend,
        codebase,
        round,
        wall_time_s: wallTimeS,
        judge_status: judgeStatus,
        tokens,
        tool_call_distribution: toolCallDistribution,
        tool_result_bytes_by_tool: toolResultBytesByTool,
        assigned_backend_tool_bytes: assignedBackendToolBytes,
        backend_exercised: backendExercised,
        backend_status: backendStatus,
        evaluation_observation: evaluationObservation,
        extraction_status: extractionStatus,
        answer_sha256: answerSha256,
        scorer_score: scorerScore,
        mutation_guard_status: mutationGuardStatus,
        harness_valid: harnessValid,
        // co-tenancy: 이 episode 실행 시작 시점 동시 실행 구성(시작 스냅샷). wall_time 부풀림 보정용.
        //   count = 동시 in-flight episode 수, backends = 백엔드별 동시 실행 수. episode 진행 중
        //   변동하므로 "시작 시점 스냅샷"임을 명시(상수 아님).
        co_tenancy: coTenancyAtStart,
        // 백엔드 MCP/언어서버 프로세스 그룹 정리 시도 여부(누적 방지).
        backend_process_group_cleaned: processResult.backend_process_group_cleaned ?? false,
    };
    writeJson(path.join(episodeDir, "result_metrics.json"), resultMetrics);
    const sealedArtifacts = REQUIRED_ARTIFACTS;
    const artifactSeal = buildArtifactSeal(episodeDir, sealedArtifacts);
    writeJson(path.join(episodeDir, "artifact_manifest.json"), { version: 1, artifacts: artifactSeal });
    const episodeMetadata = {
        recorded_at: new Date().toISOString(),
        attempt: { attempt_id: attemptId, attempt_dir: attemptDir },
        resume_identity: resumeIdentity,
        task: {
            codebase,
            public_question: { path: publicQuestionPath, ...fileDigest(publicQuestionPath) },
            fixture_proof: fixture.proof,
            evaluation_contract: taskDef.evaluation ? { value: taskDef.evaluation, sha256: evaluationContractSha256(taskDef.evaluation) } : null,
        },
        solver: {
            arm_id: arm,
            model: armDef.model,
            runtime: armDef.runtime,
            runtime_identity: solverRuntime,
            backend: armDef.backend,
            timeout_ms: timeoutMs,
            command: { command: plan.command, args: plan.args, cwd: plan.cwd },
            codemap_binary: config.codemapBinary,
            opencode_config: plan.opencodeConfigPath ? { path: plan.opencodeConfigPath, ...fileDigest(plan.opencodeConfigPath) } : null,
        },
        scoring: {
            skip_scorer: skipScorer,
            status: judgeStatus,
            contract,
            started_at: judgeStartedAt?.toISOString() ?? null,
            finished_at: judgeFinishedAt?.toISOString() ?? null,
            elapsed_ms: judgeElapsedMs,
        },
        process: {
            runner_started_at: config.runnerStartedAt,
            solve_started_at: solveStartedAt.toISOString(),
            solve_finished_at: solveFinishedAt.toISOString(),
            elapsed_ms: solveElapsedMs,
            spawn_attempted: processResult.spawn_attempted,
            reused_solver_attempt_id: processResult.reused_solver_attempt_id ?? null,
            exit_code: processResult.exitCode,
            signal: processResult.signal,
            timed_out: processResult.timedOut,
            spawn_error: processResult.spawnError,
            stdout: fileDigest(path.join(episodeDir, "stdout.txt")),
            stderr: fileDigest(path.join(episodeDir, "stderr.txt")),
        },
        token_accounting: armDef.runtime.startsWith("opencode-")
            ? { source: "stdout.txt", contract: "opencode JSON step_finish tokens are per-step values; sum each field", step_tokens: tokenSteps, aggregate: tokens }
            : { source: armDef.runtime, aggregate: tokens },
        artifact_seal: artifactSeal,
    };
    writeJson(path.join(episodeDir, "episode_metadata.json"), episodeMetadata);
    for (const artifactName of [...sealedArtifacts, "artifact_manifest.json", "episode_metadata.json"]) {
        const artifactPath = path.join(episodeDir, artifactName);
        if (existsSync(artifactPath)) copyFileSync(artifactPath, path.join(attemptDir, artifactName));
    }
    const terminalEvent = harnessValid && (skipScorer || judgeStatus === "completed") ? "completed" : "terminal_failure";
    const attemptTerminalEvents = ledgerEventsForIdentity(outRoot, resumeIdentity.solver_identity.sha256).filter((event) => event.attempt_id === attemptId && (event.event === "completed" || event.event === "terminal_failure"));
    if (attemptTerminalEvents.length !== 0) throw new Error(`[runner:ledger] duplicate terminal event refused: ${attemptId}`);
    const canonicalArtifacts = buildArtifactSeal(attemptDir, REQUIRED_ARTIFACTS);
    appendAttemptLedger(outRoot, { ...ledgerBase, event: terminalEvent, timestamp: new Date().toISOString(), exit_code: processResult.exitCode, timed_out: processResult.timedOut, extraction_status: extractionStatus, harness_valid: harnessValid, scoring_status: judgeStatus, answer_sha256: answerSha256, canonical_artifacts: canonicalArtifacts, artifact_manifest_sha256: fileDigest(path.join(attemptDir, "artifact_manifest.json")).sha256, metadata_sha256: fileDigest(path.join(attemptDir, "episode_metadata.json")).sha256 });
    terminalAppended = true;
    if (terminalEvent === "terminal_failure") {
        throw new Error(`[runner:terminal_failure] ${episodeId} scoring_status=${judgeStatus}`);
    }

    return {
        arm_id: arm,
        runtime: armDef.runtime,
        codebase,
        round,
        wall_time_s: wallTimeS,
        extraction_status: extractionStatus,
        scorer_score: scorerScore,
        mutation_guard_status: mutationGuardStatus,
        harness_valid: harnessValid,
        episode_dir: episodeDir,
        skipped: false,
    };
    } catch (error) {
        if (terminalLedgerBase && !terminalAppended) {
            try {
                appendAttemptLedger(outRoot, { ...terminalLedgerBase, event: "terminal_failure", timestamp: new Date().toISOString(), failure_kind: "runner_exception", failure_message: String(error).slice(0, 400), attempt_id: terminalAttemptId ?? terminalLedgerBase.attempt_id });
                terminalAppended = true;
            } catch (ledgerError) {
                console.error(`[runner:ledger_failure] ${String(ledgerError).slice(0, 400)}`);
            }
        }
        throw error;
    } finally {
        releaseEpisodeClaim(claimPath);
    }
}

// ============================================================
// 배치 실행 (동시성 제어)
// ============================================================

// ------------------------------------------------------------
// 동시성 모델 상수 (사용자 합의)
//   - 전역 in-flight 상한 = 10
//   - serena: 전역 세마포어 3 + codebase당 상호배제 1 (자원 + mutation-guard 정확도)
//   - codegraph: 전역 세마포어 4 (같은 codebase 동시 허용 — 테스트로 안전 확인, per-codebase 락 불필요)
//   - codemap, no-mcp(light): 백엔드별 상한 없음(전역 10만 적용)
//   - claude 런타임: 병렬 허용(claude_serial 강제 제거). 모든 런타임 동일하게 전역+백엔드 상한만 따름.
//   동시성은 episode의 backend/runtime에서 유도(미리 박힌 concurrency_group 미사용).
// ------------------------------------------------------------
const CONCURRENCY = {
    GLOBAL_CAP: 10,
    SERENA_GLOBAL: 3,
    SERENA_PER_CODEBASE: 1,
    CODEGRAPH_GLOBAL: 4,
};

/**
 * lazy Map<codebase, Semaphore(1)>: serena의 codebase당 상호배제 락.
 * 같은 codebase(= 같은 code_root, 같은 target/ 빌드 트리)에서 serena episode를 1개로 제한해
 * rust-analyzer cargo churn이 동시 실행 다른 episode로 귀속되는 shared-root race
 * (P7b harness_invalid 근본 원인)를 차단한다. serena에만 적용(codegraph/light는 불필요).
 */
function makeSerenaCodebaseLockFactory() {
    const locks = new Map();
    return (codebase) => {
        let lock = locks.get(codebase);
        if (!lock) {
            lock = new Semaphore(CONCURRENCY.SERENA_PER_CODEBASE);
            locks.set(codebase, lock);
        }
        return lock;
    };
}

/**
 * episode의 backend를 light/heavy 분류.
 *   serena = heavy(메모리 가드 + codebase 락 적용)
 *   codegraph = backend-sem만
 *   codemap, no-mcp = light(전역만)
 */
function backendOf(armDef, ep) {
    return armDef?.backend ?? ep.backend ?? "no-mcp";
}

async function runBatch(episodes, config, _legacyNonClaudeCap) {
    const armConfigArms = config.armConfig.arms;

    // 전역 슬롯 + 백엔드별 세마포어
    const globalSem = new Semaphore(CONCURRENCY.GLOBAL_CAP);
    const serenaSem = new Semaphore(CONCURRENCY.SERENA_GLOBAL);
    const codegraphSem = new Semaphore(CONCURRENCY.CODEGRAPH_GLOBAL);
    const serenaCodebaseLockFor = makeSerenaCodebaseLockFactory();

    // mock seam: 테스트가 가짜 실행 구현을 주입할 수 있게 한다(무거운 실행 없이 스케줄러 단위 검증).
    const runImpl = typeof config.runEpisodeImpl === "function" ? config.runEpisodeImpl : runEpisode;
    const batchAbortController = new AbortController();
    function abortedResult(ep, armDef, error) {
        return { arm_id: ep.arm_id ?? ep.arm, runtime: armDef?.runtime ?? "unknown", codebase: ep.codebase, round: ep.round, wall_time_s: null, extraction_status: "runner_error", scorer_score: null, mutation_guard_status: "unknown", harness_valid: false, episode_dir: null, skipped: false, error: String(error) };
    }
    function throwIfBatchAborted() {
        if (batchAbortController.signal.aborted) throw new Error("[runner:batch_aborted] a peer episode failed before this episode could start");
    }

    // co-tenancy 레지스트리: 현재 in-flight인 episode의 백엔드 구성을 추적.
    // 각 episode 실행 시작 시점 스냅샷을 result_metrics에 기록(wall_time 부풀림 보정용).
    const activeBackends = new Map(); // episodeId → backend
    function coTenancySnapshot() {
        const backends = {};
        for (const b of activeBackends.values()) backends[b] = (backends[b] || 0) + 1;
        return { count: activeBackends.size, backends };
    }

    const tasks = episodes.map((ep, sequenceOrdinal) => {
        const armDef = armConfigArms.find((a) => a.arm_id === ep.arm_id);
        const backend = backendOf(armDef, ep);
        const isSerena = backend === "serena";
        const isCodegraph = backend === "codegraph";
        const episodeId = `${ep.arm_id}__${ep.codebase}__round-${ep.round}`;
        const codebaseLock = isSerena ? serenaCodebaseLockFor(ep.codebase) : null;
        const backendSem = isSerena ? serenaSem : isCodegraph ? codegraphSem : null;

        return async () => {
            // Fast skip remains outside execution slots, but expensive preflight is shared
            // by canonical root/head/runtime/binary and bounded by preflightSemaphore.
            let prepared;
            try {
                throwIfBatchAborted();
                prepared = runImpl === runEpisode ? await prepareEpisodeForResume(ep, config) : null;
                throwIfBatchAborted();
            } catch (error) {
                batchAbortController.abort(error);
                return abortedResult(ep, armDef, error);
            }
            if (!config.force && prepared) {
                const completed = loadCompletedEpisode(prepared.episodeDir, prepared.resumeIdentity, config.skipScorer, config.outRoot, path.join(config.schemaDir, `scoring_schema.${ep.codebase}.json`), ep.codebase);
                if (completed) {
                    console.log(`[episode:skip] ${episodeId} (already complete; --force로 재실행)`);
                    return {
                        arm_id: ep.arm_id ?? ep.arm,
                        runtime: prepared.armDef.runtime,
                        codebase: ep.codebase,
                        round: ep.round,
                        wall_time_s: completed.wall_time_s ?? null,
                        extraction_status: completed.extraction_status,
                        scorer_score: completed.scorer_score ?? null,
                        mutation_guard_status: completed.mutation_guard_status,
                        harness_valid: true,
                        episode_dir: prepared.episodeDir,
                        skipped: true,
                    };
                }
            }
            // 락 획득 순서(전 episode 단일 total order → 데드락 없음):
            //   1) serena codebase 락 (serena만)
            //   2) backend 세마포어 (serena=3 / codegraph=4)
            //   3) 메모리 가드 대기 (serena만; 무거운 serena 신규 실행 전)
            //   4) 전역 세마포어 (마지막에 획득)
            // 전역을 마지막에 잡아야: 대기 중 episode가 희소한 전역 슬롯을 점유하지 않는다.
            // release는 역순(전역 → backend → codebase). **멱등**(scorer-out-of-lock에서 한 번,
            //   finally에서 한 번 호출돼도 한 번만 실제 release).
            let acquiredCodebase = false;
            let acquiredBackend = false;
            let acquiredGlobal = false;
            let released = false;
            function releaseSlots() {
                if (released) return;
                released = true;
                if (acquiredGlobal) globalSem.release();
                if (acquiredBackend && backendSem) backendSem.release();
                if (acquiredCodebase && codebaseLock) codebaseLock.release();
            }

            if (codebaseLock) {
                await codebaseLock.acquire();
                acquiredCodebase = true;
                if (batchAbortController.signal.aborted) { releaseSlots(); return abortedResult(ep, armDef, "[runner:batch_aborted] before backend slot"); }
            }
            if (backendSem) {
                await backendSem.acquire();
                acquiredBackend = true;
                if (batchAbortController.signal.aborted) { releaseSlots(); return abortedResult(ep, armDef, "[runner:batch_aborted] before global slot"); }
            }

            // 메모리 가드: serena(무거운) episode 신규 실행 전 메모리 확인(전역 슬롯 잡기 전).
            let memoryGuard = null;
            if (isSerena && config.memoryGuardEnabled !== false) {
                memoryGuard = await waitForMemoryBeforeHeavyEpisode();
                if (batchAbortController.signal.aborted) { releaseSlots(); return abortedResult(ep, armDef, "[runner:batch_aborted] during memory guard"); }
                if (memoryGuard.waited) {
                    console.log(`[memory-guard:resume] ${episodeId} waited_ms=${memoryGuard.waitedMs}`);
                }
            }

            await globalSem.acquire();
            acquiredGlobal = true;
            try { throwIfBatchAborted(); } catch (error) { releaseSlots(); return abortedResult(ep, armDef, error); }

            // co-tenancy: 실행 중으로 등록(스냅샷에 잡히도록).
            activeBackends.set(episodeId, backend);

            let result;
            try {
                result = await runImpl({ ...ep, sequence_ordinal: ep.sequence_ordinal ?? sequenceOrdinal + 1 }, config, { releaseSlots, coTenancySnapshot, prepared, abortSignal: batchAbortController.signal });
            } catch (err) {
                console.error(`[episode:error] ${ep.arm_id}/${ep.codebase}/round-${ep.round}: ${err}`);
                result = {
                    arm_id: ep.arm_id,
                    runtime: armDef?.runtime ?? "unknown",
                    codebase: ep.codebase,
                    round: ep.round,
                    wall_time_s: null,
                    extraction_status: "runner_error",
                    scorer_score: null,
                    mutation_guard_status: "unknown",
                    harness_valid: false,
                    episode_dir: null,
                    skipped: false,
                    error: String(err),
                };
            } finally {
                activeBackends.delete(episodeId);
                // 멱등 release: runImpl이 scorer 전에 이미 release했으면 no-op.
                releaseSlots();
            }
            if (result?.error) batchAbortController.abort(new Error(result.error));
            return result;
        };
    });

    // 모든 episode를 병렬로 시작 (세마포어가 동시성 제한)
    const settled = await Promise.allSettled(tasks.map((t) => t()));
    const results = settled.map((entry, index) => entry.status === "fulfilled" ? entry.value : abortedResult(episodes[index], armConfigArms.find((arm) => arm.arm_id === (episodes[index].arm_id ?? episodes[index].arm)), entry.reason));
    return results;
}

// ============================================================
// main
// ============================================================

async function main() {
    const args = parseArgs(process.argv.slice(2));

    // 필수 인자 체크
    for (const k of ["arm-config", "manifest", "scorer", "schema-dir", "out-root"]) {
        if (!args[k]) {
            console.error(`[runner] missing required arg --${k}`);
            console.error(
                "usage: runner.mjs --arm-config <path> --manifest <path> --scorer <path> --schema-dir <dir> --out-root <dir> --codemap-bin <absolute-path> [--workspace-root <dir>] [--episodes <json>] [--timeout-s 1800] [--judge-model opus] [--skip-scorer] [--force] (--force refuses an already-successful identity; use a fresh --out-root/run id)",
            );
            process.exit(2);
        }
    }

    const workspaceRoot = path.resolve(args["workspace-root"] || DEFAULT_WORKSPACE_ROOT);
    requireDirectory("workspace root", workspaceRoot);
    const armConfig = readJson(args["arm-config"]);
    const manifest = resolveManifestPlaceholders(readJson(args["manifest"]), workspaceRoot);
    const scorerPath = path.resolve(args["scorer"]);
    const schemaDir = path.resolve(args["schema-dir"]);
    const outRoot = path.resolve(args["out-root"]);
    const timeoutMs = parseInt(args["timeout-s"] || "1800", 10) * 1000;
    const judgeModel = args["judge-model"] || "opus";
    const printCmd = Boolean(args["print-cmd"]);
    const force = Boolean(args["force"]); // incomplete episode만 재시도; 성공 identity는 fresh out-root가 필요
    const allowRetry = Boolean(args["allow-retry"]); // default false; run06 never enables this
    const skipScorer = Boolean(args["skip-scorer"]);
    mkdirSync(outRoot, { recursive: true });

    // episodes 파싱
    let episodes = [];
    if (args["episodes"]) {
        let episodesText = args["episodes"];
        if (existsSync(episodesText)) {
            episodesText = readFileSync(episodesText, "utf8");
        }
        episodes = JSON.parse(episodesText);
    } else {
        console.error("[runner] --episodes 인자 없음. proof-slice 실행 시 --episodes 필수.");
        process.exit(2);
    }

    if (!Array.isArray(episodes) || episodes.length === 0) {
        console.error("[runner] episodes가 비어있음");
        process.exit(2);
    }
    const episodeKeys = new Set();
    for (const episode of episodes) {
        const round = canonicalRound(episode.round);
        episode.round = round;
        const key = `${episode.arm_id ?? episode.arm}__${episode.codebase}__round-${round}`;
        if (episodeKeys.has(key)) throw new Error(`[runner] duplicate episode input rejected: ${key}`);
        episodeKeys.add(key);
    }

    const selectedArms = episodes.map((episode) => armConfig.arms.find((arm) => arm.arm_id === (episode.arm_id ?? episode.arm)));
    if (selectedArms.some((arm) => !arm)) throw new Error("[runner] episode references an unknown arm");
    const needsCodemap = selectedArms.some((arm) => arm.backend === "codemap");
    if (needsCodemap && !args["codemap-bin"]) throw new Error("[runner] --codemap-bin is required when an episode uses the codemap backend");
    const codemapBin = needsCodemap ? path.resolve(args["codemap-bin"]) : null;
    const codemapBinary = needsCodemap ? codemapBinaryIdentity(codemapBin) : null;
    assertLedgerIdentitySchema(outRoot);

    const config = {
        armConfig,
        manifest,
        scorerPath,
        schemaDir,
        outRoot,
        timeoutMs,
        judgeModel,
        printCmd,
        force,
        allowRetry,
        skipScorer,
        codemapBin,
        codemapBinary,
        workspaceRoot,
        runnerStartedAt: new Date().toISOString(),
        preflightCache: new Map(),
        targetIdentityCache: new Map(),
        runtimeIdentityCache: new Map(),
        preflightSemaphore: new Semaphore(4),
        preflightConcurrencyCap: 4,
        paths: {
            runnerPath: path.resolve(process.argv[1]),
            manifestPath: path.resolve(args["manifest"]),
            armConfigPath: path.resolve(args["arm-config"]),
            outRoot,
            workspaceRoot,
        },
    };

    console.log(
        `[runner:start] episodes=${episodes.length} timeout_s=${timeoutMs / 1000} judge_model=${judgeModel} skip_scorer=${skipScorer} codemap_bin=${codemapBin ?? "not-required"} global_cap=${CONCURRENCY.GLOBAL_CAP} preflight_cap=${config.preflightConcurrencyCap} serena=${CONCURRENCY.SERENA_GLOBAL}+codebase${CONCURRENCY.SERENA_PER_CODEBASE} codegraph=${CONCURRENCY.CODEGRAPH_GLOBAL} force=${force}`,
    );

    const results = await runBatch(episodes, config);

    // 배치 결과 요약
    const summaryPath = path.join(outRoot, "batch_summary.json");
    const skippedCount = results.filter((r) => r && r.skipped === true).length;
    writeJson(summaryPath, {
        run_at: new Date().toISOString(),
        episode_count: episodes.length,
        executed_count: episodes.length - skippedCount,
        skipped_count: skippedCount,
        force,
        skip_scorer: skipScorer,
        results,
        concurrency_enforced: {
            global_cap: CONCURRENCY.GLOBAL_CAP,
            serena: { global: CONCURRENCY.SERENA_GLOBAL, per_codebase: CONCURRENCY.SERENA_PER_CODEBASE },
            codegraph: { global: CONCURRENCY.CODEGRAPH_GLOBAL },
            light: "global_only", // codemap, no-mcp
            claude: "parallel", // claude_serial 제거
            scorer_out_of_lock: true,
            per_episode_backend_cleanup: true,
            preflight: { cap: config.preflightConcurrencyCap, cache: "canonical root/runtime/binary identity + deterministic fixture contract hash" },
            memory_guard: "serena: free+inactive<8GB or pressure>=warn → wait",
        },
    });

    const failedResults = results.filter((result) => result?.error);
    if (failedResults.length > 0) {
        throw new Error(`[runner:batch] ${failedResults.length} episode(s) failed; see ${summaryPath}`);
    }
    console.log(`[runner:done] results=${results.length} summary=${summaryPath}`);
    return results;
}

// ============================================================
// 모듈로도, 직접 실행으로도 사용 가능
// ============================================================

export {
    backendOf,
    canonicalJson,
    CONCURRENCY,
    calcBackendExercised,
    diffSnapshots,
    evaluationContractSha256,
    extractOpencodeJsonOutput,
    isBackendArtifactPath,
    isBackendToolName,
    loadCompletedEpisode,
    makeSerenaCodebaseLockFactory,
    readMemoryStatus,
    runBatch,
    runEpisode,
    Semaphore,
    waitForMemoryBeforeHeavyEpisode,
};

// 직접 실행
if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(import.meta.url.replace("file://", ""))) {
    main().catch((err) => {
        console.error(`[runner:fatal] ${err.stack || err}`);
        process.exit(1);
    });
}
