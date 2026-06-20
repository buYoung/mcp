#!/usr/bin/env node
// mcp_comparison_tables.json -> mcp_comparison_tables.md (-03 metric correction v4/180 버전)
import fs from "node:fs";
import path from "node:path";

const ROOT_OUT = "<REPO_ROOT>/.agents/orchestration/cms-official-benchmark-20260619-03";
const o = JSON.parse(fs.readFileSync(path.join(ROOT_OUT, "analysis/mcp_comparison_tables.json"), "utf8"));

const CODEBASES = ["ClickHouse-master", "deno-main", "angular-main"];
const RUNTIMES = ["claude-sonnet", "codex-gpt54", "opencode-deepseek", "opencode-mimo", "opencode-minimax"];
const BACKENDS = ["no-mcp", "codemap", "codegraph", "serena"];
const nz = (x) => (x === null || x === undefined ? "—" : x);
const fmt = (x) => (x === null || x === undefined ? "—" : typeof x === "number" ? String(x) : x);

const md = [];
md.push("# MCP 비교표 — cms-official-benchmark-20260619-03 (metric correction v4 / 180 통합)");
md.push("");
md.push(
    `생성: ${o.summary.generated_at} · 원본 run: ${o.summary.run_id} · judge=${o.summary.judge_model} · scorer formula match=${o.summary.scorer_formula_match}`,
);
md.push(
    `metric correction: ${o.summary.metric_correction_version} — tok_in 런타임별 규약 교정 + tool_call 괄호 분해 + codex stdout 재구성 + backend_exercised 재계산 + opencode-serena 27 실데이터 통합(180 기준)`,
);
md.push("");

// ===== [v3] 텔레메트리 비대칭 경고 (본문 레벨, 각주 아님) =====
md.push("## ⚠ 텔레메트리 비대칭 경고 (데이터 해석 전 필독)");
md.push("");
md.push("**[경고 1] codex-gpt54 backend_exercised 버그 — v3에서 교정됨**");
md.push("");
md.push(
    `이전(-03 v2)의 backend_off=${o.summary.backend_off_n_old}(codex-gpt54 단독 ${o.summary.codex_backend_off_n_old})은 텔레메트리 버그 파생값이다. runner의 \`extractCodexOutput\`이 \`toolEvents:[]\`를 하드코딩 반환해 codex MCP arm 전 에피소드가 \`backend_exercised=false\`로 오기록됐다. stdout \`item.completed/mcp_tool_call\` 파싱으로 재계산 결과 **codex MCP arm 27 에피소드 전수 교정(false→true)**. 교정 후 backend_off=${o.summary.backend_off_n}(codex 기여 ${o.summary.codex_backend_off_n}).`,
);
md.push("");
md.push("**[경고 2] opencode task 서브에이전트 내부 도구 과소집계 — 미교정 (측정 불가)**");
md.push("");
md.push(
    `opencode가 \`task\` 서브에이전트로 위임한 내부 도구 호출은 **부모 세션에 집계되지 않는다**(\`task(N)[inner_untracked]\`). 이는 codex 버그와 같은 계열의 실사용 도구 과소집계다. task로 위임하는 opencode 행은 실제보다 도구 수가 적게 보인다. 도구 수 기반 효율 비교 시 opencode의 과소집계를 반드시 감안해야 한다. (codex는 v3에서 교정됐으나, opencode task 내부는 구조적으로 추적 불가 — 미교정 상태 유지.)`,
);
md.push("");
md.push("**[경고 3] codex 런타임 confound — 교정 불가, 해석 시 감안 필수**");
md.push("");
md.push(
    "codex는 read-only OS sandbox이므로 mutating bash가 없다. claude(mutating bash)와 실행환경이 본질적으로 다르다. 이는 텔레메트리 버그와 무관하게 유효한 한계다. codex의 MCP 효과를 claude와 동급의 clean 비교로 읽으면 안 된다. codemap/codegraph는 usable(사용가능한 2차 비교), serena는 degraded(일부 에피소드 에러 발생).",
);
md.push("");
md.push("**[경고 4] opencode-serena 약체 데이터 — v4 신규 추가, 과소집계 동일 적용**");
md.push("");
md.push(
    `opencode-serena(deepseek/mimo/minimax × serena × 3 codebase × 3 round = 27 에피소드)는 v4에서 real 데이터로 통합됐다. 단: (1) 평균 점수 ${o.summary.opencode_serena_avg_score}(전체 arm 중 최저권). (2) backend_exercised=${o.summary.opencode_serena_backend_exercised_n}/27 — 15개만 serena를 실제 사용. (3) opencode task 서브에이전트 과소집계(경고2)가 동일 적용 — serena 내부 task 위임 호출 미집계. (4) per_fact_score 없음(scorer 입력 불일치로 null) → 사실 단위 분석 불가. no-mcp 셀 없어 paired-delta 산출 불가. 탐색 데이터로 취급; 통계적으로 유의한 결론 도출에 충분하지 않다.`,
);
md.push("");

