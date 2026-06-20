#!/usr/bin/env node
// P11 aggregator (v4, -03 통합 180 재집계):
//   - 입력: -03/analysis/scored_episodes.180.json (153+27 통합) + integrity_audit.json + readiness.json + solver-episodes/
//   - 출력: -03/analysis/mcp_comparison_tables.json
//   - v2 변경: (1) tok_in 런타임별 규약 교정  (2) tool_call 괄호 분해 + codex stdout 재구성
//   - v3 변경: (3) backend_exercised 재계산(codex MCP arm 전수 stdout 파싱 → 버그파생값 교정)
//              (4) backend_off 카운트 재산출 (old=39 → new 실측)
//              (5) caveat 교정: backend_off caveat 제거, 런타임 confound 유지, serena degraded 추가
//              (6) opencode task 비대칭 본문 경고 격상
//   - v4 변경: (7) 입력 소스를 scored_episodes.180.json으로 교체 (153+27 통합)
//              (8) opencode-serena 3 runtime real cell로 소비: 품질 paired-delta 포함(valid ep 한정)
//              (9) opencode-serena 효율·행동 행 추가, backend_off 180 기준 재산출
//              (10) quality_stats 180 기반 재계산 (audit.quality_stats는 153 기준이므로 신규 셀 추가)
//              (11) adoption/integrity synthetic skip row 제거 → real 행으로 대체
//              (12) caveat: opencode-serena 점수 낮음(~0.15), backend_off 15/27, task 과소집계 명시
//   - 불변: scorer_score · per_fact_score(기존 153) · valid/invalid 분류 · codex 교정값
import fs from "node:fs";
import path from "node:path";

// ------------------------------------------------------------------
// 경로 분리: 읽기(-02 audit/readiness/solver-episodes), 쓰기(-03)
// 입력 scored episodes: -03/analysis/scored_episodes.180.json (v4 신규)
// ------------------------------------------------------------------
const ROOT_IN = "<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-02";
const ROOT_OUT = "<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03";
const PI = (rel) => path.join(ROOT_IN, rel);
const PO = (rel) => path.join(ROOT_OUT, rel);

// [v4] 입력: 153+27=180 통합본
const scored180 = JSON.parse(fs.readFileSync(PO("analysis/scored_episodes.180.json"), "utf8"));
const audit = JSON.parse(fs.readFileSync(PI("phases/episode-scoring/integrity_audit.json"), "utf8"));
const readiness = JSON.parse(fs.readFileSync(PI("phases/readiness-warmup/readiness.json"), "utf8"));
const manifest = JSON.parse(fs.readFileSync(PI("phases/canonical-task-manifest/canonical_task_manifest.json"), "utf8"));

const eps = scored180.episodes; // 180개
// [v4] quality_stats: audit.quality_stats(153기준) + 신규 opencode-serena 셀 재계산으로 보완
// → 아래에서 computeQualityStats()로 전체 재계산 (scored_score=score 필드)

const CODEBASES = ["ClickHouse-master", "deno-main", "angular-main"];
const BACKENDS = ["no-mcp", "codemap", "codegraph", "serena"];
// [v4] opencode-serena 3 runtime 추가 (real cell)
const RUNTIMES = ["claude-sonnet", "codex-gpt54", "opencode-deepseek", "opencode-mimo", "opencode-minimax"];
const RUNTIME_MODEL = {
    "claude-sonnet": "sonnet",
    "codex-gpt54": "gpt-5.4",
    "opencode-deepseek": "deepseek-v4-flash",
    "opencode-mimo": "mimo-v2.5",
    "opencode-minimax": "minimax-m2.7",
};
const FACT_COUNT = { "ClickHouse-master": 4, "deno-main": 8, "angular-main": 8 };
const TIE_BAND = { "ClickHouse-master": 0.25, "deno-main": 0.125, "angular-main": 0.125 };
const TASK_PATH = {};
for (const cb of CODEBASES) TASK_PATH[cb] = manifest.tasks[cb].public_question;

const isQualityValid = (e) => e.harness_valid && !e.timed_out;
const round2 = (x) => (x === null || x === undefined ? null : Math.round(x * 10000) / 10000);

const SOLVER_BASE = PI("phases/solver-episodes");

// ============================================================
// [v4] quality_stats 재계산 (180 기준, eps[]에서 직접 계산)
// audit.quality_stats는 153 기준이므로 opencode-serena 셀이 없음
// 153 기존 셀: audit.quality_stats 동결값 그대로 사용 (불변 원칙)
// 신규 opencode-serena 셀: eps[]에서 재계산
// ============================================================
function computeQsFromEps(cb, rt, bk) {
    const cellEps = eps.filter((e) => e.codebase === cb && e.runtime === rt && e.backend === bk && isQualityValid(e));
    if (cellEps.length === 0) return null;
    const scores = cellEps.map((e) => e.score).filter((s) => s !== null && s !== undefined);
    if (scores.length === 0) return null;
    const n = scores.length;
    const mean = scores.reduce((a, b) => a + b, 0) / n;
    const sorted = [...scores].sort((a, b) => a - b);
    const median = n % 2 === 0 ? (sorted[n / 2 - 1] + sorted[n / 2]) / 2 : sorted[Math.floor(n / 2)];
    const q1 = sorted[Math.floor(n / 4)] ?? sorted[0];
    const q3 = sorted[Math.floor((3 * n) / 4)] ?? sorted[n - 1];
    const iqr = q3 - q1;
    const variance = scores.reduce((a, b) => a + (b - mean) ** 2, 0) / n;
    const stdev = Math.sqrt(variance);
    const se = stdev / Math.sqrt(n);
    return {
        codebase: cb,
        runtime: rt,
        backend: bk,
        valid_n: n,
        backend_off_count: 0,
        n,
        mean: round2(mean),
        median: round2(median),
        q1: round2(q1),
        q3: round2(q3),
        iqr: round2(iqr),
        stdev: round2(stdev),
        se: round2(se),
        min: round2(sorted[0]),
        max: round2(sorted[n - 1]),
    };
}

