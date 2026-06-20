#!/usr/bin/env node

// CMS official benchmark — executable temp-0(-deterministic) LLM judge scorer.
//
// 계약(P7 cheap-proof / P10 episode 채점이 그대로 재사용):
//   scorer.mjs --raw-answer <path> --schema <path> --answer-key <path> --out <path>
//     [--judge-model <alias>] [--scorer-config <path>] [--print-cmd]
//
// 동작:
//   frozen scoring_schema.json 의 fact 목록 각각에 대해, closed-book LLM judge가
//   raw answer 가 그 fact 를 demonstrate 하는지 present/partial/absent =>
//   {1.0, 0.5, 0.0} 로 판정한다. judge 는 frozen fact 목록에 대해서만 판정하며
//   fact 를 추가/삭제하지 못한다(정확히 F 개 verdict 강제, fact_id 키 매칭).
//   score = schema 의 weighted average.
//
// 결정성(determinism):
//   judge 호출은 `claude -p`(로그인 세션) 를 쓰며 CLI 는 temperature 노출을 안 한다.
//   따라서 "가장 결정적인 설정"(고정 모델 + 고정 프롬프트 + 모든 도구 비활성 +
//   strict settings/mcp + 고정 JSON schema) 을 쓰고, 잔여 비결정성은 ±1 fact 밴드가
//   흡수한다(scorer_config.json 의 determinism_limit 에 명시).
//
// 출력 scorer_output.json:
//   { score, per_fact_score:[{fact_id, verdict, value∈{0,0.5,1.0}}],
//     scorer_output: { schema_version }, verdict, ... }

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

// ---------- arg parsing ----------
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

const args = parseArgs(process.argv.slice(2));
const required = ["raw-answer", "schema", "answer-key", "out"];
for (const k of required) {
    if (!args[k]) {
        console.error(`[scorer] missing required arg --${k}`);
        console.error(
            "usage: scorer.mjs --raw-answer <path> --schema <path> --answer-key <path> --out <path> [--judge-model opus]",
        );
        process.exit(2);
    }
}

const JUDGE_MODEL = args["judge-model"] || "opus"; // resolves to claude-opus-4-8
const VERDICT_VALUE = { present: 1.0, partial: 0.5, absent: 0.0 };

// ---------- load inputs ----------
const schema = JSON.parse(readFileSync(args["schema"], "utf8"));
const answerKey = readFileSync(args["answer-key"], "utf8");
const rawAnswer = readFileSync(args["raw-answer"], "utf8");

const facts = schema.facts;
const F = facts.length;
if (!Array.isArray(facts) || F === 0) {
    console.error("[scorer] schema has no facts");
    process.exit(2);
}

// weighted-average helper (frozen schema 의 score_formula)
function weightedAverage(perFact) {
    let num = 0;
    let den = 0;
    for (const f of facts) {
        const v = perFact[f.fact_id];
        num += f.weight * v;
        den += f.weight;
    }
    return den === 0 ? 0 : num / den;
}

// ---------- build judge prompt (frozen template) ----------
// closed-book: judge 는 codebase 가 아니라 candidate answer 만 본다.
// answer key 는 "각 fact 의 ground truth 정의(루브릭)" 로만 제공한다 — 채점 대상이 아니다.
function buildSystemPrompt() {
    return [
        "You are a deterministic, closed-book grading judge for a code-navigation benchmark.",
        "You score ONLY whether a CANDIDATE ANSWER demonstrates each of a fixed list of FACTS.",
        "The ANSWER KEY is provided solely as ground-truth context that defines what each fact means; it is NOT the thing being graded and it always trivially contains every fact.",
        "You must NOT use any tools, browse any repository, or rely on outside knowledge beyond the provided answer key. Judge the candidate answer's text alone.",
        "For each fact, decide if the CANDIDATE ANSWER demonstrates it:",
        "  - present: the candidate answer clearly and correctly conveys the fact (its core mechanism/claim).",
        "  - partial: the candidate answer gestures at the fact or gets it only generically/incompletely, missing the specific pinned content.",
        "  - absent: the candidate answer does not convey the fact, or contradicts it.",
        "Be strict and consistent. Do not invent, merge, or drop facts. Score exactly the facts given, by their fact_id.",
        "Return ONLY a JSON object matching the required schema. No prose, no markdown fences.",
    ].join("\n");
}

