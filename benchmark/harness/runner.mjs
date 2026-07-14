#!/usr/bin/env node

/**
 * CMS Official Benchmark 20260619-02 — durable run-local runner
 *
 * 설계 원칙:
 *   - episode-list-driven: 슬라이스(3 episode)와 full run(180 episode) 모두 동일 진입점 사용
 *   - 동시성 제어: claude_serial = semaphore(1), non_claude_capped = semaphore(3),
 *       per-codebase 상호배제 = codebase당 in-flight ≤ 1 (글로벌, claude 포함)
 *   - per-episode 1800s wall-time timeout (인자 --timeout-s로 조정; 측정 max 896.9s 근거)
 *   - OpenCode 무출력 60s timeout (인자 --opencode-no-output-timeout-s로 조정)
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
 *                   [--opencode-no-output-timeout-s 60]
 *                   [--concurrency 1]
 *                   --codemap-bin <absolute-path>
 *                   [--workspace-root <dir>]
 *                   [--judge-model opus] [--skip-scorer]
 *                   [--print-cmd]
 *
 * episodes 형식: [{arm_id, codebase, round}]
 * 복구 전용: node runner.mjs --recover-aggregate <existing-out-root> --episodes <json-file-or-inline-json>
 * 세션 복구: node runner.mjs --recover-opencode-sessions <existing-out-root> --session-recovery-allowlist <json-file-or-inline-json> --episodes <json-file-or-inline-json>
 */