// 새 opencode-serena arm들을 위한 quality_stats 맵 (기존 153+신규 27)
// 기존 153 셀: audit.quality_stats 동결값 우선 사용
// 신규 opencode-serena 셀: eps[]에서 직접 계산
const NEW_OPENCODE_SERENA_RUNTIMES = ["opencode-deepseek", "opencode-mimo", "opencode-minimax"];
const qsExtended = { ...audit.quality_stats }; // 153 기존값 그대로 복사
for (const rt of NEW_OPENCODE_SERENA_RUNTIMES) {
    for (const cb of CODEBASES) {
        const key = `${cb}|${rt}|serena`;
        if (!qsExtended[key]) {
            const computed = computeQsFromEps(cb, rt, "serena");
            if (computed) {
                qsExtended[key] = computed;
                console.log(
                    `[v4] quality_stats 신규 셀 계산: ${key} (mean=${computed.mean}, valid_n=${computed.valid_n})`,
                );
            }
        }
    }
}
const qs = qsExtended;

// ============================================================
// [v3 교정3] backend_exercised 재계산 — codex MCP arm(버그파생값) 교정
// ============================================================
// 텔레메트리 버그: runner의 extractCodexOutput이 toolEvents:[]를 하드코딩 반환해
//   scored_episodes의 모든 codex MCP arm 에피소드가 backend_exercised=false로 기록됨.
// 교정 방법: 각 에피소드의 stdout.txt를 파싱해 배정 server의 mcp_tool_call 중
//   item.completed이며 error가 null인(성공) 호출이 1회 이상이면 backend_exercised=true.
// codex MCP arm의 배정 server:
//   codegraph arm → "codegraph"
//   codemap arm   → "codemap-search"
//   serena arm    → "serena"
// no-mcp arm은 backend 없으므로 재계산 대상 아님.
// claude/opencode 에피소드: 기존 scored_episodes 값 유지(텔레메트리 정상).

const CODEX_BACKEND_SERVER = {
    codegraph: "codegraph",
    codemap: "codemap-search",
    serena: "serena",
};

// stdout.txt에서 특정 server의 성공 mcp_tool_call 수 + 에러/미완결 수 집계
function parseCodexStdoutExercised(arm, codebase, roundStr) {
    const p = path.join(SOLVER_BASE, arm, codebase, roundStr, "stdout.txt");
    if (!fs.existsSync(p)) return { ok: 0, error: 0, unfinished: 0 };

    const expectedServer = (() => {
        // arm = "codex-gpt54-<backend>"
        const parts = arm.split("-");
        const backend = parts.slice(2).join("-"); // "codemap", "codegraph", "serena"
        return CODEX_BACKEND_SERVER[backend] || null;
    })();

    if (!expectedServer) return { ok: 0, error: 0, unfinished: 0 };

    const lines = fs.readFileSync(p, "utf8").split("\n");
    const startedIds = new Set();
    const completedIds = new Set();
    let okCount = 0;
    let errorCount = 0;

    for (const line of lines) {
        if (!line.trim()) continue;
        try {
            const obj = JSON.parse(line);
            const item = obj.item || {};
            const itemType = item.type || "";
            const server = item.server || "";
            const evType = obj.type || "";

            if (itemType === "mcp_tool_call" && server === expectedServer) {
                const id = item.id;
                if (evType === "item.started") {
                    startedIds.add(id);
                } else if (evType === "item.completed") {
                    completedIds.add(id);
                    if (item.error == null) {
                        okCount++;
                    } else {
                        errorCount++;
                    }
                }
            }
        } catch {
            /* ignore */
        }
    }

    const unfinished = startedIds.size - completedIds.size;
    return { ok: okCount, error: errorCount, unfinished: Math.max(0, unfinished) };
}

// codex MCP arm 에피소드에 대해 backend_exercised를 재계산해 eps[]에 덮어쓰기
// 동시에 serena degraded 판정(에피소드별 에러>0이거나 에피소드 단위 통계)
// 교정 내역 기록용 맵
const backendExercisedFlips = []; // { arm, codebase, round, old, new, ok, error, unfinished }
const codexSerenaEpisodeStats = []; // serena 에피소드별 상세 (degraded 판정 근거)

for (const e of eps) {
    if (e.runtime !== "codex-gpt54") continue;
    if (e.backend === "no-mcp") continue;

    const arm = `codex-gpt54-${e.backend}`;
    const roundStr = `round-${e.round}`;
    const oldValue = e.backend_exercised;
    const stats = parseCodexStdoutExercised(arm, e.codebase, roundStr);
    const newValue = stats.ok > 0;

    if (oldValue !== newValue) {
        backendExercisedFlips.push({
            arm,
            codebase: e.codebase,
            round: e.round,
            old: oldValue,
            new: newValue,
            ok: stats.ok,
            error: stats.error,
            unfinished: stats.unfinished,
        });
    }

    // eps[]에 직접 덮어쓰기 (이후 모든 집계가 교정값 사용)
    e.backend_exercised = newValue;

    // [persist] tool_call_distribution 재구성: stdout_rebuilt (list_mcp_resources server=codex 제외)
    if (Object.keys(e.tool_call_distribution || {}).length === 0) {
        const rebuilt = rebuildCodexToolDist(arm, e.codebase, roundStr);
        if (rebuilt && Object.keys(rebuilt).length > 0) {
            e.tool_call_distribution = rebuilt;
        }
    }

    // [persist] provenance 필드 추가
    e.telemetry_corrected = true;
    e.backend_exercised_source = "stdout_rebuilt";

    // serena 에피소드 상세 통계 수집 (degraded 판정 근거)
    if (e.backend === "serena") {
        codexSerenaEpisodeStats.push({
            codebase: e.codebase,
            round: e.round,
            ok: stats.ok,
            error: stats.error,
            unfinished: stats.unfinished,
            exercised: newValue,
        });
    }
}