function buildUserPrompt() {
    const factLines = facts.map((f) => `- ${f.fact_id}: ${f.statement}`).join("\n");
    return [
        "## ANSWER KEY (ground-truth context — defines each fact; NOT graded)",
        "",
        answerKey.trim(),
        "",
        "## FROZEN FACTS TO SCORE (score the CANDIDATE ANSWER against each)",
        "",
        factLines,
        "",
        "## CANDIDATE ANSWER (the only text being graded)",
        "",
        "<<<CANDIDATE_ANSWER_START>>>",
        rawAnswer.trim(),
        "<<<CANDIDATE_ANSWER_END>>>",
        "",
        "## TASK",
        "",
        `For each of the ${F} facts above (by fact_id), judge whether the CANDIDATE ANSWER demonstrates it: "present", "partial", or "absent".`,
        `Return a JSON object: {"verdicts": [{"fact_id": "<id>", "verdict": "present|partial|absent"}, ...]} with exactly ${F} entries, one per fact_id, no extras, no omissions.`,
    ].join("\n");
}

// JSON schema forces shape; we still validate count/coverage by construction.
function buildJsonSchema() {
    const factIds = facts.map((f) => f.fact_id);
    return {
        type: "object",
        additionalProperties: false,
        required: ["verdicts"],
        properties: {
            verdicts: {
                type: "array",
                minItems: F,
                maxItems: F,
                items: {
                    type: "object",
                    additionalProperties: false,
                    required: ["fact_id", "verdict"],
                    properties: {
                        fact_id: { type: "string", enum: factIds },
                        verdict: { type: "string", enum: ["present", "partial", "absent"] },
                    },
                },
            },
        },
    };
}

// ---------- invoke judge (claude -p, closed-book, deterministic settings) ----------
function buildClaudeArgs() {
    // 결정성 + closed-book hygiene:
    //   --print                  : 비대화형 1회 응답
    //   --model <alias>          : 고정 judge 모델 (opus => claude-opus-4-8)
    //   --tools ""               : 모든 빌트인 도구 비활성 (closed-book — 핵심)
    //   --strict-mcp-config      : --mcp-config 외 모든 MCP 무시 (여기선 mcp-config 없음 => MCP 0)
    //   --setting-sources ""     : user/project/local 설정 미로드 (재현 hygiene)
    //   --no-session-persistence : 세션 디스크 미저장
    //   --output-format json     : 단일 result JSON
    //   --json-schema <inline>   : 구조화 출력 검증(정확히 F verdict 유도) — inline JSON 문자열
    //   --system-prompt <...>    : 고정 judge 페르소나
    // 주의: --bare 는 사양상 금지(+ ANTHROPIC_API_KEY 강제). temperature flag 없음.
    return [
        "--print",
        "--model",
        JUDGE_MODEL,
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--output-format",
        "json",
        "--json-schema",
        JSON.stringify(buildJsonSchema()),
        "--system-prompt",
        buildSystemPrompt(),
    ];
}

let lastModelUsageKeys = [];

function runJudge() {
    const userPrompt = buildUserPrompt();
    const claudeArgs = buildClaudeArgs();

    if (args["print-cmd"]) {
        console.error("[scorer] claude " + claudeArgs.map((a) => JSON.stringify(a)).join(" "));
    }

    // prompt 는 stdin 으로 전달(긴 텍스트 안전)
    const res = spawnSync("claude", claudeArgs, {
        input: userPrompt,
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
    });

    let parsedResultText = null;
    let outerJson = null;
    try {
        if (res.stdout) outerJson = JSON.parse(res.stdout);
    } catch {
        // stdout 가 순수 result JSON 이 아닐 수 있음 — 아래서 fallback 처리
    }

    if (res.error) {
        return { ok: false, reason: "spawn_error", detail: String(res.error) };
    }
    if (res.status !== 0) {
        return {
            ok: false,
            reason: "nonzero_exit",
            detail: `exit=${res.status} stderr=${(res.stderr || "").slice(0, 800)}`,
        };
    }

    // --output-format json + --json-schema =>
    //   { type:"result", result:"<짧은 ack>", structured_output:{...우리 JSON...}, modelUsage:{...} }
    // 구조화 출력은 structured_output 에 들어온다(result 가 아님).
    if (outerJson && typeof outerJson === "object") {
        // judge 모델이 실제로 opus-4-8 로 해석됐는지 기록(증거)
        if (outerJson.modelUsage && typeof outerJson.modelUsage === "object") {
            lastModelUsageKeys = Object.keys(outerJson.modelUsage);
        }
        if (outerJson.structured_output && typeof outerJson.structured_output === "object") {
            return { ok: true, verdictsObj: outerJson.structured_output, modelUsageKeys: lastModelUsageKeys };
        }
        if (outerJson.verdicts) {
            return { ok: true, verdictsObj: outerJson, modelUsageKeys: lastModelUsageKeys };
        }
        if (typeof outerJson.result === "string") {
            parsedResultText = outerJson.result;
        }
    }
    if (parsedResultText === null) {
        // fallback: stdout 전체를 result 텍스트로 취급
        parsedResultText = res.stdout;
    }

    // result 텍스트에서 JSON 추출(혹시 모를 fence 제거)
    const cleaned = stripFences(parsedResultText);
    let verdictsObj;
    try {
        verdictsObj = JSON.parse(cleaned);
    } catch {
        const m = cleaned.match(/\{[\s\S]*\}/);
        if (m) {
            try {
                verdictsObj = JSON.parse(m[0]);
            } catch {
                return { ok: false, reason: "unparseable_judge_json", detail: cleaned.slice(0, 800) };
            }
        } else {
            return { ok: false, reason: "no_json_in_judge_output", detail: cleaned.slice(0, 800) };
        }
    }
    return { ok: true, verdictsObj, modelUsageKeys: lastModelUsageKeys };
}