import { spawn, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { appendFileSync, chmodSync, closeSync, copyFileSync, existsSync, fsyncSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

// ============================================================
// 상수 / 경로
// ============================================================

const REPO_ROOT_PLACEHOLDER = "<REPO_ROOT>";
const DEFAULT_WORKSPACE_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const CLAUDE_BIN = "claude";
const SOLVER_CONTRACT_SCHEMA_VERSION = 1;
const ALLOCATION_LEDGER_SCHEMA_VERSION = 1;
const OPENCODE_PROTOCOL_VERSION = "opencode-1.17.18-jsonl-v2";
const OPENCODE_FRAMING_VERSION = "ndjson-complete-line-v1";
const OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE = "OPENCODE_EXPERIMENTAL_DISABLE_FILEWATCHER";
const OPENCODE_FILE_WATCHER_DISABLE_VALUE = "true";
const DEFAULT_SOLVER_TIMEOUT_SECONDS = 1800;
const DEFAULT_OPENCODE_NO_OUTPUT_TIMEOUT_SECONDS = 60;
const PROCESS_TERMINATION_GRACE_MS = 2_000;
const PROCESS_FORCE_SETTLE_GRACE_MS = 1_000;
const OPENCODE_EXPORT_TIMEOUT_MS = 30_000;
const OPENCODE_EXPORT_MAX_BYTES = 64 * 1024 * 1024;

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

function millisecondsFromPositiveSeconds(value, optionName, defaultSeconds) {
    const seconds = Number(value ?? defaultSeconds);
    const milliseconds = seconds * 1000;
    if (!Number.isFinite(seconds) || seconds <= 0 || !Number.isSafeInteger(milliseconds)) {
        throw new Error(`[runner:timeout] --${optionName} must be a positive number of seconds`);
    }
    return milliseconds;
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

function writeJsonAtomically(filePath, value) {
    mkdirSync(path.dirname(filePath), { recursive: true });
    const temporaryPath = `${filePath}.${process.pid}.${randomUUID()}.tmp`;
    const fd = openSync(temporaryPath, "wx");
    try {
        writeFileSync(fd, JSON.stringify(value, null, 2) + "\n");
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
    renameSync(temporaryPath, filePath);
    const directoryFd = openSync(path.dirname(filePath), "r");
    try { fsyncSync(directoryFd); } finally { closeSync(directoryFd); }
}

function writeTextAtomically(filePath, value, mode = 0o600) {
    mkdirSync(path.dirname(filePath), { recursive: true });
    const temporaryPath = `${filePath}.${process.pid}.${randomUUID()}.tmp`;
    const fd = openSync(temporaryPath, "wx", mode);
    try {
        writeFileSync(fd, String(value ?? ""));
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
    renameSync(temporaryPath, filePath);
    const directoryFd = openSync(path.dirname(filePath), "r");
    try { fsyncSync(directoryFd); } finally { closeSync(directoryFd); }
}

function promoteNewFileAtomically(temporaryPath, finalPath) {
    if (existsSync(finalPath)) throw new Error(`[runner:atomic] refusing to replace existing file: ${finalPath}`);
    const fd = openSync(temporaryPath, "r");
    try { fsyncSync(fd); } finally { closeSync(fd); }
    chmodSync(temporaryPath, 0o600);
    renameSync(temporaryPath, finalPath);
    const directoryFd = openSync(path.dirname(finalPath), "r");
    try { fsyncSync(directoryFd); } finally { closeSync(directoryFd); }
}

function globalSlotRecordPath(outRoot, allocationId = null) {
    return allocationId
        ? path.join(outRoot, "global-slots", `${allocationId}.json`)
        : path.join(outRoot, "global-slot-owner.json");
}

function assertNoUnfinalizedGlobalSlotOwner(outRoot) {
    const recordPath = globalSlotRecordPath(outRoot);
    if (!existsSync(recordPath)) return;
    const record = readJson(recordPath);
    if (record?.phase !== "released") {
        throw new Error(`[runner:slot] prior global-slot owner is not finalized: allocation=${record?.allocation_id ?? "unknown"} phase=${record?.phase ?? "unknown"}`);
    }
}

function writeGlobalSlotRecord(outRoot, value) {
    const record = { version: 1, runner_pid: process.pid, recorded_at: new Date().toISOString(), ...value };
    writeJsonAtomically(globalSlotRecordPath(outRoot, record.allocation_id ?? null), record);
    const lifecyclePath = path.join(outRoot, "global-slot-lifecycle.jsonl");
    const fd = openSync(lifecyclePath, "a");
    try {
        appendFileSync(fd, JSON.stringify(record) + "\n", { encoding: "utf8" });
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
    return record;
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

function allocationLedgerPath(outRoot) {
    return path.join(outRoot, "allocation-episode-ledger.jsonl");
}

function assertAllocationEpisodeId(value, fieldName = "allocation_id") {
    if (typeof value !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(value)) {
        throw new Error(`[runner:allocation] ${fieldName} must be a stable, path-safe identifier`);
    }
    return value;
}

function allocationEpisodeFromInput(episode) {
    const allocationId = assertAllocationEpisodeId(episode.allocation_id, "allocation_id");
    const replacement = episode.replacement ?? null;
    if (replacement != null && (typeof replacement !== "object" || Array.isArray(replacement))) {
        throw new Error(`[runner:allocation] replacement must be an object when provided for ${allocationId}`);
    }
    if (replacement?.replaces_allocation_id != null) {
        assertAllocationEpisodeId(replacement.replaces_allocation_id, "replacement.replaces_allocation_id");
        if (typeof replacement.replaces_logical_episode_id !== "string" || replacement.replaces_logical_episode_id.length === 0) {
            throw new Error(`[runner:allocation] replacement.replaces_logical_episode_id is required for ${allocationId}`);
        }
    }
    return { allocation_id: allocationId, replacement };
}

function allocationLedgerEvents(outRoot, allocationId) {
    const ledgerPath = allocationLedgerPath(outRoot);
    if (!existsSync(ledgerPath)) return [];
    return readFileSync(ledgerPath, "utf8").split("\n").flatMap((line, index) => {
        if (!line.trim()) return [];
        let event;
        try { event = JSON.parse(line); } catch { ledgerCorruption(`invalid JSONL at ${ledgerPath}:${index + 1}`); }
        if (!event || event.ledger_schema_version !== ALLOCATION_LEDGER_SCHEMA_VERSION || typeof event.allocation_id !== "string") {
            ledgerCorruption(`invalid allocation ledger event at ${ledgerPath}:${index + 1}`);
        }
        return event.allocation_id === allocationId ? [event] : [];
    });
}

function assertAllocationEpisodeUnused(outRoot, allocationId) {
    const prior = allocationLedgerEvents(outRoot, allocationId);
    if (prior.length > 0) {
        throw new Error(`[runner:allocation] allocation episode already recorded and is immutable: ${allocationId}`);
    }
}

function appendAllocationLedger(outRoot, event) {
    const ledgerPath = allocationLedgerPath(outRoot);
    mkdirSync(path.dirname(ledgerPath), { recursive: true });
    const fd = openSync(ledgerPath, "a");
    try {
        appendFileSync(fd, JSON.stringify({ ledger_schema_version: ALLOCATION_LEDGER_SCHEMA_VERSION, ...event }) + "\n", { encoding: "utf8" });
        fsyncSync(fd);
    } finally {
        closeSync(fd);
    }
    return ledgerPath;
}

function tokenBreakdown(tokens) {
    if (!tokens || typeof tokens !== "object") return null;
    return {
        input_tokens: tokens.input_tokens ?? null,
        output_tokens: tokens.output_tokens ?? null,
        reasoning_tokens: tokens.reasoning_tokens ?? null,
        cache_read_input_tokens: tokens.cache_read_input_tokens ?? tokens.cached_input_tokens ?? null,
        cache_creation_input_tokens: tokens.cache_creation_input_tokens ?? null,
        total_tokens: tokens.total_tokens ?? null,
        total_tokens_source: tokens.total_tokens_source ?? null,
        accounting_status: tokens.accounting_status ?? null,
        incomplete_fields: tokens.incomplete_fields ?? [],
        field_availability: tokens.field_availability ?? null,
        available_fields: tokens,
    };
}

function toolCallSummary(toolEvents) {
    const callsByTool = {};
    for (const event of toolEvents ?? []) {
        if (event?.phase !== "call") continue;
        const name = event.tool_name ?? "unknown";
        callsByTool[name] = (callsByTool[name] ?? 0) + 1;
    }
    return {
        total: Object.values(callsByTool).reduce((total, count) => total + count, 0),
        by_tool: callsByTool,
    };
}

function writeDurableSessionSidecar({ outRoot, attemptDir, ledgerBase, ep, armDef, prepared, processResult, solveStartedAt, solveFinishedAt, tokens, toolEvents, parserStatus = null, score = null, resultPaths = {}, error = null, missingEvidenceReasons = {} }) {
    const sidecarPath = path.join(attemptDir, "durable_session_sidecar.json");
    const sessionId = parserStatus?.session_id ?? null;
    const sidecar = {
        schema_version: 1,
        recorded_at: new Date().toISOString(),
        condition: {
            allocation_id: ledgerBase.allocation_id,
            pair_id: ep.pair_id ?? null,
            stratum: ep.stratum ?? null,
            arm_id: ledgerBase.arm_id,
        },
        task: { codebase: ledgerBase.codebase, round: ledgerBase.round, episode_id: ledgerBase.episode_id },
        order: {
            order: ep.order ?? null,
            period: ep.period ?? null,
            wave_id: ep.wave_id ?? null,
            sequence_ordinal: ledgerBase.sequence_ordinal,
        },
        model: { model: armDef.model, runtime: ledgerBase.runtime, backend: armDef.backend },
        binary_seal: prepared?.codemapIdentity ?? null,
        session_id: sessionId,
        process: {
            exit_code: processResult?.exitCode ?? null,
            signal: processResult?.signal ?? null,
            timed_out: processResult?.timedOut ?? null,
            spawn_error: processResult?.spawnError ?? null,
            status: parserStatus?.status ?? (error ? "runner_error" : "process_terminal"),
            started_at: solveStartedAt?.toISOString?.() ?? null,
            ended_at: solveFinishedAt?.toISOString?.() ?? null,
            wall_ms: processResult?.elapsedMs ?? null,
            raw_stream_lifecycle: processResult?.raw_stream_lifecycle ?? null,
            timeout_kind: processResult?.timeoutKind ?? null,
            termination: processResult?.termination ?? null,
        },
        tokens: tokenBreakdown(tokens),
        missing_evidence: {
            session_id: sessionId ? null : missingEvidenceReasons.session_id ?? "runtime_did_not_report_session_id",
            tokens: tokens ? null : missingEvidenceReasons.tokens ?? "runtime_did_not_report_tokens",
        },
        tool_calls: toolCallSummary(toolEvents),
        score,
        result_paths: resultPaths,
        error: error ? String(error).slice(0, 400) : null,
    };
    writeJsonAtomically(sidecarPath, sidecar);
    appendAttemptLedger(outRoot, {
        ...ledgerBase,
        event: "session_evidence",
        timestamp: sidecar.recorded_at,
        session_evidence_path: sidecarPath,
        condition: sidecar.condition,
        task: sidecar.task,
        order: sidecar.order,
        model_evidence: sidecar.model,
        binary_seal: sidecar.binary_seal,
        session_id: sidecar.session_id,
        process: sidecar.process,
        tokens: sidecar.tokens,
        missing_evidence: sidecar.missing_evidence,
        tool_calls: sidecar.tool_calls,
        score: sidecar.score,
        result_paths: sidecar.result_paths,
        error: sidecar.error,
    });
    return sidecar;
}

function allocationClaimPath(outRoot, allocationId) {
    return path.join(outRoot, "claims", `allocation-${allocationId}.claim`);
}

function acquireAllocationClaim(outRoot, allocationId) {
    const claimPath = allocationClaimPath(outRoot, allocationId);
    mkdirSync(path.dirname(claimPath), { recursive: true });
    let fd;
    try {
        fd = openSync(claimPath, "wx");
        writeFileSync(fd, JSON.stringify({ pid: process.pid, allocation_id: allocationId, claimed_at: new Date().toISOString() }) + "\n");
        fsyncSync(fd);
    } catch (error) {
        if (error?.code === "EEXIST") throw new Error(`[runner:allocation] allocation episode is currently claimed: ${allocationId}`);
        throw error;
    } finally {
        if (fd != null) closeSync(fd);
    }
    return claimPath;
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

function assertSealedCodemapBinary(expected) {
    if (!expected?.path || !expected?.sha256 || !Number.isSafeInteger(expected.bytes)) {
        throw new Error("[runner:binary] missing sealed arm binary identity");
    }
    const canonicalPath = realpathSync(expected.path);
    const observed = fileDigest(canonicalPath);
    if (canonicalPath !== expected.path || observed.sha256 !== expected.sha256 || observed.bytes !== expected.bytes) {
        throw new Error(`[runner:binary] sealed arm binary mismatch: expected=${expected.path}/${expected.sha256}/${expected.bytes} observed=${canonicalPath}/${observed.sha256}/${observed.bytes}`);
    }
    return { path: canonicalPath, ...observed };
}

function writeCodemapExecWrapper({ episodeDir, allocationId, armId, binary }) {
    const expected = assertSealedCodemapBinary(binary);
    const runtimeDir = path.join(episodeDir, "runtime-attestation");
    mkdirSync(runtimeDir, { recursive: true });
    const wrapperPath = path.join(runtimeDir, "codemap-exec-wrapper.mjs");
    const attestationPath = path.join(runtimeDir, "mcp-startup-attestation.json");
    const payload = { allocation_id: allocationId, arm_id: armId, expected_binary: expected, attestation_path: attestationPath };
    const source = `#!/usr/bin/env node\nimport { createHash } from "node:crypto";\nimport { closeSync, fsyncSync, openSync, readFileSync, realpathSync, renameSync, statSync, writeFileSync } from "node:fs";\nconst payload = ${JSON.stringify(payload)};\nconst actualPath = realpathSync(payload.expected_binary.path);\nconst content = readFileSync(actualPath);\nconst observed = { path: actualPath, sha256: createHash("sha256").update(content).digest("hex"), bytes: statSync(actualPath).size };\nconst matched = actualPath === payload.expected_binary.path && observed.sha256 === payload.expected_binary.sha256 && observed.bytes === payload.expected_binary.bytes;\nconst record = { version: 1, allocation_id: payload.allocation_id, arm_id: payload.arm_id, expected_binary: payload.expected_binary, observed_binary: observed, matched, recorded_at: new Date().toISOString() };\nconst temporaryPath = payload.attestation_path + "." + process.pid + ".tmp";\nconst fd = openSync(temporaryPath, "wx");\ntry { writeFileSync(fd, JSON.stringify(record) + "\\n"); fsyncSync(fd); } finally { closeSync(fd); }\nrenameSync(temporaryPath, payload.attestation_path);\nif (!matched) process.exit(96);\nprocess.execve(actualPath, [actualPath, ...process.argv.slice(2)], process.env);\n`;
    writeFileSync(wrapperPath, source, { mode: 0o700 });
    return { wrapperPath, attestationPath, command: [process.execPath, wrapperPath], expected, allocation_id: allocationId, arm_id: armId };
}

function assertMcpStartupAttestation(wrapper) {
    const record = readJson(wrapper.attestationPath);
    const observed = record?.observed_binary;
    if (record?.matched !== true || record?.allocation_id !== wrapper.allocation_id || record?.arm_id !== wrapper.arm_id || observed?.path !== wrapper.expected.path || observed?.sha256 !== wrapper.expected.sha256 || observed?.bytes !== wrapper.expected.bytes) {
        throw new Error(`[runner:binary] MCP startup attestation mismatch: ${wrapper.attestationPath}`);
    }
    return record;
}

// Portable bundle identity: legacy seals hashed absolute `shasum` records, so a byte-identical
// copy under another root changed identity. v2 sorts raw relative POSIX paths, frames each path
// with NUL plus its content hash, and excludes the same lock/temp basenames as the legacy walk.
// Symlinks are excluded without following them, matching `find -type f` used by the legacy seal.
function portableBundleDigestV2(bundleRoot, excludedRelativePaths = new Set()) {
    const root = realpathSync(bundleRoot);
    const files = [];
    const visit = (directory) => {
        for (const entry of readdirSync(directory, { withFileTypes: true })) {
            const entryPath = path.join(directory, entry.name);
            if (entry.isSymbolicLink()) continue;
            if (entry.isDirectory()) { visit(entryPath); continue; }
            if (!entry.isFile() || entry.name.endsWith(".lock") || entry.name.endsWith(".tmp")) continue;
            const relativePath = path.relative(root, entryPath).split(path.sep).join("/");
            if (!relativePath || relativePath.includes("\0")) throw new Error(`[bundle-digest] invalid relative path: ${entryPath}`);
            if (excludedRelativePaths.has(relativePath)) continue;
            const content = readFileSync(entryPath);
            files.push({ relative_path: relativePath, content_sha256: sha256(content), bytes: content.length });
        }
    };
    visit(root);
    files.sort((left, right) => Buffer.compare(Buffer.from(left.relative_path), Buffer.from(right.relative_path)));
    const payload = Buffer.concat(files.map((file) => Buffer.from(`${file.relative_path}\0${file.content_sha256}\0`, "utf8")));
    return {
        version: "portable-relative-content-v2",
        symlink_policy: "exclude_without_following_like_legacy_find_type_f",
        excluded_basenames: ["*.lock", "*.tmp"],
        excluded_relative_paths: [...excludedRelativePaths].sort(),
        file_count: files.length,
        bytes: files.reduce((sum, file) => sum + file.bytes, 0),
        sha256: sha256(payload),
    };
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

async function callCodemapMcp(command, targetRoot, request, environment = {}) {
    return await new Promise((resolve, reject) => {
        const [executable, ...baseArgs] = command;
        const child = spawn(executable, [...baseArgs, "mcp"], { cwd: targetRoot, env: { ...process.env, ...environment }, stdio: ["pipe", "pipe", "pipe"] });
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

async function preflightFixture({ taskDef, taskId, targetRoot, armDef, codemapCommand, wrapper, identity: suppliedIdentity = null }) {
    const fixture = taskDef.fixture;
    if (!fixture) throw new Error(`[fixture] missing fixture declaration for ${taskId}`);
    const clean = fixture.clean;
    if (!clean || typeof clean !== "object") throw new Error(`[fixture] missing immutable clean fixture contract for ${taskId}`);
    const identity = suppliedIdentity ?? targetIdentity(targetRoot);
    if (clean.source_root !== identity.realpath) throw new Error(`[fixture] clean source root mismatch for ${taskId}: manifest=${clean.source_root} observed=${identity.realpath}`);
    const codemapHome = realpathSync(clean.codemap_home);
    const externalIndexPath = realpathSync(clean.index_path);
    if (!lstatSync(externalIndexPath).isDirectory() || lstatSync(externalIndexPath).isSymbolicLink()) throw new Error(`[fixture] external index is not a regular directory: ${externalIndexPath}`);
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
        const indexPath = externalIndexPath;
        // A symlinked index can make an unrelated stale cache look valid. The root itself and
        // every visible index entry must resolve under the target worktree.
        if (!existsSync(indexPath) || !lstatSync(indexPath).isDirectory()) throw new Error(`[fixture] codemap index not ready: ${indexPath}`);
        if (lstatSync(indexPath).isSymbolicLink()) throw new Error(`[fixture] codemap index symlink is not allowed: ${indexPath}`);
        const indexEntries = readdirSync(indexPath);
        if (indexEntries.length === 0) throw new Error(`[fixture] codemap index not ready: ${indexPath}`);
        for (const entry of indexEntries) {
            const entryPath = path.join(indexPath, entry);
            if (lstatSync(entryPath).isSymbolicLink()) throw new Error(`[fixture] codemap index symlink is not allowed: ${entryPath}`);
            if (!realpathSync(entryPath).startsWith(externalIndexPath + path.sep) && realpathSync(entryPath) !== externalIndexPath) throw new Error(`[fixture] external index entry escapes configured index: ${entryPath}`);
        }
        const mcpEnvironment = { CODEMAP_HOME: codemapHome };
        const searchResult = await callCodemapMcp(codemapCommand, identity.realpath, { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "search", arguments: { query: fixture.codemap_query, limit: 10 } } }, mcpEnvironment);
        const startupAttestation = assertMcpStartupAttestation(wrapper);
        const searchText = mcpText(searchResult);
        const match = searchText.match(/^### File: (.+?) \(\d+ lines\)$/m);
        if (!match || /No indexed matches/i.test(searchText)) throw new Error(`[fixture] codemap MCP search produced no in-root match: query=${fixture.codemap_query}`);
        const matchPath = realpathInside(identity.realpath, path.join(identity.realpath, match[1]), "codemap MCP search match").candidateReal;
        const readResult = await callCodemapMcp(codemapCommand, identity.realpath, { jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "read", arguments: { file_path: readRelativePath, offset: 1, limit: 40 } } }, mcpEnvironment);
        const readText = mcpText(readResult);
        if (!readText.trim()) throw new Error(`[fixture] codemap MCP read returned empty content: ${readRelativePath}`);
        proof.index_path = indexPath;
        proof.search_query = fixture.codemap_query;
        proof.search_match_path = matchPath;
        proof.search_response_sha256 = sha256(searchText);
        proof.read_response_sha256 = sha256(readText);
        proof.mcp_read_path = readRealpath;
        proof.codemap_home = codemapHome;
        proof.external_index_path = externalIndexPath;
        proof.mcp_startup_attestation = startupAttestation;
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

function cleanArtifactBinary(taskDef, armId) {
    const artifact = taskDef.fixture?.clean?.artifacts?.[armId];
    if (!artifact?.path || !artifact?.sha256) throw new Error(`[fixture] missing clean artifact for arm ${armId}`);
    const observed = fileDigest(artifact.path);
    if (!observed.exists || observed.sha256 !== artifact.sha256) throw new Error(`[fixture] clean artifact identity mismatch for arm ${armId}: ${artifact.path}`);
    return artifact.path;
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

function solverInvocationContract(plan) {
    const behaviorEnvironmentKeys = ["CODEMAP_HOME"];
    if (plan.command === "opencode") behaviorEnvironmentKeys.push(OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE);
    const behaviorEnvironment = Object.fromEntries(
        behaviorEnvironmentKeys.flatMap((key) => plan.env?.[key] == null ? [] : [[key, plan.env[key]]]),
    );
    return {
        command: plan.command,
        argv: plan.args,
        cwd: plan.cwd,
        stdin_sha256: sha256(plan.stdin ?? ""),
        environment: behaviorEnvironment,
        opencode_config: plan.opencodeConfigPath ? fileDigest(plan.opencodeConfigPath) : null,
    };
}

function effectiveOpenCodeNoOutputTimeoutMs(runtime, config) {
    if (!runtime?.startsWith("opencode-")) return null;
    const configuredTimeoutMs = config.opencodeNoOutputTimeoutMs ?? DEFAULT_OPENCODE_NO_OUTPUT_TIMEOUT_SECONDS * 1000;
    return Math.min(config.timeoutMs, configuredTimeoutMs);
}

function buildResumeIdentity({ armDef, promptPath, prompt, config, contract, fixture, evaluationContract, runtime, episode, plan, codemapIdentity }) {
    const solverValue = {
        solver_contract_schema_version: SOLVER_CONTRACT_SCHEMA_VERSION,
        logical_episode: episode,
        task: {
            manifest: fileDigest(config.paths.manifestPath),
            code_root: fixture.identity,
            clean_fixture: fixture.proof,
            prompt: { path: promptPath, ...fileDigest(promptPath), sha256: sha256(prompt) },
            scoring_schema: fileDigest(contract.schema.path),
            evaluation_contract_sha256: evaluationContractSha256(evaluationContract),
        },
        solver: {
            runtime,
            endpoint_model: armDef.model,
            backend: armDef.backend,
            permissions: { shell_policy: armDef.shell_policy, builtin_read_policy: armDef.builtin_read_policy, mcp_config_policy: armDef.mcp_config_policy },
            codemap_binary: armDef.backend === "codemap" ? codemapIdentity : null,
            timeout_ms: config.timeoutMs,
            no_output_timeout_ms: effectiveOpenCodeNoOutputTimeoutMs(armDef.runtime, config),
            termination_grace_ms: PROCESS_TERMINATION_GRACE_MS,
            invocation: solverInvocationContract(plan),
        },
    };
    const finalValue = {
        identity_schema_version: IDENTITY_SCHEMA_VERSION,
        solver_contract_hash: sha256(canonicalJson(solverValue)),
        skip_scorer: config.skipScorer,
        scoring_contract: contract,
    };
    const runnerOperationalValue = {
        runner: fileDigest(config.paths.runnerPath),
        scheduler: { requested_concurrency: config.requestedConcurrency, preflight_cap: config.preflightConcurrencyCap },
        durable_ledger_schema_version: ALLOCATION_LEDGER_SCHEMA_VERSION,
    };
    return {
        sha256: sha256(canonicalJson(finalValue)),
        value: finalValue,
        solver_identity: { sha256: finalValue.solver_contract_hash, value: solverValue },
        solver_contract_hash: finalValue.solver_contract_hash,
        runner_operational_hash: sha256(canonicalJson(runnerOperationalValue)),
        runner_operational: runnerOperationalValue,
    };
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
function opencodeSessionIdFromEvent(event) {
    const candidates = [
        event?.sessionID,
        event?.sessionId,
        event?.session_id,
        event?.session?.id,
        event?.part?.sessionID,
        event?.part?.sessionId,
        event?.part?.session_id,
    ].filter((value) => typeof value === "string" && value.length > 0);
    return candidates.length === 0 ? null : candidates[0];
}

function parseOpencodeProtocolV2(rawBytes) {
    const bytes = Buffer.isBuffer(rawBytes) ? rawBytes : Buffer.from(rawBytes ?? "");
    const rawText = bytes.toString("utf8");
    const lines = rawText.split(/\n/);
    const hasTrailingNewline = bytes.length === 0 || rawText.endsWith("\n");
    const events = [];
    const protocolErrors = [];
    const eventTypeHistogram = {};
    const sessionIds = new Set();

    for (let index = 0; index < lines.length; index++) {
        const rawLine = lines[index].replace(/\r$/, "");
        if (index === lines.length - 1 && rawLine === "" && hasTrailingNewline) continue;
        if (rawLine.trim() === "") continue;
        let event;
        try {
            event = JSON.parse(rawLine);
        } catch {
            protocolErrors.push({ kind: "invalid_ndjson_record", line_number: index + 1, sha256: sha256(rawLine) });
            continue;
        }
        if (!event || typeof event !== "object" || Array.isArray(event) || typeof event.type !== "string") {
            protocolErrors.push({ kind: "invalid_event_shape", line_number: index + 1, sha256: sha256(rawLine) });
            continue;
        }
        events.push({ event, raw_line_number: index + 1 });
        eventTypeHistogram[event.type] = (eventTypeHistogram[event.type] ?? 0) + 1;
        const sessionId = opencodeSessionIdFromEvent(event);
        if (sessionId) sessionIds.add(sessionId);
    }
    if (!hasTrailingNewline && bytes.length > 0) {
        protocolErrors.push({ kind: "partial_final_record", line_number: lines.length, sha256: sha256(lines.at(-1) ?? "") });
    }
    if (sessionIds.size > 1) protocolErrors.push({ kind: "multiple_session_ids", session_ids: [...sessionIds] });

    const textParts = [];
    const toolEvents = [];
    const stepTokens = [];
    for (const [parsedEventIndex, { event, raw_line_number: rawLineNumber }] of events.entries()) {
        const eventPosition = { raw_line_number: rawLineNumber, parsed_event_index: parsedEventIndex };
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
                reasoning_tokens: t.reasoning ?? null,
                cache_read_input_tokens: (t.cache && t.cache.read) ?? null,
                cache_creation_input_tokens: (t.cache && t.cache.write) ?? null,
                total_tokens: t.total ?? null,
                available_fields: t,
            });
        }
    }
    const aggregateField = (field, { required = false } = {}) => {
        const values = stepTokens.map((tokens) => tokens[field]);
        const numericValues = values.filter((value) => typeof value === "number");
        const missing = values.length - numericValues.length;
        const unavailable = numericValues.length === 0;
        return {
            value: missing === 0 && numericValues.length > 0 ? numericValues.reduce((total, value) => total + value, 0) : null,
            complete: missing === 0 && numericValues.length > 0,
            incomplete: numericValues.length > 0 && missing > 0,
            unavailable,
            required,
        };
    };
    const last = events.at(-1);
    const tokenFields = {
        input_tokens: aggregateField("input_tokens", { required: true }),
        output_tokens: aggregateField("output_tokens", { required: true }),
        reasoning_tokens: aggregateField("reasoning_tokens", { required: true }),
        cache_read_input_tokens: aggregateField("cache_read_input_tokens"),
        cache_creation_input_tokens: aggregateField("cache_creation_input_tokens"),
        total_tokens: aggregateField("total_tokens"),
    };
    const incompleteFields = Object.entries(tokenFields)
        .filter(([, field]) => field.incomplete || (field.required && !field.complete))
        .map(([field]) => field);
    const fallbackTotalTokens = tokenFields.input_tokens.complete && tokenFields.output_tokens.complete && tokenFields.reasoning_tokens.complete
        ? tokenFields.input_tokens.value + tokenFields.output_tokens.value + tokenFields.reasoning_tokens.value
        : null;
    const totalTokens = tokenFields.total_tokens.complete ? tokenFields.total_tokens.value : fallbackTotalTokens;
    const totalTokensSource = tokenFields.total_tokens.complete ? "provider_step_total" : fallbackTotalTokens != null ? "input_output_reasoning_fallback" : null;
    const accountingStatus = stepTokens.length > 0 && incompleteFields.length === 0 && totalTokens != null ? "complete" : "incomplete";
    return {
        rawAnswer: textParts.join("\n").trim(),
        tokens: stepTokens.length === 0 ? null : {
            input_tokens: tokenFields.input_tokens.value,
            output_tokens: tokenFields.output_tokens.value,
            reasoning_tokens: tokenFields.reasoning_tokens.value,
            cache_read_input_tokens: tokenFields.cache_read_input_tokens.value,
            cache_creation_input_tokens: tokenFields.cache_creation_input_tokens.value,
            total_tokens: totalTokens,
            total_tokens_source: totalTokensSource,
            accounting_status: accountingStatus,
            incomplete_fields: incompleteFields,
            field_availability: Object.fromEntries(Object.entries(tokenFields).map(([field, value]) => [field, { complete: value.complete, unavailable: value.unavailable }])),
        },
        stepTokens,
        toolEvents,
        protocol: {
            protocol_version: OPENCODE_PROTOCOL_VERSION,
            framing_version: OPENCODE_FRAMING_VERSION,
            raw_bytes: bytes.length,
            raw_sha256: sha256(bytes),
            complete_line_framing: hasTrailingNewline,
            event_count: events.length,
            event_type_histogram: eventTypeHistogram,
            last_event: last ? {
                raw_line_number: last.raw_line_number,
                type: last.event.type,
                session_id: opencodeSessionIdFromEvent(last.event),
                sha256: sha256(JSON.stringify(last.event)),
            } : null,
            session_id: sessionIds.size === 1 ? [...sessionIds][0] : null,
            errors: protocolErrors,
            status: protocolErrors.length === 0 ? "valid" : "protocol_invalid",
        },
    };
}

function assistantTextFromSessionExport(value) {
    const messages = [];
    const visit = (node) => {
        if (!node || typeof node !== "object") return;
        if (Array.isArray(node)) {
            for (const item of node) visit(item);
            return;
        }
        const role = node.role ?? node.author?.role ?? node.message?.role;
        if (role === "assistant") {
            const candidates = [
                node.text,
                node.content,
                node.message?.text,
                node.message?.content,
                ...(Array.isArray(node.parts) ? node.parts.map((part) => part?.text ?? part?.content) : []),
                ...(Array.isArray(node.message?.parts) ? node.message.parts.map((part) => part?.text ?? part?.content) : []),
            ].filter((candidate) => typeof candidate === "string" && candidate.trim().length > 0);
            for (const candidate of candidates) messages.push(candidate.trim());
        }
        for (const child of Object.values(node)) visit(child);
    };
    visit(value);
    return messages.length === 0 ? null : messages.at(-1);
}

async function recoverOpencodeSessionFinal(plan, sessionId, attemptDir, { recovered = false } = {}) {
    if (!sessionId) return { status: "unavailable", session_id: null, export: null, final_answer: null, reason: "session_id_not_captured" };
    const suffix = recovered ? ".recovered" : "";
    const exportPath = path.join(attemptDir, `opencode_session_export${suffix}.json`);
    const failedExportPath = path.join(attemptDir, `opencode_session_export${suffix}.failed.raw`);
    const exportStderrPath = path.join(attemptDir, `opencode_session_export${suffix}.stderr.txt`);
    const temporaryExportPath = path.join(attemptDir, `.opencode_session_export${suffix}.${randomUUID()}.tmp`);
    for (const reservedPath of [exportPath, failedExportPath, exportStderrPath]) {
        if (existsSync(reservedPath)) throw new Error(`[runner:opencode_export] refusing to replace existing evidence: ${reservedPath}`);
    }
    writeFileSync(exportStderrPath, "", { flag: "wx", mode: 0o600 });
    const result = await runProcess(
        { ...plan, args: ["export", sessionId], stdin: null },
        OPENCODE_EXPORT_TIMEOUT_MS,
        { stderr: exportStderrPath },
        null,
        null,
        {
            terminationGraceMs: PROCESS_TERMINATION_GRACE_MS,
            forceSettleGraceMs: PROCESS_FORCE_SETTLE_GRACE_MS,
            directStdout: { path: temporaryExportPath, maxBytes: OPENCODE_EXPORT_MAX_BYTES },
        },
    );
    const temporaryDigest = fileDigest(temporaryExportPath);
    const stderrDigest = fileDigest(exportStderrPath);
    const record = {
        session_id: sessionId,
        command: { command: plan.command, args: ["export", sessionId], cwd: plan.cwd },
        capture: {
            stdout_transport: "direct_file_descriptor",
            temporary_path: temporaryExportPath,
            final_path: exportPath,
            failed_path: failedExportPath,
            stderr_path: exportStderrPath,
            max_bytes: OPENCODE_EXPORT_MAX_BYTES,
            max_mebibytes: OPENCODE_EXPORT_MAX_BYTES / (1024 * 1024),
        },
        exit_code: result.exitCode,
        signal: result.signal ?? null,
        timed_out: result.timedOut,
        timeout_kind: result.timeoutKind ?? null,
        spawn_error: result.spawnError ?? null,
        runner_error: result.runnerError ?? null,
        termination: result.termination ?? null,
        stdout: temporaryDigest,
        stderr: stderrDigest,
        validation: {
            within_size_limit: temporaryDigest.exists && temporaryDigest.bytes <= OPENCODE_EXPORT_MAX_BYTES,
            json_complete: false,
            sha256_verified: false,
        },
    };
    const fail = (reason) => {
        if (existsSync(temporaryExportPath) && !existsSync(failedExportPath)) {
            promoteNewFileAtomically(temporaryExportPath, failedExportPath);
            record.stdout = fileDigest(failedExportPath);
            record.capture.temporary_path = null;
        }
        return { status: "protocol_failure", session_id: sessionId, export: record, final_answer: null, reason };
    };
    if (!temporaryDigest.exists) return fail("session_export_stdout_missing");
    if (temporaryDigest.bytes > OPENCODE_EXPORT_MAX_BYTES || result.termination?.reason === "stdout_size_limit") return fail("session_export_size_limit_exceeded");
    if (result.exitCode !== 0 || result.signal || result.timedOut || result.spawnError || result.runnerError || result.termination?.reason) {
        return fail("session_export_process_failure");
    }
    let parsed;
    try {
        parsed = JSON.parse(readFileSync(temporaryExportPath, "utf8"));
    } catch {
        return fail("session_export_invalid_json");
    }
    const verifiedDigest = fileDigest(temporaryExportPath);
    if (verifiedDigest.sha256 !== temporaryDigest.sha256 || verifiedDigest.bytes !== temporaryDigest.bytes) return fail("session_export_digest_changed_during_validation");
    record.validation.json_complete = true;
    record.validation.sha256_verified = true;
    promoteNewFileAtomically(temporaryExportPath, exportPath);
    record.capture.temporary_path = null;
    record.stdout = fileDigest(exportPath);
    return { status: "recovered", session_id: sessionId, export: record, final_answer: assistantTextFromSessionExport(parsed) };
}

function resolveOpencodeFinal(stdoutExtraction, sessionRecovery) {
    if (stdoutExtraction.protocol.status !== "valid") return { status: "protocol_invalid", answer: "", provenance: null, reason: "stdout_protocol" };
    if (sessionRecovery.status === "protocol_failure" || sessionRecovery.status === "protocol_invalid" || sessionRecovery.status === "recovery_error") {
        return { status: "protocol_invalid", answer: "", provenance: null, reason: sessionRecovery.status };
    }
    const stdoutAnswer = stdoutExtraction.rawAnswer.trim();
    const sessionAnswer = sessionRecovery.final_answer?.trim() ?? "";
    if (stdoutAnswer && sessionAnswer && stdoutAnswer !== sessionAnswer) {
        return { status: "protocol_invalid", answer: "", provenance: null, reason: "stdout_session_mismatch" };
    }
    if (stdoutAnswer) return { status: "success", answer: stdoutAnswer, provenance: "stdout", reason: null };
    if (sessionAnswer) return { status: "success", answer: sessionAnswer, provenance: "session_recovery", reason: null };
    if (sessionRecovery.status === "unavailable") return { status: "protocol_invalid", answer: "", provenance: null, reason: "session_recovery_unavailable" };
    return { status: "model_no_final", answer: "", provenance: "absent", reason: "stdout_and_session_absent" };
}

function protocolV2SyntheticReplay() {
    const valid = parseOpencodeProtocolV2(Buffer.from('{"type":"text","sessionID":"s1","part":{"text":"answer"}}\n'));
    const noFinal = parseOpencodeProtocolV2(Buffer.from('{"type":"step_finish","sessionID":"s1","part":{"tokens":{"input":1,"output":1}}}\n'));
    const multiline = parseOpencodeProtocolV2(Buffer.from('{"type":"text",\n"part":{"text":"answer"}}\n'));
    const prefixed = parseOpencodeProtocolV2(Buffer.from('INFO {"type":"text","part":{"text":"answer"}}\n'));
    const truncated = parseOpencodeProtocolV2(Buffer.from('{"type":"text","part":{"text":"answer"}}'));
    const mismatch = resolveOpencodeFinal(valid, { status: "recovered", final_answer: "different answer" });
    const recovered = resolveOpencodeFinal(noFinal, { status: "recovered", final_answer: "session answer" });
    const noFinalResolution = resolveOpencodeFinal(noFinal, { status: "recovered", final_answer: null });
    const proof = {
        protocol_version: OPENCODE_PROTOCOL_VERSION,
        cases: {
            valid_final: valid.protocol.status === "valid" && valid.rawAnswer === "answer",
            no_final: noFinalResolution.status === "model_no_final",
            multiline: multiline.protocol.status === "protocol_invalid",
            prefixed: prefixed.protocol.status === "protocol_invalid",
            truncated: truncated.protocol.status === "protocol_invalid",
            stdout_session_mismatch: mismatch.status === "protocol_invalid",
            session_recovery: recovered.status === "success" && recovered.provenance === "session_recovery",
        },
    };
    proof.passed = Object.values(proof.cases).every(Boolean);
    if (!proof.passed) throw new Error("[runner:protocol-v2] synthetic replay failed");
    return proof;
}

function processEnvironmentIdentity(env) {
    const selected = Object.fromEntries([
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "CODEMAP_HOME",
        "HOME",
    ].map((key) => [key, env?.[key] ?? null]));
    if (env?.[OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE] != null) {
        selected[OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE] = env[OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE];
    }
    return { selected, sha256: sha256(canonicalJson(selected)) };
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

function buildMcpConfigForArm(arm, targetRoot, episodeDir, codemapCommand) {
    const mcpConfigPath = path.join(episodeDir, "mcp_config.json");
    if (arm.backend === "codemap") {
        writeJson(mcpConfigPath, {
            mcpServers: {
                "codemap-search": {
                    command: codemapCommand[0],
                    args: [...codemapCommand.slice(1), "mcp"],
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

function buildOpencodeXdgConfig(arm, targetRoot, episodeDir, codemapCommand) {
    const xdgHome = path.join(episodeDir, "opencode-xdg");
    const xdgCacheHome = path.join(episodeDir, "opencode-xdg-cache");
    const xdgDataHome = path.join(episodeDir, "opencode-xdg-data");
    const opencodeConfigDir = path.join(xdgHome, "opencode");
    const opencodeDataDir = path.join(xdgDataHome, "opencode");
    for (const directory of [opencodeConfigDir, xdgCacheHome, opencodeDataDir]) {
        mkdirSync(directory, { recursive: true, mode: 0o700 });
        chmodSync(directory, 0o700);
    }

    // OpenCode 1.17.18 stores logs and authentication below XDG_DATA_HOME. Keep the
    // mutable data and log path episode-local, but seed only the existing auth record
    // so the isolated client can still use the fixed Ollama Cloud model credential.
    const inheritedDataHome = process.env.XDG_DATA_HOME ?? path.join(os.homedir(), ".local", "share");
    const inheritedAuthPath = path.join(inheritedDataHome, "opencode", "auth.json");
    const episodeAuthPath = path.join(opencodeDataDir, "auth.json");
    if (existsSync(inheritedAuthPath)) {
        copyFileSync(inheritedAuthPath, episodeAuthPath);
        chmodSync(episodeAuthPath, 0o600);
    }

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
            command: [...codemapCommand, "mcp"],
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
        agent: { title: { disable: true } },
        tools: toolsConfig,
        permission: permissionConfig,
        mcp: mcpSection,
    };
    const configPath = path.join(opencodeConfigDir, "opencode.jsonc");
    writeFileSync(configPath, JSON.stringify(config, null, 2));
    return { xdgHome, xdgCacheHome, xdgDataHome, configPath };
}

// ============================================================
// 명령 빌드 + 실행
// ============================================================

/**
 * arm 설정과 episode 정보로 실행 명령을 빌드한다.
 * Returns: { command, args, env, cwd, lastMessagePath? }
 */
function buildEpisodeCommand(arm, targetRoot, prompt, episodeDir, codemapCommand, codemapHome) {
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
            const mcpConfigPath = buildMcpConfigForArm(arm, targetRoot, episodeDir, codemapCommand);
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
            env: { ...process.env, ...(codemapHome ? { CODEMAP_HOME: codemapHome } : {}) },
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
                    `mcp_servers.codemap-search.command=${codemapCommand[0]}`,
                    "-c",
                    `mcp_servers.codemap-search.args=${JSON.stringify([...codemapCommand.slice(1), "mcp"])}`,
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
        const isRun07OllamaCodemapArm = arm.backend === "codemap" && arm.model === "ollama-cloud/deepseek-v4-flash";
        const canonicalTargetRoot = isRun07OllamaCodemapArm ? realpathSync(targetRoot) : targetRoot;
        const { xdgHome, xdgCacheHome, xdgDataHome, configPath } = buildOpencodeXdgConfig(arm, canonicalTargetRoot, episodeDir, codemapCommand);
        // --format json: JSONL events(tool_use/text/step_finish) → 견고한 answer/tool/token 추출.
        // 기존 ANSI stdout 파싱은 fragile했고 tool_events/tokens를 못 얻었음.
        const args = isRun07OllamaCodemapArm
            ? ["run", "--dir", canonicalTargetRoot, "--model", arm.model, "--format", "json"]
            : ["run", "--model", arm.model, "--format", "json"];
        // OpenCode 1.17.18 otherwise initializes FSEvents before the model call.
        // This read-only benchmark arm reads files on demand and does not need live fixture notifications.
        const fileWatcherEnvironment = isRun07OllamaCodemapArm
            ? { [OPENCODE_FILE_WATCHER_DISABLE_ENVIRONMENT_VARIABLE]: OPENCODE_FILE_WATCHER_DISABLE_VALUE }
            : {};

        return {
            command: "opencode",
            args,
            env: { ...process.env, XDG_CONFIG_HOME: xdgHome, XDG_CACHE_HOME: xdgCacheHome, XDG_DATA_HOME: xdgDataHome, ...fileWatcherEnvironment, ...(codemapHome ? { CODEMAP_HOME: codemapHome } : {}) },
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
function runProcess(plan, timeoutMs, streamPaths = null, abortSignal = null, onStarted = null, options = {}) {
    const noOutputTimeoutMs = options.noOutputTimeoutMs ?? null;
    const terminationGraceMs = options.terminationGraceMs ?? PROCESS_TERMINATION_GRACE_MS;
    const forceSettleGraceMs = options.forceSettleGraceMs ?? PROCESS_FORCE_SETTLE_GRACE_MS;
    const directStdout = options.directStdout ?? null;
    if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) return Promise.reject(new Error("[runner:timeout] timeoutMs must be positive"));
    if (noOutputTimeoutMs != null && (!Number.isFinite(noOutputTimeoutMs) || noOutputTimeoutMs <= 0)) {
        return Promise.reject(new Error("[runner:timeout] noOutputTimeoutMs must be positive when set"));
    }
    if (directStdout != null) {
        if (typeof directStdout.path !== "string" || directStdout.path.length === 0) return Promise.reject(new Error("[runner:stdout] directStdout.path is required"));
        if (streamPaths?.stdout) return Promise.reject(new Error("[runner:stdout] directStdout and streamPaths.stdout are mutually exclusive"));
        if (!Number.isSafeInteger(directStdout.maxBytes) || directStdout.maxBytes <= 0) return Promise.reject(new Error("[runner:stdout] directStdout.maxBytes must be a positive safe integer"));
    }

    return new Promise((resolve) => {
        const startedAt = Date.now();
        let lastOutputAt = startedAt;
        let outputBytes = 0;
        let timedOut = false;
        let timeoutKind = null;
        let spawnError = null;
        let runnerError = null;
        let stdinError = null;
        let settled = false;
        let child = null;
        let childPid = null;
        let spawnAttempted = false;
        let stdoutFd = null;
        let stderrFd = null;
        let overallTimer = null;
        let noOutputTimer = null;
        let forceKillTimer = null;
        let forceSettleTimer = null;
        let spawnErrorSettleTimer = null;
        let directStdoutSizeTimer = null;
        let terminationReason = null;
        let terminationRequestedAt = null;
        let gracefulSignalEvidence = null;
        let forcedSignalEvidence = null;
        let forcedSettlement = false;
        let cleanedUp = false;
        const stdoutChunks = [];
        const stderrChunks = [];
        const streamLifecycle = {
            stdout: directStdout?.path || streamPaths?.stdout ? { path: directStdout?.path ?? streamPaths.stdout, opened_at: null, flushed_at: null, closed_at: null, capture: directStdout ? "direct_file_descriptor" : "pipe" } : null,
            stderr: streamPaths?.stderr ? { path: streamPaths.stderr, opened_at: null, flushed_at: null, closed_at: null } : null,
        };

        function closeStreamFds() {
            for (const [name, fd] of [["stdout", stdoutFd], ["stderr", stderrFd]]) {
                if (fd == null) continue;
                try {
                    fsyncSync(fd);
                    streamLifecycle[name].flushed_at = new Date().toISOString();
                } catch {}
                try {
                    closeSync(fd);
                    streamLifecycle[name].closed_at = new Date().toISOString();
                } catch {}
            }
            stdoutFd = null;
            stderrFd = null;
        }

        function signalProcessGroup(signal) {
            const evidence = {
                signal,
                attempted_at: new Date().toISOString(),
                process_group_id: childPid,
                process_group_signaled: false,
                child_signaled: false,
                errors: [],
            };
            if (childPid == null) {
                evidence.errors.push("child_pid_unavailable");
                return evidence;
            }
            try {
                process.kill(-childPid, signal);
                evidence.process_group_signaled = true;
                return evidence;
            } catch (error) {
                evidence.errors.push(`process_group:${error?.code ?? error?.message ?? error}`);
            }
            try {
                evidence.child_signaled = Boolean(child?.kill(signal));
            } catch (error) {
                evidence.errors.push(`child:${error?.code ?? error?.message ?? error}`);
            }
            return evidence;
        }

        function cleanupBackendProcesses() {
            if (cleanedUp) return;
            cleanedUp = true;
            if (terminationReason) return;
            signalProcessGroup("SIGTERM");
            setTimeout(() => signalProcessGroup("SIGKILL"), PROCESS_TERMINATION_GRACE_MS).unref();
        }

        function finish(code, signal, closeObserved) {
            if (settled) return;
            settled = true;
            for (const timer of [overallTimer, noOutputTimer, forceKillTimer, forceSettleTimer, spawnErrorSettleTimer]) clearTimeout(timer);
            clearInterval(directStdoutSizeTimer);
            abortSignal?.removeEventListener("abort", abortProcess);
            child?.stdout?.off?.("data", onStdout);
            child?.stderr?.off("data", onStderr);
            child?.off("close", onClose);
            child?.off("error", onChildError);
            cleanupBackendProcesses();
            closeStreamFds();
            const stdoutBytes = Buffer.concat(stdoutChunks);
            const stderrBytes = Buffer.concat(stderrChunks);
            let directStdoutBytes = 0;
            try { directStdoutBytes = directStdout ? statSync(directStdout.path).size : 0; } catch {}
            resolve({
                exitCode: spawnError ? null : code,
                signal: signal ?? null,
                spawn_attempted: spawnAttempted,
                stdout: stdoutBytes.toString("utf8"),
                stderr: stderrBytes.toString("utf8"),
                stdoutBytes,
                stderrBytes,
                timedOut,
                timeoutKind,
                spawnError: spawnError ? String(spawnError) : null,
                runnerError: runnerError ? String(runnerError) : null,
                stdinError: stdinError ? String(stdinError) : null,
                elapsedMs: Date.now() - startedAt,
                backend_process_group_cleaned: childPid != null,
                raw_stream_lifecycle: streamLifecycle,
                direct_stdout: directStdout ? { path: directStdout.path, bytes: directStdoutBytes, max_bytes: directStdout.maxBytes } : null,
                termination: {
                    reason: terminationReason,
                    requested_at: terminationRequestedAt,
                    graceful_signal: gracefulSignalEvidence,
                    forced_signal: forcedSignalEvidence,
                    grace_ms: terminationGraceMs,
                    force_settle_grace_ms: forceSettleGraceMs,
                    forced_settlement: forcedSettlement,
                    close_observed: closeObserved,
                    overall_timeout_ms: timeoutMs,
                    no_output_timeout_ms: noOutputTimeoutMs,
                    last_output_at: new Date(lastOutputAt).toISOString(),
                    output_bytes: outputBytes + directStdoutBytes,
                },
            });
        }

        function requestTermination(reason, isTimeout = false) {
            if (settled || terminationReason) return;
            terminationReason = reason;
            terminationRequestedAt = new Date().toISOString();
            if (isTimeout) {
                timedOut = true;
                timeoutKind = reason;
            }
            gracefulSignalEvidence = signalProcessGroup("SIGTERM");
            forceKillTimer = setTimeout(() => {
                forcedSignalEvidence = signalProcessGroup("SIGKILL");
                forceSettleTimer = setTimeout(() => {
                    forcedSettlement = true;
                    child?.stdin?.destroy();
                    child?.stdout?.destroy();
                    child?.stderr?.destroy();
                    child?.unref();
                    finish(null, null, false);
                }, forceSettleGraceMs);
            }, terminationGraceMs);
        }

        function resetNoOutputTimer() {
            clearTimeout(noOutputTimer);
            if (noOutputTimeoutMs == null || settled || terminationReason) return;
            noOutputTimer = setTimeout(() => requestTermination("no_output_timeout", true), noOutputTimeoutMs);
        }

        function checkDirectStdoutSize() {
            if (!directStdout || settled || terminationReason) return;
            try {
                if (statSync(directStdout.path).size > directStdout.maxBytes) {
                    runnerError = new Error(`[runner:stdout_size] direct stdout exceeded ${directStdout.maxBytes} bytes`);
                    requestTermination("stdout_size_limit");
                }
            } catch (error) {
                runnerError = new Error(`[runner:stdout_stat] ${error?.message ?? error}`);
                requestTermination("stdout_stat_error");
            }
        }

        function failStreamWrite(error) {
            runnerError = new Error(`[runner:stream_write] ${error?.message ?? error}`);
            requestTermination("stream_write_error");
        }

        function abortProcess() {
            requestTermination("abort_signal");
        }

        function recordOutput(chunk, chunks, fd) {
            const bytes = Buffer.from(chunk);
            chunks.push(bytes);
            outputBytes += bytes.length;
            lastOutputAt = Date.now();
            resetNoOutputTimer();
            try { if (fd != null) writeFileSync(fd, bytes); } catch (error) { failStreamWrite(error); }
        }

        function onStdout(chunk) {
            recordOutput(chunk, stdoutChunks, stdoutFd);
        }

        function onStderr(chunk) {
            recordOutput(chunk, stderrChunks, stderrFd);
        }

        function onClose(code, signal) {
            finish(code, signal, true);
        }

        function onChildError(error) {
            spawnError = error;
            if (childPid == null && !settled) {
                spawnErrorSettleTimer = setTimeout(() => finish(null, null, false), 0);
            }
        }

        try {
            if (directStdout) {
                mkdirSync(path.dirname(directStdout.path), { recursive: true });
                stdoutFd = openSync(directStdout.path, "wx", 0o600);
                streamLifecycle.stdout.opened_at = new Date().toISOString();
            } else if (streamPaths?.stdout) {
                stdoutFd = openSync(streamPaths.stdout, "a");
                streamLifecycle.stdout.opened_at = new Date().toISOString();
            }
            if (streamPaths?.stderr) {
                stderrFd = openSync(streamPaths.stderr, "a");
                streamLifecycle.stderr.opened_at = new Date().toISOString();
            }
        } catch (error) {
            runnerError = new Error(`[runner:stream_open] ${error?.message ?? error}`);
            finish(null, null, false);
            return;
        }

        try {
            spawnAttempted = true;
            child = spawn(plan.command, plan.args, {
                cwd: plan.cwd,
                env: plan.env,
                stdio: ["pipe", directStdout ? stdoutFd : "pipe", "pipe"],
                detached: true,
            });
            childPid = child.pid ?? null;
        } catch (error) {
            spawnError = error;
            finish(null, null, false);
            return;
        }

        child.stdout?.on("data", onStdout);
        child.stderr.on("data", onStderr);
        child.on("close", onClose);
        child.on("error", onChildError);
        child.stdin.on("error", (error) => { stdinError = error; });

        try {
            onStarted?.({
                pid: childPid,
                process_group_id: childPid,
                started_at: new Date().toISOString(),
                timeout_ms: timeoutMs,
                no_output_timeout_ms: noOutputTimeoutMs,
                termination_grace_ms: terminationGraceMs,
            });
        } catch (error) {
            runnerError = new Error(`[runner:on_started] ${error?.message ?? error}`);
            requestTermination("on_started_callback_error");
        }

        overallTimer = setTimeout(() => requestTermination("overall_timeout", true), timeoutMs);
        resetNoOutputTimer();
        if (directStdout) {
            directStdoutSizeTimer = setInterval(checkDirectStdoutSize, 100);
            directStdoutSizeTimer.unref();
        }
        if (abortSignal) {
            if (abortSignal.aborted) abortProcess();
            else abortSignal.addEventListener("abort", abortProcess, { once: true });
        }

        try {
            if (plan.stdin) child.stdin.write(plan.stdin, "utf8");
            child.stdin.end();
        } catch (error) {
            stdinError = error;
            requestTermination("stdin_write_error");
        }
    });
}

function processFailureKind(processResult, error = null) {
    if (processResult?.timeoutKind) return processResult.timeoutKind;
    if (processResult?.spawnError) return "spawn_error";
    if (processResult?.termination?.reason === "abort_signal") return "abort_signal";
    if (processResult?.termination?.reason) return processResult.termination.reason;
    if (processResult?.signal) return "signal_exit";
    if (processResult?.runnerError) return "runner_process_error";
    if (processResult?.exitCode != null && processResult.exitCode !== 0) return "nonzero_exit";
    return error ? "runner_exception" : null;
}

function processFailureMessage(processResult, error = null) {
    const failureKind = processFailureKind(processResult, error);
    if (!failureKind) return null;
    if (failureKind === "no_output_timeout" || failureKind === "overall_timeout") {
        const timeoutMs = failureKind === "no_output_timeout"
            ? processResult?.termination?.no_output_timeout_ms
            : processResult?.termination?.overall_timeout_ms;
        return `${failureKind} after ${timeoutMs ?? "unknown"}ms`;
    }
    if (processResult?.spawnError) return `spawn_error: ${processResult.spawnError}`;
    if (processResult?.signal) return `signal_exit: ${processResult.signal}`;
    if (processResult?.runnerError) return `${failureKind}: ${processResult.runnerError}`;
    if (processResult?.exitCode != null && processResult.exitCode !== 0) return `nonzero_exit: ${processResult.exitCode}`;
    return error ? String(error) : failureKind;
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

class PhaseBarrier {
    constructor(expectedParticipants) {
        this.expectedParticipants = expectedParticipants;
        this.arrivedParticipants = 0;
        this.released = expectedParticipants === 0;
        this.promise = new Promise((resolve) => { this.resolve = resolve; });
        if (this.released) this.resolve();
    }

    arrive() {
        if (!this.released) {
            this.arrivedParticipants++;
            this.releaseIfReady();
        }
        return this.promise;
    }

    withdraw() {
        if (!this.released) {
            this.expectedParticipants--;
            this.releaseIfReady();
        }
    }

    releaseIfReady() {
        if (!this.released && this.arrivedParticipants >= this.expectedParticipants) {
            this.released = true;
            this.resolve();
        }
    }
}

function createWaveParticipant(solverStartBarrier, scorerStartBarrier) {
    let solverStartSettled = false;
    let scorerStartSettled = false;
    return {
        async awaitSolverStart() {
            if (solverStartSettled) return;
            solverStartSettled = true;
            await solverStartBarrier.arrive();
        },
        async awaitScorerStart() {
            if (scorerStartSettled) return;
            scorerStartSettled = true;
            await scorerStartBarrier.arrive();
        },
        withdrawPending() {
            if (!solverStartSettled) {
                solverStartSettled = true;
                solverStartBarrier.withdraw();
            }
            if (!scorerStartSettled) {
                scorerStartSettled = true;
                scorerStartBarrier.withdraw();
            }
        },
    };
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
    const preflightSemaphore = config.preflightSemaphore ?? (config.preflightSemaphore = new Semaphore(1));
    const arm = ep.arm_id ?? ep.arm;
    const round = canonicalRound(ep.round);
    const allocation = allocationEpisodeFromInput(ep);
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
    const episodeCodemapBin = armDef.backend === "codemap" ? cleanArtifactBinary(taskDef, arm) : null;
    const codemapIdentity = armDef.backend === "codemap" ? codemapBinaryIdentity(episodeCodemapBin) : null;
    const episodeDir = path.join(config.outRoot, arm, ep.codebase, `round-${round}`);
    const wrapper = armDef.backend === "codemap"
        ? writeCodemapExecWrapper({ episodeDir, allocationId: allocation.allocation_id, armId: arm, binary: codemapIdentity })
        : null;
    const fixtureContractValue = { ...fixtureContract(taskDef, ep.codebase, armDef, codemapIdentity), clean: taskDef.fixture.clean };
    const fixtureContractSha256 = sha256(JSON.stringify(fixtureContractValue));
    const preflightKey = JSON.stringify({ root: fixtureIdentity.realpath, runtime, codemap: codemapIdentity, fixture_contract_sha256: fixtureContractSha256 });
    let preflight = preflightCache.get(preflightKey);
    if (!preflight) {
        preflight = (async () => {
            await preflightSemaphore.acquire();
            try {
                return await preflightFixture({ taskDef, taskId: ep.codebase, targetRoot, armDef, codemapCommand: wrapper.command, wrapper, identity: fixtureIdentity });
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
    const plan = buildEpisodeCommand(armDef, targetRoot, prompt, episodeDir, wrapper?.command ?? null, fixture.proof.codemap_home ?? null);
    const resumeIdentity = buildResumeIdentity({ armDef, promptPath: publicQuestionPath, prompt, config, contract, fixture, evaluationContract: taskDef.evaluation, runtime, episode: { arm_id: arm, codebase: ep.codebase, round }, plan, codemapIdentity });
    return { armDef, episodeDir, resumeIdentity, fixture, runtime, targetRoot, publicQuestionPath, privateAnswerKeyPath, schemaPath, codemapBin: episodeCodemapBin, codemapIdentity, wrapper };
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
    const onPhase = typeof hooks.onPhase === "function" ? hooks.onPhase : () => {};
    const coTenancySnapshot = typeof hooks.coTenancySnapshot === "function" ? hooks.coTenancySnapshot : () => null;
    const awaitSolverStartBarrier = typeof hooks.awaitSolverStartBarrier === "function" ? hooks.awaitSolverStartBarrier : async () => {};
    const awaitScorerStartBarrier = typeof hooks.awaitScorerStartBarrier === "function" ? hooks.awaitScorerStartBarrier : async () => {};
    const { armConfig, manifest, scorerPath, schemaDir, outRoot, timeoutMs, judgeModel, printCmd, skipScorer, codemapBin, workspaceRoot } = config;

    const armDef = armConfig.arms.find((a) => a.arm_id === arm);
    if (!armDef) throw new Error(`arm_id not found: ${arm}`);
    const noOutputTimeoutMs = effectiveOpenCodeNoOutputTimeoutMs(armDef.runtime, config);
    const processTimeoutPolicy = {
        overall_timeout_ms: timeoutMs,
        no_output_timeout_ms: noOutputTimeoutMs,
        termination_grace_ms: PROCESS_TERMINATION_GRACE_MS,
        force_settle_grace_ms: PROCESS_FORCE_SETTLE_GRACE_MS,
    };

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
    const allocation = allocationEpisodeFromInput(ep);

    // Allocation IDs are permanent experimental units. A runner code change must never
    // make an earlier started, invalid, or terminal unit eligible for another model call.
    assertAllocationEpisodeUnused(outRoot, allocation.allocation_id);

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
    let allocationClaim;
    try {
        allocationClaim = acquireAllocationClaim(outRoot, allocation.allocation_id);
    } catch (error) {
        releaseEpisodeClaim(claimPath);
        throw error;
    }
    let terminalAppended = false;
    let terminalLedgerBase = null;
    let terminalAttemptId = null;
    let durableAttemptPathForFailure = null;
    let allocationStarted = false;
    let allocationTerminal = false;
    let snapshotBefore = null;
    let attemptDirForFailure = null;
    let processResultForFailure = null;
    let solveStartedAtForFailure = null;
    let solveFinishedAtForFailure = null;
    let durableSessionRecorded = false;
    let partialSessionId = null;
    let coTenancyAtStart = null;
    let episodeFailure = null;
    try {

    mkdirSync(episodeDir, { recursive: true });

    console.log(`[episode:start] ${episodeId}`);

    // --- mutation guard: before snapshot ---
    snapshotBefore = snapshotTargetRoot(targetRoot);
    writeJsonAtomically(path.join(episodeDir, "mutation_guard_before.json"), snapshotBefore);

    // --- 명령 빌드 ---
    if (armDef.backend === "codemap") {
        assertSealedCodemapBinary(prepared.codemapIdentity);
        if (!prepared.wrapper?.command || prepared.wrapper.expected.sha256 !== prepared.codemapIdentity.sha256) {
            throw new Error(`[runner:binary] missing arm-specific MCP wrapper for ${allocation.allocation_id}`);
        }
        // Preflight may run in a separately prepared plan and leave no attestation file
        // when its MCP child exited before startup. Absence is not a solver result; ensure
        // the runtime directory exists and only remove an actual preflight record.
        mkdirSync(path.dirname(prepared.wrapper.attestationPath), { recursive: true });
        if (existsSync(prepared.wrapper.attestationPath)) unlinkSync(prepared.wrapper.attestationPath);
    }
    const plan = buildEpisodeCommand(armDef, targetRoot, prompt, episodeDir, armDef.backend === "codemap" ? prepared.wrapper.command : null, fixture.proof.codemap_home ?? null);
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
    attemptDirForFailure = attemptDir;
    mkdirSync(attemptDir, { recursive: true });
    const attemptStdoutPath = path.join(attemptDir, "stdout.txt");
    const attemptStderrPath = path.join(attemptDir, "stderr.txt");
    const durableAttemptPath = path.join(attemptDir, "durable_attempt.json");
    durableAttemptPathForFailure = durableAttemptPath;
    writeFileSync(attemptStdoutPath, "");
    writeFileSync(attemptStderrPath, "");
    const ledgerBase = {
        identity_schema_version: IDENTITY_SCHEMA_VERSION,
        attempt_id: attemptId,
        attempt_dir: attemptDir,
        allocation_id: allocation.allocation_id,
        replacement: allocation.replacement,
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
        solver_contract_hash: resumeIdentity.solver_contract_hash,
        runner_operational_hash: resumeIdentity.runner_operational_hash,
        final_identity_sha256: resumeIdentity.sha256,
    };
    terminalLedgerBase = ledgerBase;
    appendAllocationLedger(outRoot, {
        event: "started",
        timestamp: new Date().toISOString(),
        allocation_id: allocation.allocation_id,
        logical_episode_id: episodeId,
        replacement: allocation.replacement,
        attempt_id: attemptId,
        solver_contract_hash: resumeIdentity.solver_contract_hash,
        runner_operational_hash: resumeIdentity.runner_operational_hash,
    });
    allocationStarted = true;
    onPhase("allocation_reserved", { allocation_id: allocation.allocation_id, attempt_id: attemptId });
    appendAttemptLedger(outRoot, { ...ledgerBase, event: reusableSolverAttempt ? "scorer_started" : "started", timestamp: new Date().toISOString(), reused_solver_attempt_id: reusableSolverAttempt?.attempt_id ?? null });
    writeJsonAtomically(durableAttemptPath, {
        state: reusableSolverAttempt ? "scorer_started" : "prepared",
        ...ledgerBase,
        argv: { command: plan.command, args: plan.args, cwd: plan.cwd, codemap_home: plan.env.CODEMAP_HOME ?? null },
        paths: { stdout: attemptStdoutPath, stderr: attemptStderrPath, terminal: path.join(attemptDir, "durable_terminal.json"), scorer: path.join(attemptDir, "durable_scorer.json") },
        timeout_policy: processTimeoutPolicy,
        started_at: new Date().toISOString(),
    });
    writeJson(path.join(attemptDir, "attempt_metadata.json"), { ...ledgerBase, fixture_proof: fixture.proof, resume_identity: resumeIdentity });
    onPhase("solver_ready", { allocation_id: allocation.allocation_id, attempt_id: attemptId, ready_at: new Date().toISOString() });
    await awaitSolverStartBarrier();
    coTenancyAtStart = coTenancySnapshot();
    const solveStartedAt = new Date();
    solveStartedAtForFailure = solveStartedAt;
    const processResult = reusableSolverAttempt
        ? (() => {
            const prior = readJson(path.join(reusableSolverAttempt.attempt_dir, "process_result.json"));
            copyFileSync(path.join(reusableSolverAttempt.attempt_dir, "stdout.txt"), attemptStdoutPath);
            copyFileSync(path.join(reusableSolverAttempt.attempt_dir, "stderr.txt"), attemptStderrPath);
            return { exitCode: prior.exitCode, signal: prior.signal, timedOut: prior.timedOut, timeoutKind: prior.timeoutKind ?? null, spawnError: prior.spawnError, runnerError: prior.runnerError ?? null, termination: prior.termination ?? null, elapsedMs: prior.elapsedMs, wall_time_s: prior.wall_time_s, spawn_attempted: false, reused_solver_attempt_id: reusableSolverAttempt.attempt_id, stdout: readFileSync(attemptStdoutPath, "utf8"), stderr: readFileSync(attemptStderrPath, "utf8"), stdoutBytes: readFileSync(attemptStdoutPath), stderrBytes: readFileSync(attemptStderrPath), backend_process_group_cleaned: prior.backend_process_group_cleaned ?? false };
        })()
        : await runProcess(plan, timeoutMs, { stdout: attemptStdoutPath, stderr: attemptStderrPath }, hooks.abortSignal ?? null, (processState) => {
            writeJsonAtomically(durableAttemptPath, {
                state: "started",
                ...ledgerBase,
                argv: { command: plan.command, args: plan.args, cwd: plan.cwd, codemap_home: plan.env.CODEMAP_HOME ?? null },
                paths: { stdout: attemptStdoutPath, stderr: attemptStderrPath, terminal: path.join(attemptDir, "durable_terminal.json"), scorer: path.join(attemptDir, "durable_scorer.json") },
                timeout_policy: processTimeoutPolicy,
                ...processState,
            });
            onPhase("solver_started", { allocation_id: allocation.allocation_id, attempt_id: attemptId, ...processState });
        }, {
            noOutputTimeoutMs,
            terminationGraceMs: PROCESS_TERMINATION_GRACE_MS,
            forceSettleGraceMs: PROCESS_FORCE_SETTLE_GRACE_MS,
        });
    const solveFinishedAt = new Date();
    solveFinishedAtForFailure = solveFinishedAt;
    processResultForFailure = processResult;
    const solveElapsedMs = processResult.elapsedMs;
    const wallTimeS = processResult.elapsedMs / 1000;

    console.log(
        `[episode:done] ${episodeId} exit=${processResult.exitCode} signal=${processResult.signal ?? "none"} wall_time_s=${wallTimeS.toFixed(1)} timed_out=${processResult.timedOut} timeout_kind=${processResult.timeoutKind ?? "none"}`,
    );

    copyFileSync(attemptStdoutPath, path.join(episodeDir, "stdout.txt"));
    copyFileSync(attemptStderrPath, path.join(episodeDir, "stderr.txt"));
    let opencodeExtraction = null;
    let opencodeSessionRecovery = null;
    let opencodeFinal = null;
    let opencodeProtocolRecord = null;
    if (armDef.runtime.startsWith("opencode-")) {
        // Protocol-v2 deliberately reads only the closed, fsync'd raw file. The in-memory
        // process buffer is retained for legacy runtimes but is never a parsing source here.
        opencodeExtraction = parseOpencodeProtocolV2(readFileSync(attemptStdoutPath));
        partialSessionId = opencodeExtraction.protocol.session_id;
        const observedProcessFailure = processFailureMessage(processResult);
        const observedProcessFailureKind = processFailureKind(processResult);
        writeDurableSessionSidecar({
            outRoot,
            attemptDir,
            ledgerBase,
            ep,
            armDef,
            prepared,
            processResult,
            solveStartedAt,
            solveFinishedAt,
            tokens: opencodeExtraction.tokens,
            toolEvents: opencodeExtraction.toolEvents,
            parserStatus: { status: opencodeExtraction.protocol.status, session_id: opencodeExtraction.protocol.session_id },
            resultPaths: { stdout: attemptStdoutPath, stderr: attemptStderrPath, episode_dir: episodeDir, session_export: path.join(attemptDir, "opencode_session_export.json") },
            error: observedProcessFailure,
            missingEvidenceReasons: {
                session_id: observedProcessFailureKind
                    ? `${observedProcessFailureKind}:session_id_not_observed`
                    : `protocol_${opencodeExtraction.protocol.status}:session_id_not_observed`,
                tokens: observedProcessFailureKind
                    ? `${observedProcessFailureKind}:tokens_not_observed`
                    : `protocol_${opencodeExtraction.protocol.status}:tokens_not_observed`,
            },
        });
        durableSessionRecorded = true;
        if (opencodeExtraction.tokens?.accounting_status !== "complete") {
            throw new Error(`[runner:tokens] incomplete OpenCode token accounting for ${episodeId}: ${JSON.stringify(opencodeExtraction.tokens?.incomplete_fields ?? ["step_finish_tokens_absent"])}`);
        }
        opencodeSessionRecovery = await recoverOpencodeSessionFinal(plan, opencodeExtraction.protocol.session_id, attemptDir);
        opencodeFinal = resolveOpencodeFinal(opencodeExtraction, opencodeSessionRecovery);
        opencodeProtocolRecord = {
            protocol_version: OPENCODE_PROTOCOL_VERSION,
            framing_version: OPENCODE_FRAMING_VERSION,
            opencode_version: solverRuntime.version,
            argv: { command: plan.command, args: plan.args, cwd: plan.cwd },
            environment: processEnvironmentIdentity(plan.env),
            raw_capture: {
                stdout: fileDigest(attemptStdoutPath),
                stderr: fileDigest(attemptStderrPath),
                lifecycle: processResult.raw_stream_lifecycle ?? null,
                parsed_after_stdout_closed_at: processResult.raw_stream_lifecycle?.stdout?.closed_at ?? null,
            },
            stdout: opencodeExtraction.protocol,
            session_recovery: opencodeSessionRecovery,
            final: {
                status: opencodeFinal.status,
                provenance: opencodeFinal.provenance,
                reason: opencodeFinal.reason,
                answer_sha256: sha256(opencodeFinal.answer),
                answer_bytes: Buffer.byteLength(opencodeFinal.answer),
            },
            process: {
                exit_code: processResult.exitCode,
                signal: processResult.signal,
                timed_out: processResult.timedOut,
                flush_closed_at: processResult.raw_stream_lifecycle?.stdout?.closed_at ?? null,
            },
            recorded_at: new Date().toISOString(),
        };
        writeJsonAtomically(path.join(attemptDir, "opencode_protocol_v2.json"), opencodeProtocolRecord);
        copyFileSync(path.join(attemptDir, "opencode_protocol_v2.json"), path.join(episodeDir, "opencode_protocol_v2.json"));
        if (existsSync(path.join(attemptDir, "opencode_session_export.json"))) {
            copyFileSync(path.join(attemptDir, "opencode_session_export.json"), path.join(episodeDir, "opencode_session_export.json"));
        }
    }
    writeJson(path.join(episodeDir, "process_result.json"), {
        exitCode: processResult.exitCode,
        signal: processResult.signal,
        timedOut: processResult.timedOut,
        timeoutKind: processResult.timeoutKind ?? null,
        spawnError: processResult.spawnError,
        runnerError: processResult.runnerError ?? null,
        termination: processResult.termination ?? null,
        elapsedMs: processResult.elapsedMs,
        wall_time_s: wallTimeS,
    });
    writeJsonAtomically(path.join(attemptDir, "durable_terminal.json"), {
        state: "terminal",
        ...ledgerBase,
        exit_code: processResult.exitCode,
        timed_out: processResult.timedOut,
        timeout_kind: processResult.timeoutKind ?? null,
        signal: processResult.signal,
        spawn_error: processResult.spawnError,
        runner_error: processResult.runnerError ?? null,
        termination: processResult.termination ?? null,
        wall_time_s: wallTimeS,
        stdout: fileDigest(attemptStdoutPath),
        stderr: fileDigest(attemptStderrPath),
        raw_stream_lifecycle: processResult.raw_stream_lifecycle ?? null,
        opencode_protocol_v2: opencodeProtocolRecord ? {
            path: path.join(attemptDir, "opencode_protocol_v2.json"),
            ...fileDigest(path.join(attemptDir, "opencode_protocol_v2.json")),
            final_status: opencodeFinal.status,
        } : null,
        terminal_at: new Date().toISOString(),
    });
    onPhase("solver_terminal", { allocation_id: allocation.allocation_id, attempt_id: attemptId, exit_code: processResult.exitCode, timed_out: processResult.timedOut });

    // --- mutation guard: after snapshot ---
    const snapshotAfter = snapshotTargetRoot(targetRoot);
    writeJsonAtomically(path.join(episodeDir, "mutation_guard_after.json"), snapshotAfter);

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
        rawAnswer = opencodeFinal?.answer ?? "";
        toolEvents = opencodeExtraction?.toolEvents ?? [];
        tokens = opencodeExtraction?.tokens ?? null;
        tokenSteps = opencodeExtraction?.stepTokens ?? [];
        extractionStatus = opencodeFinal?.status ?? "protocol_invalid";
    } else {
        extractionStatus = "unsupported_runtime";
    }

    // process 에러/timeout 시 extraction_status를 명시
    if (processResult.timedOut) extractionStatus = "timeout";
    else if (processResult.exitCode !== 0 && extractionStatus !== "success") extractionStatus = "process_error";

    if (!durableSessionRecorded) {
        const observedProcessFailure = processFailureMessage(processResult);
        const observedProcessFailureKind = processFailureKind(processResult);
        writeDurableSessionSidecar({
            outRoot,
            attemptDir,
            ledgerBase,
            ep,
            armDef,
            prepared,
            processResult,
            solveStartedAt,
            solveFinishedAt,
            tokens,
            toolEvents,
            parserStatus: { status: extractionStatus, session_id: null },
            resultPaths: { stdout: attemptStdoutPath, stderr: attemptStderrPath, episode_dir: episodeDir },
            error: observedProcessFailure,
            missingEvidenceReasons: {
                session_id: observedProcessFailureKind
                    ? `${observedProcessFailureKind}:session_id_not_observed`
                    : "runtime_does_not_report_session_id",
                tokens: observedProcessFailureKind
                    ? `${observedProcessFailureKind}:tokens_not_observed`
                    : "runtime_did_not_report_tokens",
            },
        });
        durableSessionRecorded = true;
    }
    if (armDef.backend === "codemap") {
        assertMcpStartupAttestation(prepared.wrapper);
    }

    const rawAnswerPath = path.join(episodeDir, "raw_answer.txt");
    const attemptRawAnswerPath = path.join(attemptDir, "raw_answer.txt");
    writeText(rawAnswerPath, rawAnswer);
    writeText(attemptRawAnswerPath, rawAnswer);

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

    // Solver 출력·원시 증거를 모두 기록한 뒤 슬롯을 해제한다. scorer는 solver wall-time
    // 측정과 겹치지 않도록 별도 단계에서 실행된다.
    releaseSlots();
    onPhase("solver_slots_released", { allocation_id: allocation.allocation_id, attempt_id: attemptId, released_at: new Date().toISOString() });
    await awaitScorerStartBarrier();
    onPhase("scorer_wave_released", { allocation_id: allocation.allocation_id, attempt_id: attemptId, released_at: new Date().toISOString() });

    // --- scorer.mjs 호출 (solver 슬롯 밖) ---
    let scorerOutput = null;
    let scorerScore = null;
    let judgeStatus = skipScorer ? "skipped" : "not_started";
    let judgeStartedAt = null; let judgeFinishedAt = null;
    let judgeElapsedMs = null;
    const episodeScorerOutputPath = path.join(episodeDir, "scorer_output.json");
    const attemptScorerOutputPath = path.join(attemptDir, "durable_scorer_output.json");

    if (skipScorer) {
        scorerOutput = { status: judgeStatus, reason: existsSync(scorerPath) ? "skip_scorer" : "scorer_missing" };
        writeJson(path.join(episodeDir, "scorer_output.json"), scorerOutput);
    } else if (
        extractionStatus === "success" &&
        rawAnswer.trim().length > 0 &&
        existsSync(schemaPath) &&
        existsSync(privateAnswerKeyPath)
    ) {
        const scorerOutPath = attemptScorerOutputPath;
        const scorerArgs = [
            scorerPath,
            "--raw-answer",
            attemptRawAnswerPath,
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
        writeJsonAtomically(path.join(attemptDir, "durable_scorer.json"), {
            state: "started",
            attempt_id: attemptId,
            episode_id: episodeId,
            scorer: { command: "node", argv: scorerArgs.slice(0, 1).concat(["--raw-answer", attemptRawAnswerPath, "--schema", schemaPath, "--out", scorerOutPath, "--judge-model", judgeModel]) },
            started_at: judgeStartedAt.toISOString(),
        });
        console.log(`[scorer:start] ${episodeId}`);
        const scorerResult = spawnSync("node", scorerArgs, {
            encoding: "utf8",
            maxBuffer: 32 * 1024 * 1024,
            timeout: 300_000, // scorer(judge) 5분 상한
        });
        judgeFinishedAt = new Date();
        judgeElapsedMs = judgeFinishedAt.getTime() - judgeStartedAt.getTime();
        writeJsonAtomically(path.join(attemptDir, "durable_scorer.json"), {
            state: "terminal",
            attempt_id: attemptId,
            episode_id: episodeId,
            exit_code: scorerResult.status,
            signal: scorerResult.signal ?? null,
            timed_out: scorerResult.error?.code === "ETIMEDOUT",
            elapsed_ms: judgeElapsedMs,
            stdout_sha256: sha256(String(scorerResult.stdout ?? "")),
            stderr_sha256: sha256(String(scorerResult.stderr ?? "")),
            terminal_at: judgeFinishedAt.toISOString(),
        });
        onPhase("scorer_terminal", { allocation_id: allocation.allocation_id, attempt_id: attemptId, scorer_terminal_at: judgeFinishedAt.toISOString() });

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
        onPhase("scorer_terminal", { allocation_id: allocation.allocation_id, attempt_id: attemptId, scorer_terminal_at: new Date().toISOString(), skipped_reason: scorerOutput.reason });
    }
    writeJsonAtomically(attemptScorerOutputPath, scorerOutput);
    writeJson(episodeScorerOutputPath, scorerOutput);

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
    const durableOutcome = {
        schema_version: 1,
        recorded_at: new Date().toISOString(),
        allocation_id: allocation.allocation_id,
        attempt_id: attemptId,
        score: scorerScore,
        judge_status: judgeStatus,
        result_metrics: { path: path.join(episodeDir, "result_metrics.json"), ...fileDigest(path.join(episodeDir, "result_metrics.json")) },
        scorer_output: { path: attemptScorerOutputPath, ...fileDigest(attemptScorerOutputPath) },
    };
    const durableOutcomePath = path.join(attemptDir, "durable_session_outcome.json");
    writeJsonAtomically(durableOutcomePath, durableOutcome);
    appendAttemptLedger(outRoot, { ...ledgerBase, event: "session_outcome", timestamp: durableOutcome.recorded_at, score: scorerScore, judge_status: judgeStatus, session_outcome_path: durableOutcomePath, result_metrics: durableOutcome.result_metrics });
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
            no_output_timeout_ms: noOutputTimeoutMs,
            termination_grace_ms: PROCESS_TERMINATION_GRACE_MS,
            force_settle_grace_ms: PROCESS_FORCE_SETTLE_GRACE_MS,
            command: { command: plan.command, args: plan.args, cwd: plan.cwd },
            codemap_binary: prepared.codemapIdentity,
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
            timeout_kind: processResult.timeoutKind ?? null,
            spawn_error: processResult.spawnError,
            runner_error: processResult.runnerError ?? null,
            termination: processResult.termination ?? null,
            stdout: fileDigest(path.join(episodeDir, "stdout.txt")),
            stderr: fileDigest(path.join(episodeDir, "stderr.txt")),
            raw_stream_lifecycle: processResult.raw_stream_lifecycle ?? null,
            opencode_protocol_v2: opencodeProtocolRecord ? {
                path: path.join(episodeDir, "opencode_protocol_v2.json"),
                ...fileDigest(path.join(episodeDir, "opencode_protocol_v2.json")),
                protocol_version: OPENCODE_PROTOCOL_VERSION,
                framing_version: OPENCODE_FRAMING_VERSION,
                session_id: opencodeProtocolRecord.stdout.session_id,
                final_provenance: opencodeProtocolRecord.final.provenance,
                final_status: opencodeProtocolRecord.final.status,
            } : null,
        },
        token_accounting: armDef.runtime.startsWith("opencode-")
            ? { source: "stdout.txt (closed raw protocol-v2 capture)", contract: "opencode JSON step_finish tokens are per-step values; sum each field", step_tokens: tokenSteps, aggregate: tokens }
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
    appendAllocationLedger(outRoot, {
        event: terminalEvent,
        timestamp: new Date().toISOString(),
        allocation_id: allocation.allocation_id,
        logical_episode_id: episodeId,
        replacement: allocation.replacement,
        attempt_id: attemptId,
        extraction_status: extractionStatus,
        harness_valid: harnessValid,
    });
    allocationTerminal = true;
    onPhase("final_recorded", { allocation_id: allocation.allocation_id, attempt_id: attemptId, final_record_at: new Date().toISOString(), terminal_event: terminalEvent });
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
        episodeFailure = error;
        throw error;
    } finally {
        const error = episodeFailure;
        if (error) {
        const failureKind = processFailureKind(processResultForFailure, error) ?? "runner_exception";
        const failureMessage = (processFailureMessage(processResultForFailure, error) ?? String(error)).slice(0, 400);
        const missingEvidenceReasons = {
            session_id: `${failureKind}:session_id_not_observed`,
            tokens: `${failureKind}:tokens_not_observed`,
        };
        const durableSessionSidecarPath = attemptDirForFailure ? path.join(attemptDirForFailure, "durable_session_sidecar.json") : null;
        let preservedSidecar = null;
        if (durableSessionSidecarPath && existsSync(durableSessionSidecarPath)) {
            try { preservedSidecar = readJson(durableSessionSidecarPath); } catch {}
        }
        const observedSessionId = partialSessionId ?? preservedSidecar?.session_id ?? null;
        const tokensObserved = preservedSidecar?.tokens != null;
        if (attemptDirForFailure && terminalLedgerBase) {
            try {
                const failureEvidencePath = path.join(attemptDirForFailure, "durable_session_failure.json");
                const failureEvidence = {
                    recorded_at: new Date().toISOString(),
                    allocation_id: allocation.allocation_id,
                    attempt_id: terminalAttemptId,
                    process: processResultForFailure ? {
                        exit_code: processResultForFailure.exitCode,
                        signal: processResultForFailure.signal ?? null,
                        timed_out: processResultForFailure.timedOut,
                        timeout_kind: processResultForFailure.timeoutKind ?? null,
                        spawn_error: processResultForFailure.spawnError ?? null,
                        runner_error: processResultForFailure.runnerError ?? null,
                        wall_ms: processResultForFailure.elapsedMs ?? null,
                        raw_stream_lifecycle: processResultForFailure.raw_stream_lifecycle ?? null,
                        termination: processResultForFailure.termination ?? null,
                    } : null,
                    partial_session_identifiers: { session_id: observedSessionId },
                    missing_evidence: {
                        session_id: observedSessionId ? null : missingEvidenceReasons.session_id,
                        tokens: tokensObserved ? null : missingEvidenceReasons.tokens,
                    },
                    failure_kind: failureKind,
                    error: failureMessage,
                };
                writeJsonAtomically(failureEvidencePath, failureEvidence);
                appendAttemptLedger(outRoot, { ...terminalLedgerBase, event: "session_evidence_failure", timestamp: failureEvidence.recorded_at, session_failure_path: failureEvidencePath, failure_kind: failureKind, error: failureEvidence.error, process: failureEvidence.process, missing_evidence: failureEvidence.missing_evidence });
            } catch (failureEvidenceError) {
                console.error(`[runner:session_failure_evidence] ${String(failureEvidenceError).slice(0, 400)}`);
            }
        }
        if (durableSessionSidecarPath && existsSync(durableSessionSidecarPath)) {
            try {
                const existingSidecar = readJson(durableSessionSidecarPath);
                writeJsonAtomically(durableSessionSidecarPath, {
                    ...existingSidecar,
                    failure_kind: existingSidecar.failure_kind ?? failureKind,
                    error: existingSidecar.error ?? failureMessage,
                    missing_evidence: {
                        session_id: existingSidecar.session_id ? null : existingSidecar.missing_evidence?.session_id ?? missingEvidenceReasons.session_id,
                        tokens: existingSidecar.tokens ? null : existingSidecar.missing_evidence?.tokens ?? missingEvidenceReasons.tokens,
                    },
                });
                durableSessionRecorded = true;
            } catch (sidecarAugmentError) {
                console.error(`[runner:session_sidecar_augment_failure] ${String(sidecarAugmentError).slice(0, 400)}`);
            }
        }
        if (attemptDirForFailure && terminalLedgerBase && !durableSessionRecorded && !existsSync(path.join(attemptDirForFailure, "durable_session_sidecar.json"))) {
            try {
                writeDurableSessionSidecar({
                    outRoot,
                    attemptDir: attemptDirForFailure,
                    ledgerBase: terminalLedgerBase,
                    ep,
                    armDef,
                    prepared,
                    processResult: processResultForFailure,
                    solveStartedAt: solveStartedAtForFailure,
                    solveFinishedAt: solveFinishedAtForFailure,
                    tokens: null,
                    toolEvents: [],
                    parserStatus: null,
                    resultPaths: { stdout: path.join(attemptDirForFailure, "stdout.txt"), stderr: path.join(attemptDirForFailure, "stderr.txt"), episode_dir: episodeDir },
                    error: failureMessage,
                    missingEvidenceReasons,
                });
                durableSessionRecorded = true;
            } catch (sidecarError) {
                console.error(`[runner:session_sidecar_failure] ${String(sidecarError).slice(0, 400)}`);
            }
        }
        if (durableAttemptPathForFailure && !existsSync(path.join(path.dirname(durableAttemptPathForFailure), "durable_terminal.json"))) {
            try {
                writeJsonAtomically(path.join(path.dirname(durableAttemptPathForFailure), "durable_terminal.json"), {
                    state: "terminal_failure",
                    attempt_id: terminalAttemptId,
                    allocation_id: allocation.allocation_id,
                    episode_id: episodeId,
                    failure_kind: failureKind,
                    failure_message: failureMessage,
                    process: processResultForFailure ? {
                        exit_code: processResultForFailure.exitCode,
                        signal: processResultForFailure.signal ?? null,
                        timed_out: processResultForFailure.timedOut,
                        timeout_kind: processResultForFailure.timeoutKind ?? null,
                        spawn_error: processResultForFailure.spawnError ?? null,
                        runner_error: processResultForFailure.runnerError ?? null,
                        termination: processResultForFailure.termination ?? null,
                    } : null,
                    missing_evidence: {
                        session_id: observedSessionId ? null : missingEvidenceReasons.session_id,
                        tokens: tokensObserved ? null : missingEvidenceReasons.tokens,
                    },
                    terminal_at: new Date().toISOString(),
                });
            } catch (terminalError) {
                console.error(`[runner:durable_terminal_failure] ${String(terminalError).slice(0, 400)}`);
            }
        }
        if (terminalLedgerBase && !terminalAppended) {
            try {
                appendAttemptLedger(outRoot, { ...terminalLedgerBase, event: "terminal_failure", timestamp: new Date().toISOString(), failure_kind: failureKind, failure_message: failureMessage, attempt_id: terminalAttemptId ?? terminalLedgerBase.attempt_id, process: processResultForFailure ? { exit_code: processResultForFailure.exitCode, signal: processResultForFailure.signal ?? null, timed_out: processResultForFailure.timedOut, timeout_kind: processResultForFailure.timeoutKind ?? null, spawn_error: processResultForFailure.spawnError ?? null, termination: processResultForFailure.termination ?? null } : null, missing_evidence: { session_id: observedSessionId ? null : missingEvidenceReasons.session_id, tokens: tokensObserved ? null : missingEvidenceReasons.tokens } });
                terminalAppended = true;
            } catch (ledgerError) {
                console.error(`[runner:ledger_failure] ${String(ledgerError).slice(0, 400)}`);
            }
        }
        if (allocationStarted && !allocationTerminal) {
            try {
                appendAllocationLedger(outRoot, {
                    event: "terminal_failure",
                    timestamp: new Date().toISOString(),
                    allocation_id: allocation.allocation_id,
                    logical_episode_id: episodeId,
                    replacement: allocation.replacement,
                    attempt_id: terminalAttemptId,
                    failure_kind: failureKind,
                    failure_message: failureMessage,
                    missing_evidence: {
                        session_id: observedSessionId ? null : missingEvidenceReasons.session_id,
                        tokens: tokensObserved ? null : missingEvidenceReasons.tokens,
                    },
                });
                allocationTerminal = true;
            } catch (ledgerError) {
                console.error(`[runner:allocation_ledger_failure] ${String(ledgerError).slice(0, 400)}`);
            }
        }
        try {
            onPhase("final_recorded", { allocation_id: allocation.allocation_id, attempt_id: terminalAttemptId, final_record_at: new Date().toISOString(), terminal_event: "terminal_failure", failure_kind: failureKind });
        } catch (phaseError) {
            console.error(`[runner:failure_phase_record] ${String(phaseError).slice(0, 400)}`);
        }
        }
        try {
            const afterSnapshot = snapshotBefore ? snapshotTargetRoot(targetRoot) : null;
            const afterRecord = afterSnapshot ?? {
                status: "unknown_due_to_runner_failure",
                reason: "before_snapshot_unavailable",
                recorded_at: new Date().toISOString(),
            };
            writeJsonAtomically(path.join(episodeDir, "mutation_guard_after.json"), afterRecord);
            if (attemptDirForFailure) {
                const durableAfterPath = path.join(attemptDirForFailure, "durable_after_snapshot.json");
                writeJsonAtomically(durableAfterPath, {
                    recorded_at: new Date().toISOString(),
                    allocation_id: allocation.allocation_id,
                    attempt_id: terminalAttemptId,
                    snapshot_status: afterSnapshot ? "captured" : "unknown_due_to_runner_failure",
                    snapshot: afterRecord,
                });
                if (terminalLedgerBase) appendAttemptLedger(outRoot, { ...terminalLedgerBase, event: "after_snapshot_finalized", timestamp: new Date().toISOString(), after_snapshot_path: durableAfterPath, snapshot_status: afterSnapshot ? "captured" : "unknown_due_to_runner_failure" });
            }
        } catch (afterSnapshotError) {
            console.error(`[runner:after_snapshot_failure] ${String(afterSnapshotError).slice(0, 400)}`);
        }
        releaseEpisodeClaim(claimPath);
        if (allocationClaim) releaseEpisodeClaim(allocationClaim);
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
    DEFAULT_GLOBAL_CAP: 1,
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

function buildExecutionWindows(episodes, requestedConcurrency) {
    const windows = [];
    for (let start = 0; start < episodes.length; start += requestedConcurrency) {
        windows.push({ start, episodes: episodes.slice(start, start + requestedConcurrency) });
    }

    const windowByWave = new Map();
    for (const [index, episode] of episodes.entries()) {
        if (episode.wave_id == null) continue;
        const waveKey = String(episode.wave_id);
        const windowIndex = Math.floor(index / requestedConcurrency);
        const priorWindowIndex = windowByWave.get(waveKey);
        if (priorWindowIndex != null && priorWindowIndex !== windowIndex) {
            throw new Error(`[runner:schedule] wave ${waveKey} crosses concurrency-${requestedConcurrency} execution windows`);
        }
        windowByWave.set(waveKey, windowIndex);
    }
    return windows;
}

async function runBatch(episodes, config, _legacyNonClaudeCap) {
    const armConfigArms = config.armConfig.arms;
    const requestedConcurrency = config.requestedConcurrency ?? CONCURRENCY.DEFAULT_GLOBAL_CAP;

    // 전역 슬롯 + 백엔드별 세마포어
    const globalSem = new Semaphore(requestedConcurrency);
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
    let preparedEpisodes = null;
    if (runImpl === runEpisode) {
        try {
            // Solver 시작 전에 전체 입력의 결과 비의존 사전 점검을 끝낸다. 따라서 한 wave의
            // 후속 allocation이 MCP preflight를 기다리는 동안 먼저 시작한 solver를 오염시키지 않는다.
            preparedEpisodes = await Promise.all(episodes.map((episode) => prepareEpisodeForResume(episode, config)));
        } catch (error) {
            batchAbortController.abort(error);
            return episodes.map((episode) => abortedResult(episode, armConfigArms.find((arm) => arm.arm_id === (episode.arm_id ?? episode.arm)), error));
        }
    }

    // co-tenancy 레지스트리: 현재 in-flight인 episode의 백엔드 구성을 추적.
    // 각 episode 실행 시작 시점 스냅샷을 result_metrics에 기록(wall_time 부풀림 보정용).
    const activeBackends = new Map(); // episodeId → backend
    function coTenancySnapshot() {
        const backends = {};
        for (const b of activeBackends.values()) backends[b] = (backends[b] || 0) + 1;
        return { count: activeBackends.size, backends };
    }
    config.batchConcurrencyObserved ??= { max_in_flight: 0 };

    const tasks = episodes.map((ep, sequenceOrdinal) => {
        const armDef = armConfigArms.find((a) => a.arm_id === (ep.arm_id ?? ep.arm));
        const backend = backendOf(armDef, ep);
        const isSerena = backend === "serena";
        const isCodegraph = backend === "codegraph";
        const episodeId = `${ep.arm_id}__${ep.codebase}__round-${ep.round}`;
        const codebaseLock = isSerena ? serenaCodebaseLockFor(ep.codebase) : null;
        const backendSem = isSerena ? serenaSem : isCodegraph ? codegraphSem : null;

        return async (waveParticipant = null) => {
            const prepared = preparedEpisodes?.[sequenceOrdinal] ?? null;
            try { throwIfBatchAborted(); } catch (error) { return abortedResult(ep, armDef, error); }
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
            // release는 final record·child cleanup·runtime attestation까지 끝난 뒤에만 역순으로 수행한다.
            let acquiredCodebase = false;
            let acquiredBackend = false;
            let acquiredGlobal = false;
            let released = false;
            let allocation = null;
            function releaseSlots() {
                if (released) return;
                released = true;
                if (acquiredGlobal) globalSem.release();
                if (acquiredBackend && backendSem) backendSem.release();
                if (acquiredCodebase && codebaseLock) codebaseLock.release();
                activeBackends.delete(episodeId);
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
            try {
                allocation = allocationEpisodeFromInput(ep);
                writeGlobalSlotRecord(config.outRoot, {
                    allocation_id: allocation.allocation_id,
                    episode_id: episodeId,
                    phase: "reserved",
                    acquired_at: new Date().toISOString(),
                });
            } catch (error) {
                releaseSlots();
                throw error;
            }

            // co-tenancy: 실행 중으로 등록(스냅샷에 잡히도록).
            activeBackends.set(episodeId, backend);
            config.batchConcurrencyObserved.max_in_flight = Math.max(config.batchConcurrencyObserved.max_in_flight, activeBackends.size);

            let result;
            try {
                result = await runImpl({ ...ep, sequence_ordinal: ep.sequence_ordinal ?? sequenceOrdinal + 1 }, config, {
                    releaseSlots,
                    coTenancySnapshot,
                    awaitSolverStartBarrier: () => waveParticipant?.awaitSolverStart(),
                    awaitScorerStartBarrier: () => waveParticipant?.awaitScorerStart(),
                    prepared,
                    abortSignal: batchAbortController.signal,
                    onPhase: (phase, detail = {}) => writeGlobalSlotRecord(config.outRoot, {
                        allocation_id: allocation.allocation_id,
                        episode_id: episodeId,
                        phase,
                        ...detail,
                    }),
                });
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
                writeGlobalSlotRecord(config.outRoot, {
                    allocation_id: allocation.allocation_id,
                    episode_id: episodeId,
                    phase: "released",
                    cleanup_completed_at: new Date().toISOString(),
                    final_record_at: new Date().toISOString(),
                    result_error: result?.error ?? null,
                });
                releaseSlots();
            }
            if (result?.error) batchAbortController.abort(new Error(result.error));
            return result;
        };
    });

    const results = new Array(tasks.length);
    const executeTask = async (index, waveParticipant = null) => {
        try {
            results[index] = await tasks[index](waveParticipant);
        } catch (error) {
            results[index] = abortedResult(episodes[index], armConfigArms.find((arm) => arm.arm_id === (episodes[index].arm_id ?? episodes[index].arm)), error);
        } finally {
            waveParticipant?.withdrawPending();
        }
    };
    const executionWindows = buildExecutionWindows(episodes, requestedConcurrency);
    for (const [windowIndex, executionWindow] of executionWindows.entries()) {
            const windowEpisodes = executionWindow.episodes;
            const serenaEpisodes = windowEpisodes.filter((episode) => backendOf(armConfigArms.find((arm) => arm.arm_id === (episode.arm_id ?? episode.arm)), episode) === "serena");
            const codegraphCount = windowEpisodes.filter((episode) => backendOf(armConfigArms.find((arm) => arm.arm_id === (episode.arm_id ?? episode.arm)), episode) === "codegraph").length;
            if (serenaEpisodes.length > CONCURRENCY.SERENA_GLOBAL || codegraphCount > CONCURRENCY.CODEGRAPH_GLOBAL || new Set(serenaEpisodes.map((episode) => episode.codebase)).size !== serenaEpisodes.length) {
                throw new Error(`[runner:schedule] execution window ${windowIndex + 1} cannot reach the solver barrier under backend concurrency limits`);
            }
            const solverStartBarrier = new PhaseBarrier(windowEpisodes.length);
            const scorerStartBarrier = new PhaseBarrier(windowEpisodes.length);
            const participants = windowEpisodes.map(() => createWaveParticipant(solverStartBarrier, scorerStartBarrier));
            await Promise.all(participants.map((participant, offset) => executeTask(executionWindow.start + offset, participant)));
    }
    return results;
}

function validateEpisodeSchedule(episodes) {
    let previousSequence = null;
    let activeWave = null;
    const closedWaves = new Set();
    for (const [index, episode] of episodes.entries()) {
        if (episode.sequence_ordinal != null) {
            if (!Number.isInteger(episode.sequence_ordinal) || episode.sequence_ordinal < 1) {
                throw new Error(`[runner:schedule] sequence_ordinal must be a positive integer at input index ${index}`);
            }
            if (previousSequence != null && episode.sequence_ordinal <= previousSequence) {
                throw new Error(`[runner:schedule] sequence_ordinal must be strictly increasing in external episode order at input index ${index}`);
            }
            previousSequence = episode.sequence_ordinal;
        }
        if (episode.wave_id == null) continue;
        if (typeof episode.wave_id !== "string" && typeof episode.wave_id !== "number") {
            throw new Error(`[runner:schedule] wave_id must be a string or number at input index ${index}`);
        }
        const waveKey = String(episode.wave_id);
        if (activeWave !== waveKey) {
            if (closedWaves.has(waveKey)) throw new Error(`[runner:schedule] wave_id must be contiguous in external episode order: ${waveKey}`);
            if (activeWave != null) closedWaves.add(activeWave);
            activeWave = waveKey;
        }
    }
}

function readJsonLinesForRecovery(filePath) {
    if (!existsSync(filePath)) return [];
    return readFileSync(filePath, "utf8").split("\n").flatMap((line, index) => {
        if (!line.trim()) return [];
        try {
            const value = JSON.parse(line);
            if (!value || typeof value !== "object" || Array.isArray(value)) return [{ recovery_error: `invalid_jsonl_object:${path.basename(filePath)}:${index + 1}` }];
            return [value];
        } catch {
            return [{ recovery_error: `invalid_jsonl:${path.basename(filePath)}:${index + 1}` }];
        }
    });
}

function recoveryLedgerBase(attemptMetadata) {
    const keys = [
        "identity_schema_version", "attempt_id", "attempt_dir", "allocation_id", "replacement",
        "episode_id", "arm_id", "codebase", "round", "sequence_ordinal", "provider", "model",
        "command_sha256", "runtime", "repository", "solver_identity_sha256", "solver_contract_hash",
        "runner_operational_hash", "final_identity_sha256",
    ];
    return Object.fromEntries(keys.map((key) => [key, attemptMetadata[key]]));
}

function assertRecoveryDigest(label, expected) {
    if (!expected?.path || !expected.exists) throw new Error(`[runner:session_recovery] ${label} sealed path is unavailable`);
    const observed = fileDigest(expected.path);
    if (!observed.exists || observed.sha256 !== expected.sha256 || observed.bytes !== expected.bytes) {
        throw new Error(`[runner:session_recovery] ${label} digest changed: ${expected.path}`);
    }
    return observed;
}

function prepareOpenCodeSessionRecovery(outRoot, allowlistEntry) {
    for (const field of ["allocation_id", "episode_id", "attempt_id", "session_id"]) {
        if (typeof allowlistEntry?.[field] !== "string" || allowlistEntry[field].length === 0) {
            throw new Error(`[runner:session_recovery] allowlist ${field} is required`);
        }
    }
    assertAllocationEpisodeId(allowlistEntry.allocation_id, "allowlist.allocation_id");
    assertAllocationEpisodeId(allowlistEntry.episode_id, "allowlist.episode_id");
    assertAllocationEpisodeId(allowlistEntry.attempt_id, "allowlist.attempt_id");
    assertAllocationEpisodeId(allowlistEntry.session_id, "allowlist.session_id");
    const attemptDir = path.join(outRoot, "attempts", allowlistEntry.attempt_id);
    requireDirectory("session recovery attempt", attemptDir);
    const paths = {
        attemptMetadata: path.join(attemptDir, "attempt_metadata.json"),
        sidecar: path.join(attemptDir, "durable_session_sidecar.json"),
        terminal: path.join(attemptDir, "durable_terminal.json"),
        failure: path.join(attemptDir, "durable_session_failure.json"),
        protocol: path.join(attemptDir, "opencode_protocol_v2.json"),
        stdout: path.join(attemptDir, "stdout.txt"),
        processResult: path.join(attemptDir, "process_result.json"),
        originalMetrics: path.join(attemptDir, "result_metrics.json"),
        originalAfter: path.join(attemptDir, "durable_after_snapshot.json"),
        recoveredProtocol: path.join(attemptDir, "opencode_protocol_v2.recovered.json"),
        recoveredRawAnswer: path.join(attemptDir, "raw_answer.recovered.txt"),
        recoveredScorerState: path.join(attemptDir, "durable_scorer.recovered.json"),
        recoveredScorerOutput: path.join(attemptDir, "durable_scorer_output.recovered.json"),
        recoveredMetrics: path.join(attemptDir, "result_metrics.recovered.json"),
        recoveredSidecar: path.join(attemptDir, "durable_session_sidecar.recovered.json"),
        recoveredOutcome: path.join(attemptDir, "durable_session_outcome.recovered.json"),
        recoveredAfter: path.join(attemptDir, "durable_after_snapshot.recovered.json"),
        recoveryRecord: path.join(attemptDir, "durable_session_recovery.json"),
    };
    for (const [label, filePath] of Object.entries(paths).filter(([label]) => label.startsWith("recovered") || label === "recoveryRecord")) {
        if (existsSync(filePath)) throw new Error(`[runner:session_recovery] ${label} already exists: ${filePath}`);
    }
    for (const [label, filePath] of Object.entries(paths).filter(([label]) => !label.startsWith("recovered") && label !== "recoveryRecord")) {
        requireFile(`session recovery ${label}`, filePath);
    }
    const attemptMetadata = readJson(paths.attemptMetadata);
    const sidecar = readJson(paths.sidecar);
    const terminal = readJson(paths.terminal);
    const protocol = readJson(paths.protocol);
    const processResult = readJson(paths.processResult);
    const originalMetrics = readJson(paths.originalMetrics);
    if (attemptMetadata.allocation_id !== allowlistEntry.allocation_id || attemptMetadata.episode_id !== allowlistEntry.episode_id || attemptMetadata.attempt_id !== allowlistEntry.attempt_id) {
        throw new Error(`[runner:session_recovery] allowlist does not match attempt metadata: ${allowlistEntry.attempt_id}`);
    }
    if (sidecar.session_id !== allowlistEntry.session_id || protocol.stdout?.session_id !== allowlistEntry.session_id) {
        throw new Error(`[runner:session_recovery] allowlist session_id does not match sealed evidence: ${allowlistEntry.attempt_id}`);
    }
    if (protocol.stdout?.status !== "valid" || protocol.final?.status !== "protocol_invalid" || protocol.session_recovery?.reason !== "session_export_invalid_json") {
        throw new Error(`[runner:session_recovery] attempt is not an export-JSON protocol failure: ${allowlistEntry.attempt_id}`);
    }
    if (terminal.state !== "terminal" || terminal.opencode_protocol_v2?.final_status !== "protocol_invalid" || sidecar.tokens?.accounting_status !== "complete") {
        throw new Error(`[runner:session_recovery] solver evidence is not complete enough for export-only recovery: ${allowlistEntry.attempt_id}`);
    }
    const allAllocationEvents = readJsonLinesForRecovery(allocationLedgerPath(outRoot));
    const allAttemptEvents = readJsonLinesForRecovery(path.join(outRoot, "attempt-ledger.jsonl"));
    if ([...allAllocationEvents, ...allAttemptEvents].some((event) => event.recovery_error)) throw new Error("[runner:session_recovery] recovery ledger is corrupt");
    const allocationHistory = allAllocationEvents.filter((event) => event.allocation_id === allowlistEntry.allocation_id);
    const attemptHistory = allAttemptEvents.filter((event) => event.attempt_id === allowlistEntry.attempt_id);
    if (!allocationHistory.some((event) => event.event === "terminal_failure" && event.attempt_id === allowlistEntry.attempt_id)) {
        throw new Error(`[runner:session_recovery] original allocation terminal_failure is missing: ${allowlistEntry.allocation_id}`);
    }
    if (!attemptHistory.some((event) => event.event === "terminal_failure") || attemptHistory.some((event) => String(event.event).startsWith("session_recovery"))) {
        throw new Error(`[runner:session_recovery] attempt is not eligible or recovery was already attempted: ${allowlistEntry.attempt_id}`);
    }
    const runtimePath = attemptMetadata.runtime?.path;
    const runtimeDigest = runtimePath ? fileDigest(runtimePath) : { exists: false };
    if (!runtimePath || !runtimeDigest.exists || runtimeDigest.sha256 !== attemptMetadata.runtime?.file?.sha256 || runtimeDigest.bytes !== attemptMetadata.runtime?.file?.bytes) {
        throw new Error(`[runner:session_recovery] sealed OpenCode executable changed: ${runtimePath ?? "missing"}`);
    }
    const selectedEnvironment = protocol.environment?.selected;
    for (const key of ["XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME"]) {
        if (typeof selectedEnvironment?.[key] !== "string" || selectedEnvironment[key].length === 0) {
            throw new Error(`[runner:session_recovery] sealed ${key} is missing`);
        }
    }
    const scoringContract = attemptMetadata.resume_identity?.value?.scoring_contract;
    assertRecoveryDigest("scorer", scoringContract?.scorer);
    assertRecoveryDigest("schema", scoringContract?.schema);
    assertRecoveryDigest("private answer key", scoringContract?.private_answer_key);
    if (typeof scoringContract.judge_model !== "string" || scoringContract.judge_model.length === 0) throw new Error("[runner:session_recovery] sealed judge model is missing");
    return {
        allowlistEntry,
        attemptDir,
        paths,
        attemptMetadata,
        ledgerBase: recoveryLedgerBase(attemptMetadata),
        sidecar,
        protocol,
        processResult,
        originalMetrics,
        scoringContract,
        plan: {
            command: runtimePath,
            args: [],
            cwd: protocol.argv?.cwd,
            env: { ...process.env, ...selectedEnvironment },
            stdin: null,
        },
    };
}

function runRecoveredScorer(context, rawAnswer) {
    const contract = context.scoringContract;
    const scorerArgs = [
        contract.scorer.path,
        "--raw-answer", context.paths.recoveredRawAnswer,
        "--schema", contract.schema.path,
        "--answer-key", contract.private_answer_key.path,
        "--out", context.paths.recoveredScorerOutput,
        "--judge-model", contract.judge_model,
    ];
    if (contract.print_command) scorerArgs.push("--print-cmd");
    const startedAt = new Date();
    writeJsonAtomically(context.paths.recoveredScorerState, {
        state: "started",
        recovery_only: true,
        attempt_id: context.allowlistEntry.attempt_id,
        episode_id: context.allowlistEntry.episode_id,
        scorer: { command: "node", argv: scorerArgs.slice(0, 1).concat(["--raw-answer", context.paths.recoveredRawAnswer, "--schema", contract.schema.path, "--out", context.paths.recoveredScorerOutput, "--judge-model", contract.judge_model]) },
        started_at: startedAt.toISOString(),
    });
    const result = spawnSync("node", scorerArgs, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024, timeout: 300_000 });
    const finishedAt = new Date();
    const state = {
        state: "terminal",
        recovery_only: true,
        attempt_id: context.allowlistEntry.attempt_id,
        episode_id: context.allowlistEntry.episode_id,
        exit_code: result.status,
        signal: result.signal ?? null,
        timed_out: result.error?.code === "ETIMEDOUT",
        elapsed_ms: finishedAt.getTime() - startedAt.getTime(),
        stdout_sha256: sha256(String(result.stdout ?? "")),
        stderr_sha256: sha256(String(result.stderr ?? "")),
        terminal_at: finishedAt.toISOString(),
    };
    writeJsonAtomically(context.paths.recoveredScorerState, state);
    if (result.status !== 0 || !existsSync(context.paths.recoveredScorerOutput)) {
        return { status: "scorer_failed", score: null, output: null, process: state };
    }
    let output;
    try { output = readJson(context.paths.recoveredScorerOutput); } catch { return { status: "result_parse_error", score: null, output: null, process: state }; }
    if (!validScorerOutput(output, readJson(contract.schema.path), context.attemptMetadata.codebase, sha256(rawAnswer))) {
        return { status: "result_contract_error", score: null, output, process: state };
    }
    return { status: "completed", score: output.score ?? null, output, process: state };
}

function writeOpenCodeRecoveryFailure(outRoot, context, status, detail, exportRecovery = null, scorerInvocations = 0) {
    const recordedAt = new Date().toISOString();
    const record = {
        schema_version: 1,
        status,
        recorded_at: recordedAt,
        allowlist: context.allowlistEntry,
        original_evidence: {
            protocol: { path: context.paths.protocol, ...fileDigest(context.paths.protocol) },
            terminal: { path: context.paths.terminal, ...fileDigest(context.paths.terminal) },
            failure: { path: context.paths.failure, ...fileDigest(context.paths.failure) },
        },
        export_recovery: exportRecovery,
        external_invocations: { solver_model: 0, opencode_run: 0, opencode_export: exportRecovery ? 1 : 0, scorer: scorerInvocations },
        source_mutation: false,
        detail,
    };
    writeJsonAtomically(context.paths.recoveryRecord, record);
    appendAttemptLedger(outRoot, { ...context.ledgerBase, event: "session_recovery_failure", timestamp: recordedAt, recovery_path: context.paths.recoveryRecord, status, external_invocations: record.external_invocations });
    appendAllocationLedger(outRoot, { event: "session_recovery_failure", timestamp: recordedAt, allocation_id: context.allowlistEntry.allocation_id, logical_episode_id: context.allowlistEntry.episode_id, attempt_id: context.allowlistEntry.attempt_id, status });
    return record;
}

async function executeOpenCodeSessionRecovery(outRoot, context) {
    const startedAt = new Date().toISOString();
    const plannedInvocations = { solver_model: 0, opencode_run: 0, opencode_export: 1, scorer: 1 };
    appendAttemptLedger(outRoot, { ...context.ledgerBase, event: "session_recovery_started", timestamp: startedAt, allowlist: context.allowlistEntry, planned_external_invocations: plannedInvocations });
    appendAllocationLedger(outRoot, { event: "session_recovery_started", timestamp: startedAt, allocation_id: context.allowlistEntry.allocation_id, logical_episode_id: context.allowlistEntry.episode_id, attempt_id: context.allowlistEntry.attempt_id, session_id: context.allowlistEntry.session_id, planned_external_invocations: plannedInvocations });
    let exportRecovery;
    try {
        exportRecovery = await recoverOpencodeSessionFinal(context.plan, context.allowlistEntry.session_id, context.attemptDir, { recovered: true });
    } catch (error) {
        exportRecovery = { status: "protocol_failure", session_id: context.allowlistEntry.session_id, export: null, final_answer: null, reason: `session_export_exception:${String(error).slice(0, 300)}` };
    }
    if (exportRecovery.status !== "recovered") {
        return writeOpenCodeRecoveryFailure(outRoot, context, "protocol_failure", exportRecovery.reason, exportRecovery);
    }
    const stdoutExtraction = parseOpencodeProtocolV2(readFileSync(context.paths.stdout));
    const final = resolveOpencodeFinal(stdoutExtraction, exportRecovery);
    if (stdoutExtraction.protocol.status !== "valid" || stdoutExtraction.protocol.session_id !== context.allowlistEntry.session_id || final.status !== "success") {
        return writeOpenCodeRecoveryFailure(outRoot, context, "protocol_failure", final.reason ?? "recovered_final_invalid", exportRecovery);
    }
    writeTextAtomically(context.paths.recoveredRawAnswer, final.answer);
    const recoveredProtocol = {
        ...context.protocol,
        session_recovery: exportRecovery,
        final: {
            status: final.status,
            provenance: final.provenance,
            reason: final.reason,
            answer_sha256: sha256(final.answer),
            answer_bytes: Buffer.byteLength(final.answer),
        },
        recovered_from: { path: context.paths.protocol, ...fileDigest(context.paths.protocol) },
        recovery_only: true,
        recorded_at: new Date().toISOString(),
    };
    writeJsonAtomically(context.paths.recoveredProtocol, recoveredProtocol);
    let scoring;
    try {
        scoring = runRecoveredScorer(context, final.answer);
    } catch (error) {
        return writeOpenCodeRecoveryFailure(outRoot, context, "scorer_failure", `scorer_exception:${String(error).slice(0, 300)}`, exportRecovery, 0);
    }
    if (scoring.status !== "completed") {
        return writeOpenCodeRecoveryFailure(outRoot, context, "scorer_failure", scoring.status, exportRecovery, 1);
    }
    const harnessValid = context.processResult.exitCode === 0 && !context.processResult.timedOut && context.originalMetrics.mutation_guard_status === "clean";
    const recoveryIdentity = {
        recovery_only: true,
        original_attempt_id: context.allowlistEntry.attempt_id,
        original_terminal_preserved: true,
        session_id: context.allowlistEntry.session_id,
        export_capture: "direct_file_descriptor",
        solver_model_invocations: 0,
        opencode_run_invocations: 0,
    };
    const recoveredMetrics = {
        ...context.originalMetrics,
        judge_status: scoring.status,
        extraction_status: "success",
        answer_sha256: sha256(final.answer),
        scorer_score: scoring.score,
        harness_valid: harnessValid,
        recovery: recoveryIdentity,
    };
    writeJsonAtomically(context.paths.recoveredMetrics, recoveredMetrics);
    const recoveredSidecar = {
        ...context.sidecar,
        schema_version: 1,
        recorded_at: new Date().toISOString(),
        score: scoring.score,
        error: null,
        failure_kind: null,
        result_paths: {
            ...context.sidecar.result_paths,
            session_export: exportRecovery.export.capture.final_path,
            raw_answer: context.paths.recoveredRawAnswer,
            protocol: context.paths.recoveredProtocol,
            scorer_output: context.paths.recoveredScorerOutput,
            result_metrics: context.paths.recoveredMetrics,
        },
        recovery: recoveryIdentity,
    };
    writeJsonAtomically(context.paths.recoveredSidecar, recoveredSidecar);
    const recoveredOutcome = {
        schema_version: 1,
        recorded_at: new Date().toISOString(),
        allocation_id: context.allowlistEntry.allocation_id,
        attempt_id: context.allowlistEntry.attempt_id,
        score: scoring.score,
        judge_status: scoring.status,
        result_metrics: { path: context.paths.recoveredMetrics, ...fileDigest(context.paths.recoveredMetrics) },
        scorer_output: { path: context.paths.recoveredScorerOutput, ...fileDigest(context.paths.recoveredScorerOutput) },
        recovery: recoveryIdentity,
    };
    writeJsonAtomically(context.paths.recoveredOutcome, recoveredOutcome);
    const recoveredAfter = {
        recorded_at: new Date().toISOString(),
        recovery_only: true,
        source_mutation: false,
        original_after_snapshot: { path: context.paths.originalAfter, ...fileDigest(context.paths.originalAfter) },
        snapshot_status: "preserved_from_original_attempt",
    };
    writeJsonAtomically(context.paths.recoveredAfter, recoveredAfter);
    const completedAt = new Date().toISOString();
    const record = {
        schema_version: 1,
        status: harnessValid ? "recovered_completed" : "recovered_harness_invalid",
        recorded_at: completedAt,
        allowlist: context.allowlistEntry,
        original_terminal_preserved: { path: context.paths.terminal, ...fileDigest(context.paths.terminal) },
        export_recovery: exportRecovery,
        final: recoveredProtocol.final,
        score: scoring.score,
        judge_status: scoring.status,
        external_invocations: { solver_model: 0, opencode_run: 0, opencode_export: 1, scorer: 1 },
        source_mutation: false,
        recovered_artifacts: {
            sidecar: { path: context.paths.recoveredSidecar, ...fileDigest(context.paths.recoveredSidecar) },
            outcome: { path: context.paths.recoveredOutcome, ...fileDigest(context.paths.recoveredOutcome) },
            result_metrics: { path: context.paths.recoveredMetrics, ...fileDigest(context.paths.recoveredMetrics) },
            after: { path: context.paths.recoveredAfter, ...fileDigest(context.paths.recoveredAfter) },
            protocol: { path: context.paths.recoveredProtocol, ...fileDigest(context.paths.recoveredProtocol) },
        },
    };
    writeJsonAtomically(context.paths.recoveryRecord, record);
    appendAttemptLedger(outRoot, {
        ...context.ledgerBase,
        event: "session_recovered",
        timestamp: completedAt,
        recovery_path: context.paths.recoveryRecord,
        recovered_sidecar_path: context.paths.recoveredSidecar,
        recovered_outcome_path: context.paths.recoveredOutcome,
        recovered_result_metrics_path: context.paths.recoveredMetrics,
        recovered_after_path: context.paths.recoveredAfter,
        status: record.status,
        external_invocations: record.external_invocations,
    });
    appendAllocationLedger(outRoot, { event: "session_recovered", timestamp: completedAt, allocation_id: context.allowlistEntry.allocation_id, logical_episode_id: context.allowlistEntry.episode_id, attempt_id: context.allowlistEntry.attempt_id, session_id: context.allowlistEntry.session_id, status: record.status, external_invocations: record.external_invocations });
    return record;
}

async function recoverOpenCodeSessionsFromAllowlist(outRoot, allowlist) {
    const contexts = allowlist.map((entry) => prepareOpenCodeSessionRecovery(outRoot, entry));
    const results = [];
    for (const context of contexts) results.push(await executeOpenCodeSessionRecovery(outRoot, context));
    return {
        schema_version: 1,
        mode: "opencode_export_recovery_only",
        recorded_at: new Date().toISOString(),
        allowlist_count: allowlist.length,
        external_invocations: {
            solver_model: 0,
            opencode_run: 0,
            opencode_export: results.filter((result) => result.export_recovery).length,
            scorer: results.filter((result) => result.external_invocations?.scorer === 1).length,
        },
        results,
    };
}

function recoverAggregateFromLedgers(outRoot, episodes) {
    const allocationEvents = readJsonLinesForRecovery(allocationLedgerPath(outRoot));
    const attemptEvents = readJsonLinesForRecovery(path.join(outRoot, "attempt-ledger.jsonl"));
    const errors = [...allocationEvents, ...attemptEvents]
        .filter((event) => event.recovery_error)
        .map((event) => ({ kind: "ledger_corruption", detail: event.recovery_error }));
    for (const [ledger, events] of [["allocation", allocationEvents], ["attempt", attemptEvents]]) {
        for (const event of events) {
            if (!event.recovery_error && typeof event.allocation_id !== "string") errors.push({ kind: "ledger_event_missing_allocation_id", ledger, event: event.event ?? null });
        }
    }
    const missing = [];
    const allocations = episodes.map((episode, inputIndex) => {
        const allocation = allocationEpisodeFromInput(episode);
        const allocationHistory = allocationEvents.filter((event) => event.allocation_id === allocation.allocation_id);
        const allocationAttemptEvents = attemptEvents.filter((event) => event.allocation_id === allocation.allocation_id);
        const attempts = [...new Set([
            ...allocationHistory.map((event) => event.attempt_id),
            ...allocationAttemptEvents.map((event) => event.attempt_id),
        ].filter((attemptId) => typeof attemptId === "string" && attemptId.length > 0))];
        const evidence = attempts.map((attemptId) => {
            const attemptDir = path.join(outRoot, "attempts", attemptId);
            const ledgerEvents = allocationAttemptEvents.filter((event) => event.attempt_id === attemptId);
            const recoveryEvent = ledgerEvents.filter((event) => event.event === "session_recovered").at(-1) ?? null;
            const sidecarPath = recoveryEvent?.recovered_sidecar_path ?? path.join(attemptDir, "durable_session_sidecar.json");
            const terminalPath = path.join(attemptDir, "durable_terminal.json");
            const outcomePath = recoveryEvent?.recovered_outcome_path ?? path.join(attemptDir, "durable_session_outcome.json");
            const afterPath = recoveryEvent?.recovered_after_path ?? path.join(attemptDir, "durable_after_snapshot.json");
            const episodeDir = path.join(outRoot, episode.arm_id ?? episode.arm, episode.codebase, `round-${episode.round}`);
            const resultMetricsPath = recoveryEvent?.recovered_result_metrics_path ?? path.join(episodeDir, "result_metrics.json");
            const recoveryPath = recoveryEvent?.recovery_path ?? null;
            const readOptional = (filePath) => {
                try { return existsSync(filePath) ? readJson(filePath) : null; } catch { return { recovery_error: `invalid_json:${filePath}` }; }
            };
            return {
                attempt_id: attemptId,
                sidecar_path: sidecarPath,
                sidecar: readOptional(sidecarPath),
                terminal_path: terminalPath,
                terminal: readOptional(terminalPath),
                outcome_path: outcomePath,
                outcome: readOptional(outcomePath),
                after_path: afterPath,
                after: readOptional(afterPath),
                result_metrics_path: resultMetricsPath,
                result_metrics: readOptional(resultMetricsPath),
                recovery_path: recoveryPath,
                recovery: recoveryPath ? readOptional(recoveryPath) : null,
                recovery_event: recoveryEvent,
                ledger_events: ledgerEvents,
            };
        });
        const terminalEvents = allocationHistory.filter((event) => event.event === "completed" || event.event === "terminal_failure");
        const terminalAttemptIds = [...new Set(terminalEvents.map((event) => event.attempt_id).filter(Boolean))];
        const terminal = terminalEvents.at(-1) ?? null;
        const selectedAttemptId = terminalAttemptIds.length === 1
            ? terminalAttemptIds[0]
            : [...evidence].reverse().find((item) => item.outcome || item.sidecar || item.terminal)?.attempt_id
                ?? null;
        const selectedEvidence = evidence.find((item) => item.attempt_id === selectedAttemptId) ?? null;
        const duplicateTerminalAttempts = terminalAttemptIds.length > 1;
        const selectedArtifacts = selectedEvidence ? [
            ["sidecar", selectedEvidence.sidecar, selectedEvidence.sidecar_path],
            ["terminal", selectedEvidence.terminal, selectedEvidence.terminal_path],
            ["outcome", selectedEvidence.outcome, selectedEvidence.outcome_path],
            ["result_metrics", selectedEvidence.result_metrics, selectedEvidence.result_metrics_path],
            ["after", selectedEvidence.after, selectedEvidence.after_path],
        ] : [];
        for (const [artifact, value, artifactPath] of selectedArtifacts) {
            if (value == null) missing.push({ allocation_id: allocation.allocation_id, attempt_id: selectedAttemptId, artifact, path: artifactPath });
            else if (value.recovery_error) errors.push({ kind: "artifact_corruption", allocation_id: allocation.allocation_id, attempt_id: selectedAttemptId, artifact, detail: value.recovery_error });
        }
        if (!selectedAttemptId) missing.push({ allocation_id: allocation.allocation_id, artifact: "selected_attempt", path: null });
        if (duplicateTerminalAttempts) errors.push({ kind: "duplicate_terminal_attempts", allocation_id: allocation.allocation_id, attempt_ids: terminalAttemptIds });
        const tokenAccountingIncomplete = selectedEvidence?.sidecar?.tokens?.accounting_status === "incomplete";
        if (tokenAccountingIncomplete) {
            errors.push({ kind: "incomplete_token_accounting", allocation_id: allocation.allocation_id, attempt_id: selectedAttemptId, incomplete_fields: selectedEvidence.sidecar.tokens.incomplete_fields ?? [] });
        }
        const recoveredStatus = selectedEvidence?.recovery_event?.status ?? null;
        const status = duplicateTerminalAttempts
            ? "analysis_blocked_duplicate_terminal"
            : recoveredStatus
                ? recoveredStatus
            : terminal?.event === "completed"
            ? "completed"
            : terminal?.event === "terminal_failure" || evidence.some((item) => item.terminal?.state === "terminal_failure")
                ? "terminal_failure"
                : allocationHistory.some((event) => event.event === "started") || evidence.length > 0
                    ? "unknown_due_to_runner_failure"
                    : "not_started";
        return {
            allocation_id: allocation.allocation_id,
            input_index: inputIndex,
            pair_id: episode.pair_id ?? null,
            arm_id: episode.arm_id ?? episode.arm ?? null,
            order: episode.order ?? null,
            sequence_ordinal: episode.sequence_ordinal ?? inputIndex + 1,
            wave_id: episode.wave_id ?? null,
            status,
            selected_attempt_id: selectedAttemptId,
            analysis_eligible: !duplicateTerminalAttempts && !tokenAccountingIncomplete && recoveredStatus !== "recovered_harness_invalid",
            score: selectedEvidence?.outcome?.score ?? selectedEvidence?.result_metrics?.scorer_score ?? null,
            judge_status: selectedEvidence?.outcome?.judge_status ?? selectedEvidence?.result_metrics?.judge_status ?? null,
            final_metrics: selectedEvidence?.result_metrics ?? null,
            selected_evidence: selectedEvidence,
            attempt_history: attempts.map((attemptId) => ({ attempt_id: attemptId })),
            allocation_ledger_events: allocationHistory,
        };
    });
    const duplicateSessionIds = [];
    const firstAllocationBySessionKey = new Map();
    for (const allocation of allocations) {
        const sessionId = allocation.selected_evidence?.sidecar?.session_id ?? null;
        const sessionKey = sessionId ? `session:${sessionId}` : allocation.selected_attempt_id ? `attempt:${allocation.selected_attempt_id}` : null;
        allocation.session_id = sessionId;
        allocation.session_key = sessionKey;
        allocation.included_in_recovery_totals = Boolean(sessionKey && allocation.analysis_eligible && allocation.selected_evidence?.sidecar && !allocation.selected_evidence.sidecar.recovery_error);
        if (!sessionKey) {
            allocation.included_in_recovery_totals = false;
            continue;
        }
        const firstAllocationId = firstAllocationBySessionKey.get(sessionKey);
        if (firstAllocationId) {
            allocation.included_in_recovery_totals = false;
            allocation.analysis_eligible = false;
            allocation.duplicate_of_allocation_id = firstAllocationId;
            duplicateSessionIds.push({ session_key: sessionKey, first_allocation_id: firstAllocationId, duplicate_allocation_id: allocation.allocation_id });
        } else {
            firstAllocationBySessionKey.set(sessionKey, allocation.allocation_id);
        }
    }
    if (duplicateSessionIds.length > 0) errors.push(...duplicateSessionIds.map((duplicate) => ({ kind: "duplicate_session_identifier", ...duplicate })));

    const includedAllocations = allocations.filter((allocation) => allocation.included_in_recovery_totals);
    const aggregateNumericField = (label, valueFor) => {
        const observed = [];
        const missingAllocationIds = [];
        for (const allocation of includedAllocations) {
            const value = valueFor(allocation);
            if (typeof value === "number" && Number.isFinite(value)) observed.push(value);
            else missingAllocationIds.push(allocation.allocation_id);
        }
        return {
            field: label,
            sum: observed.length > 0 ? observed.reduce((total, value) => total + value, 0) : null,
            mean: observed.length > 0 ? observed.reduce((total, value) => total + value, 0) / observed.length : null,
            observed_session_count: observed.length,
            missing_session_count: missingAllocationIds.length,
            missing_allocation_ids: missingAllocationIds,
        };
    };
    const tokenValue = (allocation, field) => allocation.selected_evidence?.sidecar?.tokens?.[field] ?? null;
    const toolCallsByName = {};
    for (const allocation of includedAllocations) {
        for (const [toolName, count] of Object.entries(allocation.selected_evidence?.sidecar?.tool_calls?.by_tool ?? {})) {
            if (typeof count === "number" && Number.isFinite(count)) toolCallsByName[toolName] = (toolCallsByName[toolName] ?? 0) + count;
        }
    }
    const statusCounts = {};
    for (const allocation of allocations) statusCounts[allocation.status] = (statusCounts[allocation.status] ?? 0) + 1;
    const totals = {
        included_session_count: includedAllocations.length,
        status_counts: statusCounts,
        wall_ms: aggregateNumericField("wall_ms", (allocation) => allocation.selected_evidence?.sidecar?.process?.wall_ms ?? null),
        tokens: {
            input_tokens: aggregateNumericField("input_tokens", (allocation) => tokenValue(allocation, "input_tokens")),
            output_tokens: aggregateNumericField("output_tokens", (allocation) => tokenValue(allocation, "output_tokens")),
            reasoning_tokens: aggregateNumericField("reasoning_tokens", (allocation) => tokenValue(allocation, "reasoning_tokens")),
            cache_read_input_tokens: aggregateNumericField("cache_read_input_tokens", (allocation) => tokenValue(allocation, "cache_read_input_tokens")),
            cache_creation_input_tokens: aggregateNumericField("cache_creation_input_tokens", (allocation) => tokenValue(allocation, "cache_creation_input_tokens")),
            total_tokens: aggregateNumericField("total_tokens", (allocation) => tokenValue(allocation, "total_tokens")),
        },
        tool_calls: {
            total: aggregateNumericField("tool_calls.total", (allocation) => allocation.selected_evidence?.sidecar?.tool_calls?.total ?? null),
            by_tool: toolCallsByName,
        },
        score: aggregateNumericField("score", (allocation) => allocation.score),
    };
    return {
        schema_version: 1,
        recovered_at: new Date().toISOString(),
        evidence_reconstruction: true,
        reconstruction_invocations: { model: 0, scorer: 0, opencode: 0 },
        source_ledgers: [allocationLedgerPath(outRoot), path.join(outRoot, "attempt-ledger.jsonl")],
        allocation_count: allocations.length,
        deduplication: {
            rule: "use each non-empty session_id once; use selected attempt_id only when session_id is unavailable",
            duplicate_session_identifiers: duplicateSessionIds,
        },
        totals,
        errors,
        missing,
        allocations,
    };
}

async function runSyntheticSlotGate(config) {
    const syntheticRoot = path.join(config.outRoot, "synthetic-slot-gate");
    const episodes = [
        { allocation_id: "SYNTH-OK-01", arm_id: "baseline", codebase: "ClickHouse-master", round: 101 },
        { allocation_id: "SYNTH-FAIL-02", arm_id: "a2", codebase: "ClickHouse-master", round: 102, synthetic_failure: true },
        { allocation_id: "SYNTH-AFTER-03", arm_id: "baseline", codebase: "ClickHouse-master", round: 103 },
    ];
    const fakeRun = async (episode, _runConfig, hooks) => {
        const allocation = allocationEpisodeFromInput(episode);
        const startedAt = new Date().toISOString();
        hooks.onPhase("allocation_reserved", { allocation_id: allocation.allocation_id, synthetic: true, started_at: startedAt });
        hooks.onPhase("solver_started", { allocation_id: allocation.allocation_id, synthetic: true, pid: process.pid, process_group_id: process.pid });
        hooks.onPhase("solver_terminal", { allocation_id: allocation.allocation_id, synthetic: true, terminal_at: new Date().toISOString() });
        hooks.onPhase("scorer_terminal", { allocation_id: allocation.allocation_id, synthetic: true, scorer_terminal_at: new Date().toISOString(), skipped_reason: episode.synthetic_failure ? "synthetic_failure" : null });
        hooks.onPhase("final_recorded", { allocation_id: allocation.allocation_id, synthetic: true, final_record_at: new Date().toISOString(), terminal_event: episode.synthetic_failure ? "terminal_failure" : "completed" });
        return { arm_id: episode.arm_id, runtime: "synthetic", codebase: episode.codebase, round: episode.round, wall_time_s: 0, extraction_status: episode.synthetic_failure ? "empty" : "success", scorer_score: null, mutation_guard_status: "clean", harness_valid: !episode.synthetic_failure, episode_dir: null, skipped: false };
    };
    const results = await runBatch(episodes, { ...config, outRoot: syntheticRoot, runEpisodeImpl: fakeRun });
    const lifecycle = readFileSync(path.join(syntheticRoot, "global-slot-lifecycle.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
    const firstStart = lifecycle.find((event) => event.allocation_id === "SYNTH-OK-01" && event.phase === "allocation_reserved");
    const firstReleased = lifecycle.find((event) => event.allocation_id === "SYNTH-OK-01" && event.phase === "released");
    const failureStart = lifecycle.find((event) => event.allocation_id === "SYNTH-FAIL-02" && event.phase === "allocation_reserved");
    const failureReleased = lifecycle.find((event) => event.allocation_id === "SYNTH-FAIL-02" && event.phase === "released");
    const afterStart = lifecycle.find((event) => event.allocation_id === "SYNTH-AFTER-03" && event.phase === "allocation_reserved");
    const ordered = Date.parse(failureStart.recorded_at) > Date.parse(firstReleased.recorded_at) && Date.parse(afterStart.recorded_at) > Date.parse(failureReleased.recorded_at);
    const proof = { dry_run: true, model_invocations: 0, results, ordered, first_started_at: firstStart.recorded_at, first_released_at: firstReleased.recorded_at, failure_started_at: failureStart.recorded_at, failure_released_at: failureReleased.recorded_at, after_started_at: afterStart.recorded_at, lifecycle };
    writeJson(path.join(syntheticRoot, "slot-order.synthetic.json"), proof);
    if (!ordered) throw new Error("[runner:slot] synthetic ordering proof failed");
    return proof;
}

// ============================================================
// main
// ============================================================

function readRecoveryEpisodes(episodesArgument) {
    if (typeof episodesArgument !== "string" || episodesArgument.length === 0) {
        throw new Error("[runner:recovery] --episodes is required");
    }
    const trimmed = episodesArgument.trim();
    const episodesText = trimmed.startsWith("[") || trimmed.startsWith("{")
        ? episodesArgument
        : existsSync(episodesArgument) ? readFileSync(episodesArgument, "utf8") : episodesArgument;
    const parsed = JSON.parse(episodesText);
    const episodes = (Array.isArray(parsed) ? parsed : parsed.episodes)?.map((episode) => ({
        ...episode,
        codebase: episode.codebase ?? episode.task,
        round: canonicalRound(episode.round),
    }));
    if (!Array.isArray(episodes) || episodes.length === 0) throw new Error("[runner:recovery] episodes must be a non-empty array");
    const allocationIds = new Set();
    for (const episode of episodes) {
        const allocation = allocationEpisodeFromInput(episode);
        if (allocationIds.has(allocation.allocation_id)) throw new Error(`[runner:recovery] duplicate allocation_id: ${allocation.allocation_id}`);
        allocationIds.add(allocation.allocation_id);
    }
    validateEpisodeSchedule(episodes);
    return episodes;
}

function readSessionRecoveryAllowlist(argument) {
    if (typeof argument !== "string" || argument.length === 0) {
        throw new Error("[runner:session_recovery] --session-recovery-allowlist is required");
    }
    const trimmed = argument.trim();
    const text = trimmed.startsWith("[") || trimmed.startsWith("{")
        ? argument
        : existsSync(argument) ? readFileSync(argument, "utf8") : argument;
    const parsed = JSON.parse(text);
    const allowlist = Array.isArray(parsed) ? parsed : parsed.allowlist;
    if (!Array.isArray(allowlist) || allowlist.length === 0) throw new Error("[runner:session_recovery] allowlist must be a non-empty array");
    for (const field of ["allocation_id", "episode_id", "attempt_id", "session_id"]) {
        const values = allowlist.map((entry) => entry?.[field]);
        if (values.some((value) => typeof value !== "string" || value.length === 0)) throw new Error(`[runner:session_recovery] every allowlist entry requires ${field}`);
        if (new Set(values).size !== values.length) throw new Error(`[runner:session_recovery] duplicate ${field} in allowlist`);
    }
    return allowlist;
}

async function main() {
    const args = parseArgs(process.argv.slice(2));

    if (args["recover-opencode-sessions"]) {
        const recoveryRootArgument = typeof args["recover-opencode-sessions"] === "string" ? args["recover-opencode-sessions"] : args["out-root"];
        if (typeof recoveryRootArgument !== "string" || recoveryRootArgument.length === 0) {
            throw new Error("[runner:session_recovery] use --recover-opencode-sessions <existing-out-root>");
        }
        const recoveryRoot = path.resolve(recoveryRootArgument);
        requireDirectory("session recovery output root", recoveryRoot);
        const recoveryEpisodes = readRecoveryEpisodes(args.episodes);
        const allowlist = readSessionRecoveryAllowlist(args["session-recovery-allowlist"]);
        const episodeByAllocation = new Map(recoveryEpisodes.map((episode) => [allocationEpisodeFromInput(episode).allocation_id, episode]));
        for (const entry of allowlist) {
            const episode = episodeByAllocation.get(entry.allocation_id);
            const expectedEpisodeId = episode ? `${episode.arm_id ?? episode.arm}__${episode.codebase}__round-${episode.round}` : null;
            if (!episode || expectedEpisodeId !== entry.episode_id) throw new Error(`[runner:session_recovery] allowlist episode is absent from --episodes: ${entry.allocation_id}`);
        }
        const summaryPath = path.join(recoveryRoot, "opencode_session_recovery.summary.json");
        if (existsSync(summaryPath)) throw new Error(`[runner:session_recovery] refusing to replace recovery summary: ${summaryPath}`);
        const summary = await recoverOpenCodeSessionsFromAllowlist(recoveryRoot, allowlist);
        writeJsonAtomically(summaryPath, summary);
        const recoveredAggregate = recoverAggregateFromLedgers(recoveryRoot, recoveryEpisodes);
        recoveredAggregate.generated_by = { mode: "opencode_export_recovery_then_aggregate", source_mutation: false, recovery_summary_path: summaryPath };
        const recoveredPath = path.join(recoveryRoot, "batch_summary.recovered.json");
        writeJsonAtomically(recoveredPath, recoveredAggregate);
        console.log(`[runner:session_recovery] allowlisted=${allowlist.length} recovered=${summary.results.filter((result) => result.status === "recovered_completed").length} solver_model=0 opencode_run=0 opencode_export=${summary.external_invocations.opencode_export} scorer=${summary.external_invocations.scorer} summary=${summaryPath} aggregate=${recoveredPath}`);
        if (summary.results.some((result) => result.status !== "recovered_completed")) throw new Error(`[runner:session_recovery] one or more allowlisted sessions did not recover; see ${summaryPath}`);
        return { summary, recoveredAggregate };
    }

    if (args["recover-aggregate"]) {
        const recoveryRootArgument = typeof args["recover-aggregate"] === "string" ? args["recover-aggregate"] : args["out-root"];
        if (typeof recoveryRootArgument !== "string" || recoveryRootArgument.length === 0) {
            throw new Error("[runner:recovery] use --recover-aggregate <existing-out-root>");
        }
        const recoveryRoot = path.resolve(recoveryRootArgument);
        requireDirectory("recovery output root", recoveryRoot);
        const recoveryEpisodes = readRecoveryEpisodes(args.episodes);
        const recoveredAggregate = recoverAggregateFromLedgers(recoveryRoot, recoveryEpisodes);
        recoveredAggregate.generated_by = { mode: "recovery_only_cli", source_mutation: false };
        const recoveredPath = path.join(recoveryRoot, "batch_summary.recovered.json");
        writeJsonAtomically(recoveredPath, recoveredAggregate);
        console.log(`[runner:recovery] allocations=${recoveredAggregate.allocation_count} included_sessions=${recoveredAggregate.totals.included_session_count} errors=${recoveredAggregate.errors.length} missing=${recoveredAggregate.missing.length} path=${recoveredPath}`);
        return recoveredAggregate;
    }

    if (args["protocol-v2-synthetic-replay"]) {
        const proof = protocolV2SyntheticReplay();
        if (args["out-root"]) {
            const proofPath = path.join(path.resolve(args["out-root"]), "protocol-v2-synthetic-replay.json");
            writeJson(proofPath, proof);
            console.log(`[runner:protocol-v2-synthetic-replay] passed=${proof.passed} path=${proofPath}`);
        } else {
            console.log(JSON.stringify(proof));
        }
        return;
    }

    // 필수 인자 체크
    for (const k of ["arm-config", "manifest", "scorer", "schema-dir", "out-root"]) {
        if (!args[k]) {
            console.error(`[runner] missing required arg --${k}`);
            console.error(
                "usage: runner.mjs --arm-config <path> --manifest <path> --scorer <path> --schema-dir <dir> --out-root <dir> [--workspace-root <dir>] --episodes <json> [--timeout-s 1800] [--opencode-no-output-timeout-s 60] [--concurrency 1] [--judge-model opus] [--skip-scorer] [--force]",
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
    const timeoutMs = millisecondsFromPositiveSeconds(args["timeout-s"], "timeout-s", DEFAULT_SOLVER_TIMEOUT_SECONDS);
    const opencodeNoOutputTimeoutMs = millisecondsFromPositiveSeconds(
        args["opencode-no-output-timeout-s"],
        "opencode-no-output-timeout-s",
        DEFAULT_OPENCODE_NO_OUTPUT_TIMEOUT_SECONDS,
    );
    const requestedConcurrency = Number(args.concurrency ?? CONCURRENCY.DEFAULT_GLOBAL_CAP);
    if (!Number.isInteger(requestedConcurrency) || requestedConcurrency < 1) {
        throw new Error("[runner:concurrency] --concurrency must be a positive integer");
    }
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
        const parsedEpisodes = JSON.parse(episodesText);
        episodes = Array.isArray(parsedEpisodes) ? parsedEpisodes : parsedEpisodes.episodes;
    } else {
        console.error("[runner] --episodes 인자 없음. proof-slice 실행 시 --episodes 필수.");
        process.exit(2);
    }

    if (!Array.isArray(episodes) || episodes.length === 0) {
        console.error("[runner] episodes가 비어있음");
        process.exit(2);
    }
    const episodeKeys = new Set();
    const allocationIds = new Set();
    for (const episode of episodes) {
        if (!episode.codebase && episode.task) episode.codebase = episode.task;
        const round = canonicalRound(episode.round);
        episode.round = round;
        const allocation = allocationEpisodeFromInput(episode);
        const key = `${episode.arm_id ?? episode.arm}__${episode.codebase}__round-${round}`;
        if (episodeKeys.has(key)) throw new Error(`[runner] duplicate episode input rejected: ${key}`);
        if (allocationIds.has(allocation.allocation_id)) throw new Error(`[runner:allocation] duplicate allocation_id input rejected: ${allocation.allocation_id}`);
        episodeKeys.add(key);
        allocationIds.add(allocation.allocation_id);
    }
    validateEpisodeSchedule(episodes);

    if (args["allocation-gate-only"]) {
        const gates = episodes.map((episode) => {
            const allocation = allocationEpisodeFromInput(episode);
            assertAllocationEpisodeUnused(outRoot, allocation.allocation_id);
            return {
                allocation_id: allocation.allocation_id,
                replacement: allocation.replacement,
                logical_episode_id: `${episode.arm_id ?? episode.arm}__${episode.codebase}__round-${episode.round}`,
                allowed: true,
            };
        });
        const gatePath = path.join(outRoot, "allocation-gate.dry-run.json");
        writeJson(gatePath, { dry_run: true, model_invocations: 0, preflight_invocations: 0, gates });
        console.log(`[runner:allocation-gate] wrote ${gatePath}`);
        return;
    }

    const selectedArms = episodes.map((episode) => armConfig.arms.find((arm) => arm.arm_id === (episode.arm_id ?? episode.arm)));
    if (selectedArms.some((arm) => !arm)) throw new Error("[runner] episode references an unknown arm");
    const needsCodemap = selectedArms.some((arm) => arm.backend === "codemap");
    if (needsCodemap && args["codemap-bin"]) throw new Error("[runner:binary] --codemap-bin is forbidden for arm-attested crossover runs; each episode must resolve clean.artifacts[arm_id]");
    const codemapBin = null;
    const codemapBinary = null;
    assertLedgerIdentitySchema(outRoot);

    const config = {
        armConfig,
        manifest,
        scorerPath,
        schemaDir,
        outRoot,
        timeoutMs,
        opencodeNoOutputTimeoutMs,
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
        requestedConcurrency,
        batchConcurrencyObserved: { max_in_flight: 0 },
        preflightSemaphore: new Semaphore(1),
        preflightConcurrencyCap: 1,
        paths: {
            runnerPath: path.resolve(process.argv[1]),
            manifestPath: path.resolve(args["manifest"]),
            armConfigPath: path.resolve(args["arm-config"]),
            outRoot,
            workspaceRoot,
        },
    };

    console.log(
        `[runner:start] episodes=${episodes.length} timeout_s=${timeoutMs / 1000} opencode_no_output_timeout_s=${opencodeNoOutputTimeoutMs / 1000} termination_grace_ms=${PROCESS_TERMINATION_GRACE_MS} judge_model=${judgeModel} skip_scorer=${skipScorer} codemap_bin=${codemapBin ?? "not-required"} requested_concurrency=${requestedConcurrency} preflight_cap=${config.preflightConcurrencyCap} serena=${CONCURRENCY.SERENA_GLOBAL}+codebase${CONCURRENCY.SERENA_PER_CODEBASE} codegraph=${CONCURRENCY.CODEGRAPH_GLOBAL} force=${force}`,
    );

    if (args["synthetic-slot-gate"]) {
        const proof = await runSyntheticSlotGate(config);
        console.log(`[runner:synthetic-slot-gate] ordered=${proof.ordered}`);
        return;
    }

    if (args["dry-run"]) {
        const plans = [];
        for (const episode of episodes) {
            const allocation = allocationEpisodeFromInput(episode);
            assertAllocationEpisodeUnused(outRoot, allocation.allocation_id);
            const prepared = await prepareEpisodeForResume(episode, config);
            const prompt = readFileSync(prepared.publicQuestionPath, "utf8");
            const plan = buildEpisodeCommand(prepared.armDef, prepared.targetRoot, prompt, prepared.episodeDir, prepared.codemapBin ?? codemapBin, prepared.fixture.proof.codemap_home ?? null);
            plans.push({
                allocation_id: allocation.allocation_id,
                replacement: allocation.replacement,
                episode_id: `${episode.arm_id ?? episode.arm}__${episode.codebase}__round-${episode.round}`,
                task_id: episode.codebase,
                target_root: prepared.targetRoot,
                codemap_home: prepared.fixture.proof.codemap_home ?? null,
                external_index_path: prepared.fixture.proof.external_index_path ?? null,
                command: plan.command,
                args: plan.args,
                cwd: plan.cwd,
                codemap_home_forwarded: plan.env.CODEMAP_HOME ?? null,
                opencode_config: plan.opencodeConfigPath ? { path: plan.opencodeConfigPath, ...fileDigest(plan.opencodeConfigPath) } : null,
            });
        }
        const dryRunPath = path.join(outRoot, "clean_child_resolution.dry-run.json");
        writeJson(dryRunPath, { dry_run: true, model_invocations: 0, plans });
        console.log(`[runner:dry-run] wrote ${dryRunPath}`);
        return;
    }

    let results;
    let recoveredAggregate;
    try {
        results = await runBatch(episodes, config);
    } finally {
        recoveredAggregate = recoverAggregateFromLedgers(outRoot, episodes);
        writeJsonAtomically(path.join(outRoot, "batch_summary.recovered.json"), recoveredAggregate);
    }

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
            requested_concurrency: requestedConcurrency,
            observed_max_in_flight: config.batchConcurrencyObserved.max_in_flight,
            execution_windows: "fixed input-order windows; all participants reach the solver barrier before spawn",
            serena: { global: CONCURRENCY.SERENA_GLOBAL, per_codebase: CONCURRENCY.SERENA_PER_CODEBASE },
            codegraph: { global: CONCURRENCY.CODEGRAPH_GLOBAL },
            light: "global_only", // codemap, no-mcp
            claude: "parallel", // claude_serial 제거
            scorer_out_of_lock: true,
            per_episode_backend_cleanup: true,
            preflight: { cap: config.preflightConcurrencyCap, cache: "canonical root/runtime/binary identity + deterministic fixture contract hash" },
            memory_guard: "serena: free+inactive<8GB or pressure>=warn → wait",
        },
        solver_timeout: {
            overall_timeout_ms: timeoutMs,
            opencode_no_output_timeout_ms: Math.min(timeoutMs, opencodeNoOutputTimeoutMs),
            termination_grace_ms: PROCESS_TERMINATION_GRACE_MS,
            force_settle_grace_ms: PROCESS_FORCE_SETTLE_GRACE_MS,
        },
        recovered_aggregate_path: path.join(outRoot, "batch_summary.recovered.json"),
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
    buildExecutionWindows,
    buildEpisodeCommand,
    canonicalJson,
    commandLine,
    CONCURRENCY,
    calcBackendExercised,
    diffSnapshots,
    evaluationContractSha256,
    parseOpencodeProtocolV2,
    protocolV2SyntheticReplay,
    isBackendArtifactPath,
    isBackendToolName,
    loadCompletedEpisode,
    makeSerenaCodebaseLockFactory,
    readMemoryStatus,
    recoverAggregateFromLedgers,
    recoverOpenCodeSessionsFromAllowlist,
    recoverOpencodeSessionFinal,
    runBatch,
    runEpisode,
    runProcess,
    portableBundleDigestV2,
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