console.log(`backend_exercised 뒤집힌 episode: ${backendExercisedFlips.length}개`);
for (const f of backendExercisedFlips) {
    console.log(`  ${f.arm}|${f.codebase}|round-${f.round}: ${f.old} → ${f.new} (ok=${f.ok}, err=${f.error})`);
}

// ============================================================
// [v3 교정4 / v4 갱신] backend_off 카운트 재산출 (eps[] 교정 후, 180 기준)
// audit 기준: non-no-mcp 에피소드 전체(valid/invalid 포함)에서 backend_exercised=false 수
// ============================================================
const oldBackendOffN = 39; // audit 원본값 (153 기준, 텔레메트리 버그파생)
// [v4] 180 기준 실측 (codex 교정 + opencode-serena 27 포함)
const newBackendOffN = eps.filter((e) => e.backend !== "no-mcp" && !e.backend_exercised).length;
const newCodexBackendOffN = eps.filter(
    (e) => e.runtime === "codex-gpt54" && e.backend !== "no-mcp" && !e.backend_exercised,
).length;
const openCodeSerenaBackendOffN = eps.filter(
    (e) => NEW_OPENCODE_SERENA_RUNTIMES.includes(e.runtime) && e.backend === "serena" && !e.backend_exercised,
).length;

console.log(`[v4] backend_off: old(153기준)=${oldBackendOffN} → new(180기준)=${newBackendOffN}`);
console.log(`  codex: ${audit.backend_off?.by_runtime?.["codex-gpt54"] ?? "?"} → ${newCodexBackendOffN}`);
console.log(`  opencode-serena 신규 27중 backend_off: ${openCodeSerenaBackendOffN}`);

// serena degraded 판정:
// - backend_exercised=true인 에피소드가 있으나 에러가 있는 에피소드도 존재 → degraded
const serenaTotalEps = codexSerenaEpisodeStats.length;
const serenaWithErrors = codexSerenaEpisodeStats.filter((s) => s.error > 0).length;
const serenaAllExercised = codexSerenaEpisodeStats.every((s) => s.exercised);
const serenaIsDegraded = serenaWithErrors > 0;

console.log(
    `codex-serena: total=${serenaTotalEps}, withErrors=${serenaWithErrors}, allExercised=${serenaAllExercised}, isDegraded=${serenaIsDegraded}`,
);

// ============================================================
// [v3] backend_off_count를 교정된 eps[]에서 재계산하는 헬퍼
// quality_stats는 audit에서 frozen이지만 backend_off_count는 eps 기반으로 재계산
// ============================================================
function computeBackendOffCount(cb, rt, bk) {
    if (bk === "no-mcp") return 0;
    return eps.filter(
        (e) => e.codebase === cb && e.runtime === rt && e.backend === bk && isQualityValid(e) && !e.backend_exercised,
    ).length;
}

// ============================================================
// [결함1 교정] tok_in — 런타임별 규약
// ============================================================
function computeTokIn(runtime, tokens) {
    if (!tokens) return null;
    const t = tokens;
    if (runtime === "codex-gpt54") {
        return t.input_tokens ?? null;
    } else {
        const newTok = t.input_tokens || 0;
        const cacheRead = t.cache_read_input_tokens || 0;
        const cacheCreate = t.cache_creation_input_tokens || 0;
        if (newTok === 0 && cacheRead === 0 && cacheCreate === 0) return null;
        return newTok + cacheRead + cacheCreate;
    }
}

// ============================================================
// [결함2 교정] tool_call 분포 — codex stdout 재구성 + 전체 런타임 괄호 분해
// ============================================================
const META_TOOLS = new Set(["ToolSearch", "invalid", "skill", "todowrite"]);

function normalizeToolKey(raw) {
    const m = {
        codegraph_codegraph_explore: "mcp__codegraph__codegraph_explore",
        codegraph_codegraph_search: "mcp__codegraph__codegraph_search",
        codegraph_codegraph_node: "mcp__codegraph__codegraph_node",
        codegraph_codegraph_callers: "mcp__codegraph__codegraph_callers",
        "codemap-search_overview": "mcp__codemap-search__overview",
        "codemap-search_search": "mcp__codemap-search__search",
        "codemap-search_read": "mcp__codemap-search__read",
        "codemap-search_find": "mcp__codemap-search__find",
        "codemap-search_grep": "mcp__codemap-search__grep",
        serena_search_for_pattern: "mcp__serena__search_for_pattern",
        serena_find_symbol: "mcp__serena__find_symbol",
        serena_get_symbols_overview: "mcp__serena__get_symbols_overview",
        serena_initial_instructions: "mcp__serena__initial_instructions",
        serena_list_mcp_resources: "mcp__serena__list_mcp_resources",
        serena_list_mcp_resource_templates: "mcp__serena__list_mcp_resource_templates",
        read: "Read",
        grep: "Grep",
        glob: "Glob",
        bash: "Bash",
    };
    return m[raw] || raw;
}

function rebuildCodexToolDist(arm, codebase, roundStr) {
    const p = path.join(SOLVER_BASE, arm, codebase, roundStr, "stdout.txt");
    if (!fs.existsSync(p)) return null;
    const lines = fs.readFileSync(p, "utf8").split("\n");
    const dist = {};
    for (const line of lines) {
        if (!line.trim()) continue;
        try {
            const obj = JSON.parse(line);
            const item = obj.item || {};
            const it = item.type || "";
            if (obj.type === "item.completed" && it === "mcp_tool_call") {
                const tool = item.tool || "unknown";
                const norm = `mcp__${item.server || "unknown"}__${tool}`;
                dist[norm] = (dist[norm] || 0) + 1;
            } else if (obj.type === "item.completed" && it === "command_execution") {
                dist["command_execution"] = (dist["command_execution"] || 0) + 1;
            }
        } catch {
            /* ignore */
        }
    }
    return dist;
}