function stripFences(s) {
    if (!s) return "";
    let t = s.trim();
    if (t.startsWith("```")) {
        t = t.replace(/^```[a-zA-Z]*\s*/, "").replace(/```\s*$/, "");
    }
    return t.trim();
}

// ---------- main ----------
const judge = runJudge();
if (!judge.ok) {
    console.error(`[scorer] judge failed: ${judge.reason} :: ${judge.detail || ""}`);
    process.exit(3);
}

const verdicts = judge.verdictsObj.verdicts;
if (!Array.isArray(verdicts)) {
    console.error("[scorer] judge output missing verdicts[]");
    process.exit(3);
}

// frozen-facts-only 강제: 정확히 F 개, 모든 fact_id 1:1 커버, 중복/외부 fact 금지
const byId = new Map();
for (const v of verdicts) {
    if (!v || typeof v.fact_id !== "string") {
        console.error("[scorer] verdict entry malformed");
        process.exit(3);
    }
    if (byId.has(v.fact_id)) {
        console.error(`[scorer] duplicate verdict for ${v.fact_id} (fact invention/dup rejected)`);
        process.exit(3);
    }
    byId.set(v.fact_id, v.verdict);
}
const factIdSet = new Set(facts.map((f) => f.fact_id));
for (const id of byId.keys()) {
    if (!factIdSet.has(id)) {
        console.error(`[scorer] verdict for unknown fact_id ${id} (fact invention rejected)`);
        process.exit(3);
    }
}
if (byId.size !== F) {
    console.error(`[scorer] expected ${F} verdicts, got ${byId.size} distinct fact_ids`);
    process.exit(3);
}

const perFactValue = {};
const perFactOut = [];
for (const f of facts) {
    const verdict = byId.get(f.fact_id);
    if (!(verdict in VERDICT_VALUE)) {
        console.error(`[scorer] invalid verdict "${verdict}" for ${f.fact_id}`);
        process.exit(3);
    }
    const value = VERDICT_VALUE[verdict];
    perFactValue[f.fact_id] = value;
    perFactOut.push({ fact_id: f.fact_id, verdict, value });
}

const score = weightedAverage(perFactValue);

const answerSha = createHash("sha256").update(rawAnswer).digest("hex");

const output = {
    task_id: schema.task_id,
    candidate_id: schema.candidate_id,
    score: Number(score.toFixed(6)),
    per_fact_score: perFactOut,
    scorer_output: {
        schema_version: schema.schema_version,
    },
    schema_version: schema.schema_version,
    verdict: score >= 0.5 ? "pass" : "fail", // descriptive threshold; 본 채점은 점수만 사용
    judge_model: JUDGE_MODEL,
    judge_model_resolved: judge.modelUsageKeys || [], // claude -p 가 실제 사용한 모델 키(증거)
    fact_count_F: F,
    reproduction_tolerance_band: schema.reproduction_tolerance_band,
    raw_answer_path: args["raw-answer"],
    answer_sha256: answerSha,
    partial_credit_rule: schema.partial_credit_rule,
    score_formula: schema.score_formula,
};

writeFileSync(args["out"], JSON.stringify(output, null, 2));
console.error(`[scorer] ${schema.task_id} score=${output.score} (F=${F}) -> ${args["out"]}`);
console.log(JSON.stringify({ score: output.score, per_fact_score: perFactOut }));