md.push("## 요약 (denominator·skip·무결성)");
md.push("");
md.push("| 항목 | 값 |");
md.push("|---|---|");
md.push(`| nominal N | ${o.summary.nominal_n} (5 model/runtime × 4 backend × 3 codebase × 1 task × 3 round) |`);
md.push(`| executed N [v4: 180] | ${o.summary.executed_n} (구: 153 + 신규 opencode-serena 27) |`);
md.push(
    `| skipped N [v4: 0] | ${o.summary.skipped_n} — ${o.summary.skipped_reason ?? "없음(opencode-serena 실행됨)"} |`,
);
md.push(`| quality valid (harness_valid && !timed_out) [v4: 180 기준] | ${o.summary.quality_valid_n} |`);
md.push(`| harness invalid | ${o.summary.harness_invalid_n} (timeout ${o.summary.timeout_n}) |`);
md.push(
    `| backend_exercised=false [v4: 180 기준 실측] | ${o.summary.backend_off_n} (구 153기준: ${o.summary.backend_off_n_old}, codex 27 교정됨 + opencode-serena신규 ${o.summary.opencode_serena_backend_off_n}/27 추가) |`,
);
md.push(
    `| codex backend_exercised 교정 | ${o.summary.codex_backend_exercised_flipped}개 false→true (codex MCP arm 전수) |`,
);
md.push(
    `| codex-serena degraded | ${o.summary.codex_serena_degraded} (에러발생 에피소드 ${o.summary.codex_serena_episodes_with_errors}/${o.summary.codex_serena_total_episodes}) |`,
);
md.push(
    `| opencode-serena [v4 신규] | 27 에피소드, valid=${o.summary.opencode_serena_valid_n}, backend_exercised=${o.summary.opencode_serena_backend_exercised_n}/27, avg_score=${o.summary.opencode_serena_avg_score} (약체 — caveat 참고) |`,
);
md.push(`| mutation violations | ${o.summary.mutation_violations} (mutation_guard 전 episode clean) |`);
md.push("");
md.push("> 해석 규칙: " + Object.values(o.notes).join(" · "));
md.push("");

// ===== tok_in 각주 =====
md.push("### tok_in 런타임별 규약 (결함1 교정)");
md.push("");
md.push("| 런타임 | tok_in 산출 공식 | 근거 |");
md.push("|---|---|---|");
md.push(
    "| claude-sonnet | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | stdout result 이벤트 usage: input_tokens=신규분만(예: 10), cache 필드 분리. 보정 전 tok_in=10 → 보정 후 ~186k |",
);
md.push(
    "| opencode-* | `input_tokens + cache_read_input_tokens + cache_creation_input_tokens` | step_finish tokens: input=신규분, cache.read·write 분리 확인. result_metrics도 동일 분리 구조 |",
);
md.push(
    "| codex-gpt54 | `input_tokens` (그대로) | turn.completed usage: input_tokens=캐시포함 합산(예: 190248), cached_input_tokens=그 중 캐시분(106752). 재합산 시 이중계산 → 불적용 |",
);
md.push("");
md.push(
    "> 보정 후 tok_in = 전 런타임 공통 '캐시 포함 총 입력 토큰'. codex 190k와 claude ~186k는 동일 기준으로 직접 비교 가능.",
);
md.push("");