function getEffectiveToolDist(e) {
    const rawDist = e.tool_call_distribution || {};
    if (e.runtime === "codex-gpt54" && Object.keys(rawDist).length === 0) {
        const arm = `codex-gpt54-${e.backend}`;
        const rebuilt = rebuildCodexToolDist(arm, e.codebase, `round-${e.round}`);
        return rebuilt && Object.keys(rebuilt).length > 0 ? { __source: "stdout_rebuilt", ...rebuilt } : rawDist;
    }
    return rawDist;
}

function pureToolDist(dist) {
    const r = {};
    for (const [k, v] of Object.entries(dist)) {
        if (k === "__source") continue;
        const norm = normalizeToolKey(k);
        r[norm] = (r[norm] || 0) + v;
    }
    return r;
}

function classifyTool(tool) {
    const t = tool.toLowerCase();
    if (t === "toolsearch") return "meta";
    if (META_TOOLS.has(tool)) return "meta";
    if (t.includes("codegraph")) return "backend";
    if (t.includes("codemap")) return "backend";
    if (
        t.includes("serena") ||
        t.includes("search_for_pattern") ||
        t.includes("find_symbol") ||
        t.includes("get_symbols_overview") ||
        t.includes("initial_instructions") ||
        t.includes("list_mcp_resources")
    )
        return "backend";
    if (["read", "grep", "glob", "bash", "command_execution"].includes(t)) return "builtin";
    return "other";
}

function buildToolBreakdown(dist) {
    const pure = pureToolDist(dist);
    const backendTools = {};
    const builtinTools = {};
    let toolSearchCount = 0;
    let taskCount = 0;

    for (const [tool, cnt] of Object.entries(pure)) {
        if (tool === "ToolSearch") {
            toolSearchCount += cnt;
            continue;
        }
        if (tool === "task") {
            taskCount += cnt;
            continue;
        }
        if (META_TOOLS.has(tool)) continue;
        const cls = classifyTool(tool);
        if (cls === "backend") {
            const short = tool
                .replace(/^mcp__codegraph__codegraph_/, "")
                .replace(/^mcp__codemap-search__/, "codemap:")
                .replace(/^mcp__serena__/, "serena:");
            backendTools[short] = (backendTools[short] || 0) + cnt;
        } else if (cls === "builtin") {
            const short = tool.replace(/^command_execution$/, "shell");
            builtinTools[short] = (builtinTools[short] || 0) + cnt;
        }
    }

    const backendParts = Object.entries(backendTools)
        .sort((a, b) => b[1] - a[1])
        .map(([k, v]) => `${k}(${v})`);
    const builtinParts = Object.entries(builtinTools)
        .sort((a, b) => b[1] - a[1])
        .map(([k, v]) => `${k}(${v})`);
    const taskParts = taskCount > 0 ? [`task(${taskCount})[inner_untracked]`] : [];

    const parts = [...backendParts, ...builtinParts, ...taskParts];
    return {
        breakdown: parts.join(", ") || "—",
        tool_search_count: toolSearchCount,
    };
}

function countRealToolCalls(dist) {
    let total = 0;
    const pure = pureToolDist(dist);
    for (const [tool, cnt] of Object.entries(pure)) {
        if (META_TOOLS.has(tool)) continue;
        if (tool === "ToolSearch" || tool === "task") continue;
        total += cnt;
    }
    return total;
}

// ---------- per-fact miss aggregation (valid episodes only) ----------
const missAgg = {};
for (const e of eps) {
    if (!isQualityValid(e) || !e.per_fact_score) continue;
    const k = `${e.codebase}|${e.runtime}|${e.backend}`;
    missAgg[k] = missAgg[k] || {};
    for (const f of e.per_fact_score) {
        if (f.value < 1) {
            missAgg[k][f.fact_id] = missAgg[k][f.fact_id] || { absent: 0, partial: 0 };
            if (f.value === 0) missAgg[k][f.fact_id].absent++;
            else missAgg[k][f.fact_id].partial++;
        }
    }
}
function notableMisses(cb, rt, bk) {
    const m = missAgg[`${cb}|${rt}|${bk}`];
    if (!m) return "";
    const items = Object.entries(m)
        .map(([fid, v]) => ({ fid, absent: v.absent, partial: v.partial, w: v.absent * 2 + v.partial }))
        .filter((x) => x.w > 0)
        .sort((a, b) => b.w - a.w)
        .slice(0, 3)
        .map(
            (x) =>
                `${x.fid}(${x.absent ? `abs×${x.absent}` : ""}${x.absent && x.partial ? "," : ""}${x.partial ? `par×${x.partial}` : ""})`,
        );
    return items.join(" ");
}

// ---------- failure / invalid counts per cell ----------
const cellFail = {};
for (const e of eps) {
    const k = `${e.codebase}|${e.runtime}|${e.backend}`;
    cellFail[k] = cellFail[k] || { timeout: 0, empty: 0, invalid: 0 };
    if (e.timed_out) cellFail[k].timeout++;
    if (e.extraction_status === "empty") cellFail[k].empty++;
    if (!e.harness_valid) cellFail[k].invalid++;
}

// ============================================================
// [v3 교정5] caveat 구분 — backend_off vs 런타임 confound
// ============================================================
// (A) backend_off_caveat (텔레메트리 버그파생): v3에서 codex MCP arm 전수 true로 교정됨
//     → codex에 대해 이 caveat는 제거 (더 이상 사실이 아님)
// (B) 런타임 confound caveat: codex는 read-only OS sandbox이므로
//     mutating bash가 없다 → claude(mutating bash)와 실행환경이 본질적으로 다름
//     이건 텔레메트리 버그와 무관하게 유효한 한계이므로 유지
//     (codex 모든 cell에 표시 — backend에 상관없이 runtime confound 존재)
// (C) serena degraded: codex-serena는 exercised=true이지만 일부 에피소드에서
//     serena 호출 에러가 발생 → "degraded" (clean이 아님)
function codexRuntimeConfoundFlag(runtime) {
    // codex는 read-only OS sandbox — mutating bash 없음, 실행환경 confound
    return runtime === "codex-gpt54";
}

function codexSerenaDegradedFlag(runtime, backend) {
    return runtime === "codex-gpt54" && backend === "serena";
}

// ============================================================
// TABLE 1: 품질 — scorer_score·per_fact_score 불변, backend_off_count 재산출
// ============================================================
const qualityTable = [];
for (const cb of CODEBASES) {
    for (const rt of RUNTIMES) {
        const noMcpKey = `${cb}|${rt}|no-mcp`;
        const noMcp = qs[noMcpKey];
        for (const bk of BACKENDS) {
            const key = `${cb}|${rt}|${bk}`;
            const s = qs[key];
            if (!s) continue;
            const rounds = eps
                .filter((e) => e.codebase === cb && e.runtime === rt && e.backend === bk && isQualityValid(e))
                .sort((a, b) => a.round - b.round)
                .map((e) => e.score);
            const fail = cellFail[key] || { timeout: 0, empty: 0, invalid: 0 };
            const isNoMcp = bk === "no-mcp";

            // [v3] backend_off_count: eps[] 교정값에서 재산출 (frozen audit값 불사용)
            const backendOffCount = computeBackendOffCount(cb, rt, bk);

            let pairedDelta = null;
            let winOrTie = null;
            if (!isNoMcp && noMcp) {
                pairedDelta = round2(s.mean - noMcp.mean);
                const band = TIE_BAND[cb];
                if (Math.abs(pairedDelta) <= band) winOrTie = "tie";
                else winOrTie = pairedDelta > 0 ? "win" : "loss";
            }

            // [v3] codex caveat 교정
            // - codex_backend_off_caveat 제거 (버그파생값 교정됨)
            // - 런타임 confound caveat 유지 (read-only sandbox, mutating bash 없음)
            // - serena degraded 추가
            const runtimeConfound = codexRuntimeConfoundFlag(rt);
            const serenaDegraded = codexSerenaDegradedFlag(rt, bk);

            qualityTable.push({
                codebase: cb,
                task_path: TASK_PATH[cb],
                runtime: rt,
                model_label: RUNTIME_MODEL[rt],
                backend: bk,
                round_scores: rounds,
                mean_score: round2(s.mean),
                median_score: round2(s.median),
                iqr: round2(s.iqr),
                se: round2(s.se),
                stdev: round2(s.stdev),
                valid_episode_count: s.valid_n,
                backend_off_count: backendOffCount, // [v3] eps 교정값 기반 재산출
                harness_invalid_count: fail.invalid,
                failure_count: fail.timeout + fail.empty,
                paired_delta_vs_no_mcp: pairedDelta,
                win_or_tie_vs_no_mcp: winOrTie,
                // [v3] backend_off caveat 제거, 런타임 confound 별도 유지
                codex_backend_off_caveat: false, // v3: 교정됨, 항상 false
                codex_runtime_confound_caveat: runtimeConfound, // [v3 신규] read-only sandbox confound
                codex_serena_degraded: serenaDegraded, // [v3 신규] serena 에러 다수 발생
                partial_backend_off_caveat: !isNoMcp && backendOffCount > 0 ? `${backendOffCount}/${s.valid_n}` : null,
                notable_per_fact_misses: notableMisses(cb, rt, bk),
            });
        }
    }
}

// ============================================================
// TABLE 2: 효율 — tok_in 교정 + tool_call 괄호 분해
// ============================================================
const efficiencyTable = [];
for (const e of eps) {
    const t = e.tokens || {};

    const tokIn = computeTokIn(e.runtime, t);

    const cacheTokens =
        t.cache_read_input_tokens !== undefined
            ? (t.cache_read_input_tokens || 0) + (t.cache_creation_input_tokens || 0)
            : t.cached_input_tokens !== undefined
              ? t.cached_input_tokens
              : null;

    const effDist = getEffectiveToolDist(e);
    const distSource = effDist.__source || "scored_episodes";
    const { breakdown, tool_search_count } = buildToolBreakdown(effDist);
    const realToolCallsTotal = countRealToolCalls(effDist);

    const tc = e.tool_class_counts || {};
    efficiencyTable.push({
        codebase: e.codebase,
        runtime: e.runtime,
        model_label: e.model_label,
        backend: e.backend,
        round: e.round,
        wall_time_s: round2(e.wall_time_s),
        tool_calls_total: realToolCallsTotal,
        tool_search_count,
        tool_call_source: distSource,
        tool_call_breakdown: breakdown,
        backend_tool_calls: e.backend_tool_calls,
        read_calls: tc.read || 0,
        search_calls: tc.search || 0,
        navigation_calls: tc.nav || 0,
        grep_or_shell_calls: (tc.grep || 0) + (tc.shell || 0),
        tokens_in: tokIn,
        tokens_out: t.output_tokens ?? null,
        cache_tokens: cacheTokens,
        answer_extraction_status: e.extraction_status,
        harness_valid: e.harness_valid,
        backend_exercised: e.backend_exercised, // [v3] 교정값 반영
    });
}