// ===== 품질 =====
md.push("## 1. 품질 (codebase × runtime × backend)");
md.push("");
md.push("- denominator: " + o.notes.quality_denominator);
md.push("- paired_delta: " + o.notes.paired_delta);
md.push("- win/tie 밴드: " + o.notes.win_or_tie);
md.push("- [v3] codex caveat: " + o.notes.codex_caveat);
md.push("");
md.push(
    "> **[v3 헤드라인]** codex는 이제 MCP를 실제로 호출했으므로 **무의미(null)가 아닌 사용가능(usable)한 두 번째 비교**다. 단 backend별 입자도: codemap/codegraph = usable, serena = degraded(일부 에러). claude와 동급의 clean 비교는 아님 — 런타임 confound 유지.",
);
md.push("");
for (const cb of CODEBASES) {
    md.push(`### ${cb}  (task: ${o.quality.find((r) => r.codebase === cb)?.task_path})`);
    md.push("");
    md.push(
        "| runtime | model | backend | round_scores | mean | median | IQR | SE | stdev | valid_n | off | inval | fail | Δ vs no-mcp | win/tie | confound | notable misses |",
    );
    md.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (const rt of RUNTIMES) {
        for (const bk of BACKENDS) {
            const r = o.quality.find((x) => x.codebase === cb && x.runtime === rt && x.backend === bk);
            if (!r) continue;
            // [v3] codex_backend_off_caveat는 false (교정됨), partial_backend_off_caveat는 비-codex용
            const flag = r.partial_backend_off_caveat ? ` ⚠partial(${r.partial_backend_off_caveat})` : "";
            const delta =
                r.paired_delta_vs_no_mcp === null
                    ? "—"
                    : `${r.paired_delta_vs_no_mcp > 0 ? "+" : ""}${r.paired_delta_vs_no_mcp}${flag}`;
            // [v3] 런타임 confound 표시
            const confoundFlag = r.codex_runtime_confound_caveat
                ? r.codex_serena_degraded
                    ? "sandbox+degraded"
                    : "sandbox"
                : "—";
            md.push(
                `| ${rt} | ${r.model_label} | ${bk} | ${r.round_scores.join(", ")} | ${r.mean_score} | ${r.median_score} | ${r.iqr} | ${r.se} | ${r.stdev} | ${r.valid_episode_count} | ${r.backend_off_count} | ${r.harness_invalid_count} | ${r.failure_count} | ${delta} | ${nz(r.win_or_tie_vs_no_mcp)} | ${confoundFlag} | ${r.notable_per_fact_misses || "—"} |`,
            );
        }
    }
    md.push("");
}
md.push(
    "> ⚠partial(k/n) = 비-codex cell 인데 backend_off>0: valid n 중 k episode 가 backend MCP 미호출(builtin-only)이라 이 paired_delta 도 부분적으로 builtin-only episode 를 반영한다. delta 를 순수 MCP 효과로 읽지 말 것.",
);
md.push(
    "> confound=sandbox: codex는 read-only OS sandbox(mutating bash 없음) — claude와 실행환경이 다름. sandbox+degraded: serena 호출 에러도 추가 발생.",
);
md.push(
    "> [v3] ⚠full (codex backend_off 전수)는 텔레메트리 버그로 인한 오기록이었음. v3에서 제거 — 관련 표 주석 참고.",
);
md.push("");