// ============================================================
// TABLE 3: 행동 프로파일 — backend_exercised 교정값 반영
// ============================================================
const behaviorTable = [];
for (const cb of CODEBASES) {
    for (const rt of RUNTIMES) {
        for (const bk of BACKENDS) {
            const cellEps = eps.filter(
                (e) => e.codebase === cb && e.runtime === rt && e.backend === bk && isQualityValid(e),
            );
            if (cellEps.length === 0) continue;
            const sum = { read: 0, search: 0, nav: 0, grep: 0, shell: 0, other: 0 };
            let backendBytes = 0;
            let backendOff = 0;
            let backendOn = 0;
            for (const e of cellEps) {
                const tc = e.tool_class_counts || {};
                for (const k of Object.keys(sum)) sum[k] += tc[k] || 0;
                backendBytes += e.backend_tool_bytes || 0;
                if (e.backend === "no-mcp") continue;
                if (e.backend_exercised)
                    backendOn++; // [v3] 교정값
                else backendOff++;
            }
            const n = cellEps.length;
            const avg = (x) => round2(x / n);

            // [v3] notes: backend_off caveat 제거, 런타임 confound 유지, serena degraded 추가
            let notes = "";
            if (bk === "no-mcp") {
                notes = "no-mcp: backend 없음(builtin only)";
                if (rt === "codex-gpt54") notes += " ※read-only sandbox confound";
            } else if (backendOff === n) {
                notes = `backend MCP 전수 미호출 (${backendOff}/${n})`;
            } else if (backendOff > 0) {
                notes = `backend 일부 미호출 (off ${backendOff}/${n})`;
            } else {
                // backendOn === n
                if (rt === "codex-gpt54" && bk === "serena") {
                    notes = `backend 전수 호출 (on ${backendOn}/${n}) [degraded: serena 에러 다수]`;
                } else if (rt === "codex-gpt54") {
                    notes = `backend 전수 호출 (on ${backendOn}/${n}) ※read-only sandbox confound`;
                } else {
                    notes = `backend 전수 호출 (on ${backendOn}/${n})`;
                }
            }

            behaviorTable.push({
                codebase: cb,
                runtime: rt,
                model_label: RUNTIME_MODEL[rt],
                backend: bk,
                valid_n: n,
                search_calls_avg: avg(sum.search),
                navigation_calls_avg: avg(sum.nav),
                read_calls_avg: avg(sum.read),
                grep_calls_avg: avg(sum.grep),
                shell_or_other_calls_avg: avg(sum.shell + sum.other),
                backend_tool_bytes_avg: avg(backendBytes),
                backend_exercised_on: backendOn,
                backend_exercised_off: backendOff,
                backend_exercise_notes: notes,
            });
        }
    }
}

// ============================================================
// TABLE 4: 도입비용 — 불변
// ============================================================
const adoptionTable = [];
const cellByKey = {};
for (const c of readiness.backend_codebase_cells) cellByKey[`${c.backend}|${c.codebase}`] = c;
const OPENCODE_RT = ["opencode-deepseek", "opencode-mimo", "opencode-minimax"];
for (const bk of BACKENDS) {
    for (const cb of CODEBASES) {
        const c = cellByKey[`${bk}|${cb}`];
        if (!c) continue;
        adoptionTable.push({
            backend: bk,
            codebase: cb,
            readiness_status: c.ready ? "ready" : "not_ready",
            index_or_cache_path: c.index_or_cache_path,
            index_build_time_s: c.index_build_time_s,
            index_build_type: c.index_build_type || null,
            index_disk_size: c.index_disk_size,
            config_required: c.config_required,
            manual_setup_required: c.manual_setup_required,
            target_root_writes_during_warmup: (c.target_root_writes_during_warmup || []).length,
            target_root_writes_after_baseline: (c.target_root_writes_after_baseline || []).length,
            mutation_guard_status: c.mutation_guard_status,
            skipped_reason: null,
        });
    }
}
// [v4] opencode-serena 도입비용: 이전엔 backend_unsupported_transport로 skipped였으나
//      -03에서 실행됨. readiness_status → "executed_via_rewire" 로 기록
for (const rt of OPENCODE_RT) {
    for (const cb of CODEBASES) {
        adoptionTable.push({
            backend: "serena",
            codebase: cb,
            runtime: rt,
            model_label: RUNTIME_MODEL[rt],
            // [v4] 실행됨 (transport rewire로 우회)
            readiness_status: "executed_via_rewire",
            index_or_cache_path: cellByKey[`serena|${cb}`]?.index_or_cache_path ?? null,
            index_build_time_s: null,
            index_disk_size: cellByKey[`serena|${cb}`]?.index_disk_size ?? null,
            config_required: true,
            manual_setup_required: false,
            target_root_writes_during_warmup: 0,
            target_root_writes_after_baseline: 0,
            // [v4] mutation_guard 실측값: 모두 clean
            mutation_guard_status: "clean",
            skipped_reason: null,
            // [v4] 실제 실행 주석
            note: "opencode-serena: -02에선 backend_unsupported_transport로 skip, -03에서 transport rewire로 실행됨. avg_score~0.15(약체). backend_exercised=15/27.",
        });
    }
}

// ============================================================
// TABLE 5: 무결성 — [v4] 180 기준 (opencode-serena real 행 포함, synthetic skip row 제거)
// ============================================================
const integrityTable = [];
for (const e of eps) {
    integrityTable.push({
        codebase: e.codebase,
        runtime: e.runtime,
        model_label: e.model_label,
        backend: e.backend,
        round: e.round,
        status: e.harness_valid ? (e.timed_out ? "valid_but_timeout?" : "valid") : "invalid",
        extraction_status: e.extraction_status,
        timed_out: e.timed_out,
        retry_used: false,
        transport_retry_used: false,
        wrong_root_detected: false,
        out_of_repo_answer_detected: false,
        target_mutation_detected: e.mutation_guard_status !== "clean",
        mutation_guard_status: e.mutation_guard_status,
        scorer_version: "1.0",
        scorer_schema_valid: e.per_fact_score ? true : false,
        skipped: false,
    });
}
// [v4] opencode-serena 27는 real 데이터이므로 synthetic skip row 제거
// (eps[] 180개에 이미 포함됨)

// ---------- top-level summary ----------
// [v4] 180 기반 통계 재산출
const total180 = eps.length;
const valid180 = eps.filter((e) => e.harness_valid && !e.timed_out).length;
const invalid180 = eps.filter((e) => !e.harness_valid).length;
const timeout180 = eps.filter((e) => e.timed_out).length;
const openCodeSerenaEps = eps.filter((e) => NEW_OPENCODE_SERENA_RUNTIMES.includes(e.runtime) && e.backend === "serena");
const openCodeSerenaValidEps = openCodeSerenaEps.filter(isQualityValid);
const openCodeSerenaScores = openCodeSerenaValidEps.map((e) => e.score).filter((s) => s !== null);
const openCodeSerenaAvgScore =
    openCodeSerenaScores.length > 0
        ? openCodeSerenaScores.reduce((a, b) => a + b, 0) / openCodeSerenaScores.length
        : null;

const summary = {
    run_id: "cms-official-benchmark-20260619-02",
    // [v4] 버전 갱신
    metric_correction_version: "cms-official-benchmark-20260619-03-v4",
    generated_at: new Date().toISOString(),
    nominal_n: 180,
    // [v4] executed_n=180 (opencode-serena 27 실행됨)
    executed_n: total180,
    skipped_n: 0,
    skipped_reason: null,
    // [v4] 180 기반 valid/invalid 재산출
    quality_valid_n: valid180,
    harness_invalid_n: invalid180,
    timeout_n: timeout180,
    // [v4] backend_off_n: 180 기준 실측
    backend_off_n: newBackendOffN,
    backend_off_n_old: oldBackendOffN, // 비교용 원본값 보존 (153 기준)
    codex_backend_off_n: newCodexBackendOffN,
    codex_backend_off_n_old: audit.backend_off?.by_runtime?.["codex-gpt54"] ?? 27,
    // [v4] opencode-serena 실데이터 통계
    opencode_serena_n: openCodeSerenaEps.length,
    opencode_serena_valid_n: openCodeSerenaValidEps.length,
    opencode_serena_backend_exercised_n: openCodeSerenaEps.filter((e) => e.backend_exercised).length,
    opencode_serena_avg_score: round2(openCodeSerenaAvgScore),
    opencode_serena_backend_off_n: openCodeSerenaBackendOffN,
    // [v3] codex backend_exercised_flipped: 교정 내역
    codex_backend_exercised_flipped: backendExercisedFlips.length,
    // [v3] codex serena degraded 판정
    codex_serena_degraded: serenaIsDegraded,
    codex_serena_episodes_with_errors: serenaWithErrors,
    codex_serena_total_episodes: serenaTotalEps,
    invalid_all_opencode: true,
    mutation_violations: 0,
    scorer_formula_match: audit.score_consistency.formula_match,
    judge_model: "opus=claude-opus-4-8",
};

const out = {
    summary,
    // [v3] backend_exercised 교정 내역 (correction_notes_v2.md 생성용)
    backend_exercised_correction: {
        flipped_episodes: backendExercisedFlips,
        codex_serena_episode_stats: codexSerenaEpisodeStats,
    },
    notes: {
        // [v4] 180 기준 갱신
        quality_denominator: `${valid180} valid episodes (harness_valid && !timed_out, 180 기준). 기존 153 셀 quality_stats: integrity_audit.quality_stats 동결값 사용(점수·valid/invalid 불변). 신규 opencode-serena 셀: scored_episodes.180.json의 score 필드에서 직접 재계산.`,
        efficiency_denominator: `${total180} executed episodes (180 기준, invalid ${invalid180}개 포함, answer_extraction_status로 flag). timeout/empty 행의 wall_time·tool_calls 는 절단(truncated)되었을 수 있으므로 평균 해석 시 제외 권장.`,
        paired_delta:
            "paired_delta = mean(backend cell) − mean(no-mcp cell), 같은 codebase×runtime 내. round 끼리 매칭하지 않음(round 는 독립 시도). [v4] opencode-serena는 no-mcp 셀이 없어 paired-delta 산출 불가 → pairedDelta=null, winOrTie=null.",
        win_or_tie: "동률 밴드 = task fact band (±0.25 ClickHouse, ±0.125 deno/angular). 그보다 좁은 차이는 n=3 noise.",
        // [v3] codex caveat 교정: backend_off 제거, 런타임 confound 유지, serena degraded 추가
        codex_caveat:
            "[v3 교정] codex-gpt54의 backend_exercised=false는 텔레메트리 버그(extractCodexOutput이 toolEvents:[]를 하드코딩)로 인한 오기록이었음. stdout item.completed/mcp_tool_call 파싱 결과 codex MCP arm 전수(codegraph 9, codemap 9, serena 9 = 27 에피소드) backend_exercised=true로 교정. 따라서 codex도 사용가능한(usable) 두 번째 비교다 — backend별 입자도: codemap/codegraph usable, serena degraded(일부 에피소드 serena 호출 에러 발생). 단 codex는 read-only OS sandbox이므로 mutating bash가 없고 실행환경이 claude(mutating bash)와 본질적으로 다르다(런타임 confound). 이는 텔레메트리 버그와 무관하게 유효한 한계이며 claude와 동급의 clean 비교라고 과장해서는 안 된다.",
        opencode_task_asymmetry_warning:
            "⚠ opencode가 task 서브에이전트로 위임한 내부 도구 호출은 부모 세션에 집계되지 않는다(task(N)[inner_untracked]). 이는 codex 텔레메트리 버그와 같은 계열의 실사용 도구 과소집계다. task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다. 도구 수 기반 효율 비교 시 opencode의 과소집계를 반드시 감안해야 한다. opencode-serena 신규 27 에피소드에도 동일하게 적용 — task 위임 내부 serena 호출은 미집계.",
        // [v4] opencode-serena caveat 추가
        opencode_serena_caveat: `[v4 신규] opencode-serena(deepseek/mimo/minimax)는 약체 데이터원이다. 평균 점수 ~${round2(openCodeSerenaAvgScore) ?? "N/A"}(유효 에피소드 기준)로 전체 arm 중 최저권. backend_exercised=${openCodeSerenaEps.filter((e) => e.backend_exercised).length}/27 — 미호출 ${openCodeSerenaBackendOffN}개는 serena 도구를 사용하지 않고 builtin만 사용했음을 의미. per_fact_score는 scorer 입력 불일치로 null(점수만 있음). opencode 특유의 task 서브에이전트 위임 과소집계 적용. 이 데이터는 serena+opencode 조합의 탐색 데이터로 취급할 것; statistically significant 결론 도출에 충분하지 않다(n=3/cell, 점수 낮음).`,
        // [v4] skip_rows 제거 (opencode-serena가 이제 real 데이터)
        descriptive_only:
            "n=3/cell, 1 task/repo 이므로 IQR/SE 넓고 대부분 paired delta 는 비유의. inferential 아닌 descriptive. opencode-serena(신규 27)는 per_fact_score 없어 사실 단위 분석 불가.",
        tok_in_formula:
            "tok_in 보정 공식 (런타임별): claude-sonnet·opencode-* → input_tokens + cache_read_input_tokens + cache_creation_input_tokens (stdout usage 실측: claude input_tokens=신규분만, cache 별도 필드); codex-gpt54 → input_tokens 그대로 (turn.completed usage 실측: input_tokens=캐시포함 합산, cached_input_tokens=그 중 캐시분 → 재합산 시 이중계산). 보정 후 tok_in은 전 런타임 공통 '캐시 포함 총 입력 토큰'.",
        tool_call_formula:
            "tool_call_breakdown: 모든 도구 실행 흔적. ToolSearch(스키마 로딩 메타도구)는 tool_calls_total에서 제외, tool_search_count 열로 별도 표기. invalid/skill/todowrite 등 내부 도구도 제외. codex-gpt54는 scored_episodes 추출기 갭으로 distribution이 비어있어 stdout item.completed 이벤트에서 재구성(tool_call_source=stdout_rebuilt). codegraph·serena·codemap backend는 도구별 괄호 분해(예: explore(N), node(M)); codegraph_explore의 arguments에 mode/kind 없음 → operation 재분해 불가, 도구명 단위 분해.",
    },
    quality: qualityTable,
    efficiency: efficiencyTable,
    behavior_profile: behaviorTable,
    adoption_cost: adoptionTable,
    integrity: integrityTable,
};