// ===== 효율 =====
md.push("## 2. 효율 (per-episode, 153 rows)");
md.push("");
md.push("- denominator: " + o.notes.efficiency_denominator);
md.push("- 효율은 같은 runtime/model 안에서만 backend 4종을 비교한다. token 은 tool_calls 와 묶어 해석.");
md.push(
    "- **tok_in 보정**: claude/opencode = input+cache_read+cache_creation(캐시포함 총입력); codex = input_tokens 그대로(이미 캐시포함). 보정 후 전 런타임 동일 기준 비교 가능.",
);
md.push("- cache_tokens: claude/opencode = cache_read + cache_creation, codex = cached_input.");
md.push(
    "- **tool_call_breakdown**: 모든 도구 실행 흔적. ToolSearch(메타도구)는 tool_search_cnt 열로 분리. codex 는 scored_episodes 추출기 갭으로 distribution {} → stdout 재구성(tool_src=rebuilt). codegraph·serena·codemap 는 도구별 괄호 분해.",
);
md.push("");
md.push(`> **[본문 경고] opencode task 내부 도구 과소집계**: ${o.notes.opencode_task_asymmetry_warning}`);
md.push("");
md.push(
    "| codebase | runtime | model | backend | rnd | wall_s | tools | ts_cnt | bk_calls | tok_in | tok_out | cache_tok | tool_breakdown | tool_src | extract |",
);
md.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
for (const cb of CODEBASES) {
    for (const rt of RUNTIMES) {
        for (const bk of BACKENDS) {
            const rows = o.efficiency
                .filter((e) => e.codebase === cb && e.runtime === rt && e.backend === bk)
                .sort((a, b) => a.round - b.round);
            for (const e of rows) {
                const inval = e.harness_valid ? "" : "*";
                const srcShort = e.tool_call_source === "stdout_rebuilt" ? "rebuilt" : "ep";
                md.push(
                    `| ${cb} | ${rt} | ${e.model_label} | ${bk} | ${e.round} | ${fmt(e.wall_time_s)}${inval} | ${e.tool_calls_total} | ${e.tool_search_count || 0} | ${e.backend_tool_calls} | ${fmt(e.tokens_in)} | ${fmt(e.tokens_out)} | ${fmt(e.cache_tokens)} | ${e.tool_call_breakdown} | ${srcShort} | ${e.answer_extraction_status}${inval} |`,
                );
            }
        }
    }
}
md.push("");
md.push(
    "> `*` = harness invalid (timeout/extraction_empty). 해당 행의 wall_time/tool_calls 는 미완(절단) 가능성이 있어 효율 평균 산출에서 제외 권장.",
);
md.push(
    "> tool_src: ep=scored_episodes, rebuilt=codex stdout 재구성(item.completed/mcp_tool_call·command_execution 이벤트 집계).",
);
md.push("> ts_cnt=ToolSearch(스키마 로딩 메타도구, tools 합계 제외). invalid/skill/todowrite 도 제외.");
md.push(
    "> task(N)[inner_untracked]: opencode 서브에이전트 스폰 도구. N=스폰 횟수. 서브에이전트 내부 도구 호출은 parent에 기록되지 않아 추적 불가. tools 합계에도 미포함(측정 불가 명시).",
);
md.push(
    "> codegraph explore 인자는 {query:...} 뿐 mode/kind 없음 → operation 재분해 불가. 도구명 단위 분해(explore/node/search/callers). 단일 codegraph 콜이 serena 여러 콜의 일을 하므로 콜수 직접 비교 시 주의.",
);
md.push("");

// ===== 행동 프로파일 =====
md.push("## 3. 행동 프로파일 (codebase × runtime × backend, valid 평균)");
md.push("");
md.push(
    "| codebase | runtime | model | backend | valid_n | search | nav | read | grep | shell/other | bk_bytes | bk_on | bk_off | notes |",
);
md.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
for (const cb of CODEBASES) {
    for (const rt of RUNTIMES) {
        for (const bk of BACKENDS) {
            const r = o.behavior_profile.find((x) => x.codebase === cb && x.runtime === rt && x.backend === bk);
            if (!r) continue;
            md.push(
                `| ${cb} | ${rt} | ${r.model_label} | ${bk} | ${r.valid_n} | ${r.search_calls_avg} | ${r.navigation_calls_avg} | ${r.read_calls_avg} | ${r.grep_calls_avg} | ${r.shell_or_other_calls_avg} | ${r.backend_tool_bytes_avg} | ${r.backend_exercised_on} | ${r.backend_exercised_off} | ${r.backend_exercise_notes} |`,
            );
        }
    }
}
md.push("");
md.push(
    "> backend_tool_bytes 는 backend MCP 가 반환한 바이트의 cell 평균. no-mcp 는 0(backend 없음). off=valid 중 backend 미호출 episode 수.",
);
md.push(
    "> read_bytes(§12 열): scored_episodes 에 미수록 → 산출 불가. backend_tool_bytes 만 제공한다(누락을 숨기지 않고 명시).",
);
md.push("");