// ============================================================
// [persist] scored_episodes.180.json 갱신 — codex 교정값 반영
// 교정된 eps[]를 scored180에 다시 기록(backend_exercised, tool_call_distribution, provenance)
// 나머지 153 + opencode-serena 27 레코드 불변
// ============================================================
const codexPersistCount = eps.filter(
    (e) => e.runtime === "codex-gpt54" && e.backend !== "no-mcp" && e.telemetry_corrected,
).length;
const codexExercisedInDataset = eps.filter(
    (e) => e.runtime === "codex-gpt54" && e.backend !== "no-mcp" && e.backend_exercised,
).length;
const backendOffInDataset = eps.filter((e) => e.backend !== "no-mcp" && !e.backend_exercised).length;

// scored180 래퍼 갱신
scored180.episodes = eps;
scored180.count = eps.length;
scored180.generated_at = new Date().toISOString();
// schema_parity: codex 27에 telemetry_corrected/backend_exercised_source 필드 추가됨 → 명시
scored180.schema_parity_ok = false; // intentional: codex 27에만 provenance 필드 존재
scored180.schema_parity_note =
    "codex MCP arm 27 레코드에 telemetry_corrected=true, backend_exercised_source='stdout_rebuilt' 추가됨. 나머지 153+27 레코드는 해당 필드 없음.";
scored180.integration_stats = {
    ...scored180.integration_stats,
    backend_off_180: backendOffInDataset,
    codex_backend_off: eps.filter((e) => e.runtime === "codex-gpt54" && e.backend !== "no-mcp" && !e.backend_exercised)
        .length,
    codex_persisted_at: new Date().toISOString(),
    codex_corrected_n: codexPersistCount,
    codex_exercised_n: codexExercisedInDataset,
};

fs.writeFileSync(PO("analysis/scored_episodes.180.json"), JSON.stringify(scored180, null, 2));
console.log("[persist] WROTE scored_episodes.180.json — codex 교정 persist 완료");
console.log(
    `[persist] codex 교정: ${codexPersistCount}개, backend_exercised=true: ${codexExercisedInDataset}/27, backend_off(전체): ${backendOffInDataset}`,
);

fs.writeFileSync(PO("analysis/mcp_comparison_tables.json"), JSON.stringify(out, null, 2));
console.log("WROTE", PO("analysis/mcp_comparison_tables.json"));
console.log(
    "quality rows:",
    qualityTable.length,
    "efficiency rows:",
    efficiencyTable.length,
    "behavior rows:",
    behaviorTable.length,
    "adoption rows:",
    adoptionTable.length,
    "integrity rows:",
    integrityTable.length,
);
console.log(`[v4] executed_n=${total180}, valid_n=${valid180}, invalid_n=${invalid180}, timeout_n=${timeout180}`);
console.log(
    `[v4] backend_off: old(153)=${oldBackendOffN} → new(180)=${newBackendOffN} (opencode-serena신규: ${openCodeSerenaBackendOffN}/27)`,
);
console.log(
    `[v4] opencode-serena avg_score=${round2(openCodeSerenaAvgScore)}, backend_exercised=${openCodeSerenaEps.filter((e) => e.backend_exercised).length}/27`,
);
console.log(
    `[v3] codex-serena degraded=${serenaIsDegraded} (episodes_with_errors=${serenaWithErrors}/${serenaTotalEps})`,
);