// ===== 도입비용 =====
md.push("## 4. 도입비용 / readiness (backend × codebase — index 비용은 runtime 간 공유)");
md.push("");
md.push(
    "- index_build_time_s / index_disk_size 는 backend×codebase 단위이며 같은 cell 을 쓰는 모든 runtime 이 공유한다(arm 별 중복 부담 아님).",
);
md.push(
    "- opencode×serena 9 cell 은 scored_episodes 에 없는 skip row(backend_unsupported_transport)로 여기에만 기록한다.",
);
md.push("");
md.push(
    "| backend | codebase | runtime | readiness | index/cache path | build_s | build_type | disk | config_req | manual | writes(warmup/after) | mutation_guard | skipped_reason |",
);
md.push("|---|---|---|---|---|---|---|---|---|---|---|---|---|");
for (const r of o.adoption_cost) {
    const p = r.index_or_cache_path ? r.index_or_cache_path.replace("<REPO_ROOT>/.agents/benchmark-data/", "…/") : "—";
    md.push(
        `| ${r.backend} | ${r.codebase} | ${r.runtime || "(shared)"} | ${r.readiness_status} | ${p} | ${fmt(r.index_build_time_s)} | ${fmt(r.index_build_type)} | ${fmt(r.index_disk_size)} | ${r.config_required} | ${r.manual_setup_required} | ${r.target_root_writes_during_warmup}/${r.target_root_writes_after_baseline} | ${r.mutation_guard_status} | ${nz(r.skipped_reason)} |`,
    );
}
md.push("");
md.push(
    "> serena build_s 실측: ClickHouse 62s(clangd cold reindex), angular 41s(tsserver cold), deno warm-cache(<1s, cold 미측정). codegraph angular 80s(cold init). codemap/codegraph 의 다른 cell 은 index 사전존재로 build_s 미측정. serena 디스크가 가장 큼(151–277M).",
);
md.push("");

// ===== 무결성 =====
md.push("## 5. 무결성 요약");
md.push("");
const inv = o.integrity.filter((r) => r.status === "invalid");
const skip = o.integrity.filter((r) => r.skipped);
md.push("| 지표 | 값 |");
md.push("|---|---|");
md.push(`| valid | ${o.integrity.filter((r) => r.status === "valid").length} |`);
md.push(
    `| invalid | ${inv.length} (timeout ${inv.filter((r) => r.extraction_status === "timeout").length} + empty ${inv.filter((r) => r.extraction_status === "empty").length}) |`,
);
md.push(`| skipped (backend_unsupported_transport) | ${skip.length} |`);
md.push(`| wrong_root_detected | ${o.integrity.filter((r) => r.wrong_root_detected).length} |`);
md.push(`| out_of_repo_answer_detected | ${o.integrity.filter((r) => r.out_of_repo_answer_detected).length} |`);
md.push(`| target_mutation_detected | ${o.integrity.filter((r) => r.target_mutation_detected).length} |`);
md.push(`| scorer_version | 1.0 (모든 episode 동일) · formula match=${o.summary.scorer_formula_match} |`);
md.push("");
md.push("### invalid episode (9, 전부 opencode)");
md.push("");
md.push("| runtime | backend | codebase | round | extraction | cause |");
md.push("|---|---|---|---|---|---|");
for (const r of inv)
    md.push(
        `| ${r.runtime} | ${r.backend} | ${r.codebase} | ${r.round} | ${r.extraction_status} | ${r.timed_out ? "timeout" : "extraction_empty"} |`,
    );
md.push("");
md.push(
    "> wrong_root / out_of_repo / target_mutation = 0 (전 episode). skip 27 = opencode×serena transport 미부팅(readiness 가 codex/claude transport 로만 검증 → 실행 시점에 드러난 known-untested transport).",
);
md.push("");

fs.writeFileSync(path.join(ROOT_OUT, "analysis/mcp_comparison_tables.md"), md.join("\n"));
console.log("WROTE mcp_comparison_tables.md (" + md.length + " lines)");
