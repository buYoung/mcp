#!/usr/bin/env python3
"""Render the related-lookup assessment as one offline Korean HTML report."""

from __future__ import annotations

from datetime import datetime
from html import escape
import json
from pathlib import Path
from zoneinfo import ZoneInfo


REPO = Path(__file__).resolve().parents[4]
RUN = REPO / ".codemap/poc/related-terms/evaluation/related-lookup-v3/assessment-1"
OUTPUT = REPO / "docs/reports/related-terms-lookup-assessment.html"


def load(relative: str):
    return json.loads((RUN / relative).read_text())


def pct(value: float) -> str:
    return f"{value * 100:.1f}%"


def badge(status: str) -> str:
    names = {"pass": "통과", "fail": "불합격", "pending": "집계 중", "inconclusive": "판정 유보"}
    return f'<span class="badge {escape(status)}">{names[status]}</span>'


def effect_rows(effect: dict | None) -> str:
    if effect is None:
        return '<tr><td>Grafana · codemap-search</td><td colspan="5">두 모델의 반복 실행을 수집하고 있습니다.</td></tr>'
    rows = []
    for target in ("grafana", "codemap-search"):
        item = effect["targets"][target]
        rows.append(
            f'<tr><th scope="row">{escape(target)}</th><td>{item["cases"]}</td>'
            f'<td>{pct(item["control"])}</td><td>{pct(item["treatment"])}</td>'
            f'<td class="number">{item["difference"] * 100:+.1f}%p</td><td>{badge(item["status"])}</td></tr>'
        )
    return "".join(rows)


def model_rows(effect: dict | None) -> str:
    if effect is None:
        return ""
    rows = []
    for target, item in effect["targets"].items():
        for model, values in item["per_model"].items():
            name = "gpt-6-luna" if model == "luna" else "gpt-6.1-sol"
            rows.append(f'<tr><td>{escape(target)}</td><td>{name}</td><td>{pct(values["control"])}</td>'
                        f'<td>{pct(values["treatment"])}</td><td>{values["difference"] * 100:+.1f}%p</td></tr>')
    return "".join(rows)


def main() -> None:
    quality = load("quality/judgment.json")
    preparation = load("preparation.json")
    diagnostics = load("quality/diagnostics.json")
    effect_path = RUN / "search/summary.json"
    effect = load("search/summary.json") if effect_path.exists() else None
    technical_path = REPO / ".codemap/poc/related-terms/thesaurus-v1/verification-uncapped/verification.json"
    technical = json.loads(technical_path.read_text())["summary"]
    technical_pass = (
        technical["keys_parity"] == "pass" and not technical["probe_misses_by_category"]
        and not technical["fixed_cases_failed"] and technical["regression_identical"]
        and not technical["freeze_changed"] and not technical["invariants"]["unsorted_outputs"]
        and not technical["invariants"]["outputs_with_unscored_candidates"]
        and not technical["symbol_invariance"].get("different", 0)
    )
    complete = effect is not None
    overall_pass = technical_pass and quality["recommended_status"] == "pass" and bool(effect and effect["status"] == "pass")
    verdict = "통과" if overall_pass else "불합격"
    headline = f"종합 판정: {verdict}" if complete else "현재 판정: 의미 품질 불합격"
    lead = (
        "현재 관련어 제공 방식은 이번 합격 기준을 충족하지 못했습니다. 구현은 동작하지만, 검색 과제에서 반환된 후보의 의미 품질이 기준에 크게 못 미칩니다."
        if not overall_pass else
        "현재 데이터와 조회 방식은 이번 비교에서 추천 합격 기준을 충족했습니다. 제품 통합 전에는 새로운 문제집으로 재검증해야 합니다."
    )
    if not complete:
        lead += " 검색 효과는 집계 중이며, 완료 후 이 보고서에 반영합니다."
    elif effect["status"] == "fail":
        lead += " 실제 검색에서도 두 대상 모두 필요한 개선 폭을 넘지 못했습니다."
    interval = quality["wilson_95"]
    uncertainty_count = quality["counts"].get("uncertain", 0)
    effect_status = effect["status"] if effect else "pending"
    timestamp = datetime.now(ZoneInfo("Asia/Seoul")).strftime("%Y-%m-%d %H:%M KST")
    source_group = diagnostics["source_groups"]["llm_only"]
    selected_examples = diagnostics["error_examples"][:4]
    example_rows = "".join(
        f'<tr><td><code>{escape(row["input"])}</code></td><td><code>{escape(row["candidate"])}</code></td>'
        f'<td>{escape(row["rationale"])}</td></tr>'
        for row in selected_examples
    )
    bin_names = {"80+": "80% 이상", "60-80": "60% 이상 · 80% 미만", "below60": "60% 미만"}
    score_rows = "".join(
        f'<tr><td>{bin_names[key]}</td><td>{diagnostics["score_bins"][key]["useful"]} / '
        f'{diagnostics["score_bins"][key]["n"]}</td>'
        f'<td>{pct(diagnostics["score_bins"][key]["useful"] / diagnostics["score_bins"][key]["n"])}</td></tr>'
        for key in ("80+", "60-80", "below60")
    )
    useful_width = quality["useful_rate"] * 100
    error_width = quality["error_rate"] * 100
    uncertain_width = uncertainty_count / quality["sample_count"] * 100
    effect_note = (
        f'32문제 × 두 조건 × 두 모델 × 10회, 총 {effect["observations"]:,}개 관측을 채점했습니다. '
        f'중복을 합친 실제 검색은 기준 검색을 포함해 {effect["searches"]:,}회입니다.'
        if effect else
        "각 모델이 관련어 없이 10회, 관련어를 읽고 10회씩 검색어를 만듭니다. 생성한 검색어는 고정된 인덱스에서 실제로 실행합니다."
    )
    exclusions = RUN / "search/rejected-partial-read.json"
    rejected = len(json.loads(exclusions.read_text())["records"]) if exclusions.exists() else 0
    effect_direction = (
        "검색 효과도 두 대상에서 기준을 넘지 못했다면, 현재 형태의 제공 방식을 제품에 넣을 근거가 없습니다."
        if effect is None else
        "현재 전량 출력은 의미 품질과 검색 효과 모두 기준에 미달했습니다. 제품 도입은 보류하고, 이번 결과를 다음 정제 실험의 비교 기준으로 사용합니다."
        if effect["status"] == "fail" and quality["recommended_status"] == "fail" else
        "품질과 검색 효과를 함께 확인한 뒤 제품 도입을 결정하고, 이번 결과를 다음 실험의 비교 기준으로 사용합니다."
    )
    cli = """B=apps/codemap-search/experiments/related-terms-poc/lookup-v3/target/release/rtpoc3
D=.codemap/poc/related-terms/thesaurus-v1/datasets/rtr1-930791c83bea6132/dataset.sqlite3
"$B" related --dataset "$D" --format text '$eventLoop' .net 'c++' retry"""
    root_link = "../../.codemap/poc/related-terms/evaluation/related-lookup-v3/assessment-1"
    html = f"""<!doctype html>
<html lang="ko">
<head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>관련어 조회 PoC — 종합 판정과 다음 방향</title>
<style>
:root{{--ink:#1b272c;--muted:#5d6b72;--paper:#f4f6f5;--line:#dce3df;--green:#146448;--red:#a33d2e;--amber:#815a19;--soft:#e9eeeb}}
*{{box-sizing:border-box}}html{{scroll-behavior:smooth}}body{{margin:0;background:var(--paper);color:var(--ink);font-family:-apple-system,BlinkMacSystemFont,"Apple SD Gothic Neo","Malgun Gothic",sans-serif;font-size:16px;line-height:1.75}}
a{{color:var(--green);text-underline-offset:3px}}nav{{background:#fff;border-bottom:1px solid var(--line)}}.nav-inner{{max-width:1120px;margin:auto;padding:16px 28px;display:flex;align-items:center;justify-content:space-between;gap:24px}}.brand{{font-weight:800;letter-spacing:-.04em}}.nav-links{{display:flex;gap:22px;flex-wrap:wrap;font-size:14px}}.nav-links a{{text-decoration:none;color:var(--muted)}}
main{{max-width:1120px;margin:auto;padding:42px 28px 70px}}.eyebrow{{font-size:13px;color:var(--green);font-weight:800;letter-spacing:.08em;margin:0 0 12px}}h1{{font-size:44px;line-height:1.2;letter-spacing:-.055em;margin:0 0 18px}}h2{{font-size:27px;line-height:1.35;letter-spacing:-.04em;margin:0 0 20px}}h3{{font-size:18px;margin:0 0 10px;letter-spacing:-.025em}}p{{margin:0 0 14px}}.lead{{max-width:860px;font-size:19px;line-height:1.75}}.meta{{font-size:13px;color:var(--muted);margin:16px 0 30px}}.hero{{padding:30px 32px;background:#fff;border:1px solid var(--line);border-left:5px solid var(--red);border-radius:12px}}
.cards{{display:grid;grid-template-columns:repeat(3,1fr);gap:16px;margin:24px 0 38px}}.card{{background:#fff;padding:22px;border:1px solid var(--line);border-radius:12px}}.card .label{{font-size:13px;font-weight:700;color:var(--muted)}}.card .value{{display:block;font-size:30px;font-weight:800;letter-spacing:-.04em;margin:7px 0 8px}}.card p{{font-size:14px;color:var(--muted);margin:12px 0 0}}.badge{{display:inline-block;padding:3px 11px;border-radius:99px;font-size:13px;font-weight:800;white-space:nowrap}}.pass{{background:#e3f2e9;color:var(--green)}}.fail{{background:#fae8e3;color:var(--red)}}.pending,.inconclusive{{background:#fff0d4;color:var(--amber)}}section{{scroll-margin-top:24px;margin-top:42px}}.panel{{background:#fff;border:1px solid var(--line);border-radius:12px;padding:28px;margin-top:18px}}.section-number{{font-size:13px;color:var(--muted);font-weight:700;display:block;margin-bottom:8px}}
.table-wrap{{overflow:auto;border:1px solid var(--line);border-radius:9px}}table{{border-collapse:collapse;width:100%;font-size:14px}}th,td{{text-align:left;padding:13px 16px;border-bottom:1px solid var(--line);vertical-align:top}}thead{{background:#edf2ee}}th{{font-weight:750}}tr:last-child td,tr:last-child th{{border-bottom:0}}.number{{font-variant-numeric:tabular-nums;white-space:nowrap;font-weight:750}}.small{{font-size:13px;color:var(--muted)}}.note{{background:#edf3ee;border-left:3px solid var(--green);padding:16px 18px;border-radius:0 8px 8px 0;margin:20px 0}}.note p:last-child{{margin-bottom:0}}.split{{display:grid;grid-template-columns:1fr 1fr;gap:20px}}.metric{{font-size:35px;line-height:1.2;font-weight:800;letter-spacing:-.04em;margin:8px 0 12px}}.metric small{{font-size:16px;font-weight:500;letter-spacing:0;color:var(--muted)}}.distribution{{display:flex;height:20px;border-radius:8px;overflow:hidden;margin:20px 0 10px}}.distribution span:nth-child(1){{background:var(--green)}}.distribution span:nth-child(2){{background:var(--red)}}.distribution span:nth-child(3){{background:#879590}}.legend{{display:flex;gap:22px;flex-wrap:wrap;font-size:13px;color:var(--muted)}}.legend b{{color:var(--ink)}}ul{{padding-left:22px;margin:12px 0}}li{{padding:4px 0}}code{{font-family:ui-monospace,SFMono-Regular,Consolas,monospace;font-size:.88em;overflow-wrap:anywhere}}pre{{margin:14px 0;padding:20px;overflow:auto;background:#18292a;color:#f2f7f4;border-radius:9px;line-height:1.6;white-space:pre}}pre code{{overflow-wrap:normal;font-size:12px}}details{{border-top:1px solid var(--line);padding-top:18px;margin-top:20px}}summary{{cursor:pointer;font-weight:750;padding:4px 0 12px}}.steps{{list-style:none;padding:0;counter-reset:step}}.steps li{{counter-increment:step;position:relative;padding:0 0 24px 45px}}.steps li::before{{content:counter(step);position:absolute;left:0;top:1px;width:29px;height:29px;background:var(--green);color:#fff;border-radius:50%;text-align:center;font-size:14px;font-weight:800;line-height:29px}}.steps p{{font-size:15px;color:var(--muted)}}.sources{{display:grid;grid-template-columns:1fr 1fr;gap:9px 26px;font-size:14px}}footer{{margin-top:42px;padding-top:18px;border-top:1px solid var(--line);color:var(--muted);font-size:12px}}
@media(max-width:720px){{main{{padding:28px 18px 50px}}.nav-inner{{padding:14px 18px;display:block}}.nav-links{{margin-top:9px;gap:16px}}h1{{font-size:34px}}h2{{font-size:23px}}.hero{{padding:24px 22px}}.lead{{font-size:17px}}.cards,.split{{grid-template-columns:1fr}}.cards{{gap:10px}}.card{{padding:18px 20px}}.panel{{padding:22px 18px}}.sources{{grid-template-columns:1fr}}th,td{{padding:11px 12px}}}}
@media print{{body{{background:#fff;font-size:11pt}}nav{{display:none}}main{{padding:0;max-width:none}}.panel,.card,.hero{{break-inside:avoid}}section{{margin-top:24px}}a{{color:inherit}}pre{{white-space:pre-wrap;color:#111;background:#eee}}.table-wrap{{overflow:visible}}}}
</style>
</head>
<body>
<nav aria-label="보고서 목차"><div class="nav-inner"><span class="brand">관련어 조회 PoC</span><div class="nav-links"><a href="#verdict">판정</a><a href="#criteria">합격 기준</a><a href="#quality">의미 품질</a><a href="#effect">검색 효과</a><a href="#next">다음 방향</a><a href="#evidence">근거</a></div></div></nav>
<main>
<header id="verdict">
<p class="eyebrow">평가 보고서 · {timestamp}</p>
<div class="hero"><h1>{headline}</h1><p class="lead">{lead}</p></div>
<p class="meta">대상: <code>rtpoc3 related</code> · 데이터셋 <code>rtr1-930791c83bea6132</code> · 실험 도구만 평가, 제품 코드 변경 없음</p>
</header>
<div class="cards">
<article class="card"><span class="label">동작 검증</span><span class="value">28,312건</span>{badge("pass" if technical_pass else "fail")}<p>표기 변형의 기대 일치 누락 0건.<br>고정 사례·정렬·회귀 검사도 통과했습니다.</p></article>
<article class="card"><span class="label">의미 품질 · 유용한 관계</span><span class="value">{pct(quality["useful_rate"])}</span>{badge(quality["recommended_status"])}<p>{quality["useful"]}/{quality["sample_count"]}쌍이 유용했습니다.<br>추천 기준은 80% 이상입니다.</p></article>
<article class="card"><span class="label">검색 효과</span><span class="value">32문제</span>{badge(effect_status)}<p>두 모델 × 조건별 10회.<br>두 대상 모두 10%p 초과 개선이 필요합니다.</p></article>
</div>

<section id="criteria"><span class="section-number">01 · 무엇을 통과해야 하나</span><h2>품질과 실제 검색 효과를 함께 판정합니다</h2>
<div class="table-wrap"><table><thead><tr><th>항목</th><th>추천 합격 기준</th><th>이번 확인</th></tr></thead><tbody>
<tr><th scope="row">동작</th><td>정규화·분리·고유어 보호·점수 정렬·기존 조회 회귀 검사 통과</td><td>{badge("pass" if technical_pass else "fail")}</td></tr>
<tr><th scope="row">유용한 관계</th><td>관측 비율 80% 이상, 95% 신뢰구간 하한 75% 이상</td><td>{pct(quality["useful_rate"])} · 하한 {pct(interval[0])} {badge(quality["recommended_status"])}</td></tr>
<tr><th scope="row">의미 오류</th><td>다른 뜻·무관한 관계·경쟁 기술 합계 15% 이하</td><td>{pct(quality["error_rate"])} {badge("pass" if quality["error_rate"] <= .15 else "fail")}</td></tr>
<tr><th scope="row">검색 효과</th><td>관련어를 받은 쪽의 성공률이 통제군보다 대상별로 10%p 초과 높음</td><td>{badge(effect_status)}</td></tr>
</tbody></table></div>
<div class="note"><p><strong>이 기준을 추천하는 이유</strong></p><p>동의어뿐 아니라 <code>retry → backoff</code>, <code>event → listener</code>처럼 코드에서 직접 연결되는 개념도 유용한 관계로 인정합니다. 같은 분야라는 이유만으로 묶은 경쟁 기술과 다른 뜻의 단어는 오류로 봅니다.</p><p>이 PoC에서는 적어도 후보 5개 중 4개가 도움이 되고, 확인된 의미 오류는 15%를 넘지 않도록 권합니다. 95% 신뢰구간 하한도 함께 확인해 표본의 우연한 고득점을 경계합니다. 이는 이 프로젝트의 운영 기준 제안이며, 업계 표준 수치는 아닙니다.</p></div>
<p class="small">이는 요청에 따라 제시한 추천 기준입니다. 기존 공식 기준인 “유용 비율의 95% 하한 ≥ 75%”도 별도로 계산했으며 결과는 <strong>{"불합격" if quality["legacy_status"] == "fail" else "통과" if quality["legacy_status"] == "pass" else "판정 유보"}</strong>입니다. 기준 선택에 따른 결론 차이는 {"없습니다" if quality["legacy_status"] == quality["recommended_status"] else "있습니다"}.</p>
</section>

<section id="quality"><span class="section-number">02 · 관련어가 실제 뜻에 맞나</span><h2>넓게 찾지만, 과제의 뜻과 다른 후보가 많습니다</h2>
<div class="panel">
<div class="split"><div><h3>전체 반환 후보에서 뽑은 무작위 표본</h3><p class="metric">{quality["useful"]}<small> / {quality["sample_count"]}쌍 유용</small></p><p>유용 비율의 95% 신뢰구간은 <strong>{pct(interval[0])}~{pct(interval[1])}</strong>입니다. 표본의 유용 비율과 그 하한 모두 추천 기준에 못 미칩니다.</p></div>
<div><h3>표본 구성</h3><p>실제 과제 168개에서 반환된 <strong>{quality["population_count"]:,}개 과제·입력·후보 쌍</strong>이 모집단입니다. 고정 시드로 {quality["sample_count"]}쌍을 뽑았고, 판단이 불확실한 쌍도 분모에서 제외하지 않았습니다.</p></div></div>
<div class="distribution" role="img" aria-label="유용 {pct(quality['useful_rate'])}, 오류 {pct(quality['error_rate'])}, 불확실 {pct(uncertainty_count / quality['sample_count'])}"><span style="width:{useful_width}%"></span><span style="width:{error_width}%"></span><span style="width:{uncertain_width}%"></span></div>
<div class="legend"><span>유용 <b>{quality["useful"]}쌍 · {pct(quality["useful_rate"])}</b></span><span>오류 <b>{quality["errors"]}쌍 · {pct(quality["error_rate"])}</b></span><span>불확실 <b>{uncertainty_count}쌍</b></span></div>
</div>
<div class="panel"><h3>어디에서 뜻이 달라졌나</h3><p>아래는 에이전트가 오류로 판정한 실제 표본입니다. 구를 이루는 단어가 따로 확장되면서, 그 단어의 다른 뜻이 결과에 섞이는 모습을 보여줍니다.</p>
<div class="table-wrap"><table><thead><tr><th>입력</th><th>반환 후보</th><th>평가 이유</th></tr></thead><tbody>{example_rows}</tbody></table></div>
<p class="small" style="margin-top:14px">LLM 생성 자료만 근거로 가진 표본에서도 {source_group["errors"]}/{source_group["n"]}쌍이 오류였습니다. 이는 해당 후보가 <em>과제의 의도에 맞지 않았다</em>는 평가이며, 그 관계가 모든 문맥에서 틀렸다는 뜻은 아닙니다.</p>
<details><summary>유사도 점수가 높으면 품질도 높았나</summary><p>다음은 판정 후 살펴본 탐색 결과입니다. 유사도는 정답 확률이 아닙니다.</p>
<div class="table-wrap"><table><thead><tr><th>표시된 코사인 유사도</th><th>유용 / 표본</th><th>유용 비율</th></tr></thead><tbody>{score_rows}</tbody></table></div>
<p class="small" style="margin-top:12px">80% 이상 구간은 표본이 15개뿐입니다. 이 관측만으로 80%를 필터 기준으로 확정할 수 없습니다. 후속 실험의 가설로만 사용해야 합니다.</p></details></div>
</section>

<section id="effect"><span class="section-number">03 · 정답 파일을 더 잘 찾나</span><h2>관련어를 읽은 조건과 읽지 않은 조건을 비교했습니다</h2>
<p>기준 검색이 정답을 놓친 Grafana 22문제와 codemap-search 10문제가 대상입니다. 각 사례에서 모델이 후속 검색어 4개를 만들고, 어느 검색이든 상위 10개 안에 정답 파일이 있으면 성공입니다.</p>
<div class="table-wrap"><table><thead><tr><th>대상</th><th>문제 수</th><th>관련어 없음</th><th>관련어 있음</th><th>성공률 차이</th><th>판정</th></tr></thead><tbody>{effect_rows(effect)}</tbody></table></div>
<p class="small" style="margin-top:13px">{effect_note}</p>
<p class="small">합격선은 <strong>+10%p 초과</strong>입니다. 예를 들어 60%에서 70%로 오르면 +10%p이므로 합격이 아니고, 70%를 넘어야 합니다.</p>
<details open><summary>모델별 결과와 비교 범위</summary>
<div class="table-wrap"><table><thead><tr><th>대상</th><th>모델</th><th>관련어 없음</th><th>관련어 있음</th><th>차이</th></tr></thead><tbody>{model_rows(effect)}</tbody></table></div>
<p class="small" style="margin-top:12px"><code>gpt-6-luna</code>(medium)와 <code>gpt-6.1-sol</code>(xhigh)를 각각 조건별 10회 실행했습니다. 이번에는 두 모델의 통제군도 새로 만들었습니다. 예전 Codex·Claude 조합의 수치와 직접 빼서 개선량으로 해석하지 않습니다.</p></details>
</section>

<section id="completed"><span class="section-number">04 · 무엇이 만들어졌나</span><h2>구현과 데이터 보강은 완료됐습니다</h2>
<div class="split"><div class="panel"><h3>현재 산출물</h3><ul><li>52개 범주에서 시작한 <strong>364,074개 표기·916,718개 관계</strong></li><li>SQLite 약 <strong>430.2 MiB</strong>. 조회 시 모델·네트워크·저장소 재색인 불필요</li><li>기호 처리, 식별자·부분 구 분해, 보호 표기, 저장된 벡터의 유사도 정렬</li><li>임의의 70개·50개 상한으로 잘렸던 관계 <strong>12,179개 복원</strong></li></ul></div>
<div class="panel"><h3>통과한 기술 검증</h3><ul><li>Python·Rust 키 전수 검사 <strong>364,074개 일치</strong></li><li>표기 변형 <strong>28,312건</strong>, 고정 사례 <strong>24개</strong></li><li>출력 <strong>28,336개</strong>에서 정렬·점수 누락 오류 0</li><li>기존 조회 <strong>1,187건</strong> 동일, 동결 파일 <strong>12개</strong> 변경 없음</li><li>검사 실패도 종료 코드 0을 반환하던 검증기 수정</li></ul></div></div>
<div class="note"><p>조회 결과가 나온 입력은 <strong>{preparation["nonempty_inputs"]:,}/{preparation["unique_inputs"]:,}개({pct(preparation["nonempty_inputs"] / preparation["unique_inputs"])})</strong>입니다. 다만 반환 유무는 의미 품질이나 검색 효과와 다른 지표입니다.</p></div>
<p>반환량도 확인했습니다. 효과 평가 문제당 관련어 텍스트는 중앙값 <strong>{preparation["related_text_bytes"]["median"] / 1000:.1f}KB</strong>, 최대 <strong>{preparation["related_text_bytes"]["max"] / 1000:.1f}KB</strong>였습니다. 후보를 제한하지 않은 현재 방식에서는 읽기 비용도 다음 설계에서 다뤄야 합니다.</p>
<details><summary>생성 자료의 구성과 남은 데이터 문제</summary>
<div class="table-wrap"><table><thead><tr><th>생성 자료</th><th>표제어</th><th>관계</th><th>표제어당 평균</th></tr></thead><tbody>
<tr><th scope="row">Claude</th><td>11,041개</td><td>519,202개</td><td>47.02개</td></tr>
<tr><th scope="row">Codex</th><td>806개</td><td>17,659개</td><td>21.91개</td></tr>
</tbody></table></div>
<p class="small" style="margin-top:12px">생성 관계 합계는 536,861개입니다. 모델마다 맡은 표제어가 다르므로 평균 개수 차이를 품질 차이로 해석할 수 없습니다. 최종 SQLite는 기존 사전 자료도 합친 결과이며, 관계 수는 방향·종류·출처를 구분한 행 수입니다.</p>
<p><code>batch-009</code>와 <code>batch-013</code>의 임의 상한을 제거하면서 중복 제거·오류 교정은 유지했습니다. 두 배치는 각각 23,297개와 21,418개 관계로 복원됐고, 원자료와 이전 SQLite도 보존했습니다.</p>
<p><code>svelte → Blazor</code>는 여전히 73.9%로 반환됩니다. GitHub Explore의 <code>related_topic</code>을 역방향으로 따른 경쟁 프레임워크 관계입니다. 이런 관계는 다음 의미 정제의 우선 검토 대상입니다.</p>
</details>
</section>

<section id="next"><span class="section-number">05 · 다음에 무엇을 할까</span><h2>데이터 양보다 뜻과 반환 경로를 먼저 정리하는 것이 좋습니다</h2>
<div class="panel"><ol class="steps">
<li><h3>현재 데이터와 평가 결과를 기준선으로 보존합니다</h3><p>{effect_direction} 이번 평가 중에는 사전과 조회 규칙을 튜닝하지 않았습니다.</p></li>
<li><h3>전체 구와 부분 단어에서 나온 후보를 구분해 정제합니다</h3><p><code>source map</code>의 <code>source</code>, <code>app sub url</code>의 <code>sub</code>처럼 다른 뜻으로 퍼지는 경로부터 검토합니다. 식별자 분리 기능은 유지하면서, 경로별로 어떤 후보가 유용한지 비교하는 방식입니다.</p></li>
<li><h3>생성 관계의 뜻·기술 계열을 확인합니다</h3><p>동일 기술의 생태계 관계와 경쟁 기술을 구분하고, 다의어의 뜻을 분리합니다. 출처만으로 일괄 선택하기보다 관계와 입력의 의미를 함께 정제해야 합니다. 조회 시점의 오프라인 동작은 유지할 수 있습니다.</p></li>
<li><h3>필터 후보는 개발 자료로 비교한 뒤 새 문제집으로 최종 판정합니다</h3><p>유사도 구간과 부분 일치 경로는 후속 실험의 후보입니다. 이번 평가 결과에 맞춰 기준을 고치거나 같은 문제집만 반복해 최종 합격을 선언하지 않습니다. 다음에는 합격선과 평가 입력을 다시 고정한 후 측정합니다.</p></li>
</ol><p class="small">위 내용은 다음 단계의 권장 방향입니다. 필터 구현과 제품 통합은 이번 평가 작업에서 수행하지 않았습니다.</p></div>
</section>

<section id="evidence"><span class="section-number">06 · 근거와 해석 범위</span><h2>이번 결과가 말해 주는 범위를 분명히 했습니다</h2>
<div class="panel"><ul>
<li><strong>새로운 미공개 문제집 검증이 아닙니다.</strong> 이전에 살펴본 168개 문제와 고정 인덱스를 재사용했습니다. 이번 결과는 그 문제집에서의 재측정입니다.</li>
<li><strong>품질 라벨은 사람의 정답이 아닙니다.</strong> gpt-6.1-sol 에이전트가 점수·출처·관계 유형을 가린 패킷을 판정했습니다. 각 표본은 한 번 판정했으며, 과제·입력·후보·일치한 입력 부분은 제공했습니다. 신뢰구간은 표본추출의 불확실성을 나타내며 라벨 오류나 모델 편향을 포함하지 않습니다.</li>
<li><strong>32문제의 효과를 측정했습니다.</strong> 반복 20회는 모델 실행의 흔들림을 줄이지만, 새로운 문제 20배를 확보한 것은 아닙니다.</li>
<li><strong>입력 전달에 문제가 있던 결과는 제외했습니다.</strong> 일부만 읽었거나 출력이 잘린 경우, 원문 읽기를 확인할 수 없거나 고정한 원문 대신 압축 표현을 읽은 초기 출력 {rejected}개를 채점 전에 보존·제외하고 재실행했습니다. 재실행은 검색 성공 여부를 보기 전에 결정했습니다.</li>
<li><strong>입력 읽기 감사에는 한계가 있습니다.</strong> 재실행은 입력 묶음별 새 에이전트가 원문을 순차 출력하도록 했습니다. 나머지 실행의 일부 구간은 대화 압축 전 완료 기록과 담당 에이전트의 확인에 근거하며, 전체 도구 출력을 독립적으로 재감사한 것은 아닙니다. 실행별 대화 문맥도 같지 않아 모델 간 차이는 참고용입니다.</li>
<li><strong>품질은 실제 출력 쌍을 균등 추출했습니다.</strong> 52개 어휘 범주를 각각 같은 비중으로 측정한 결과가 아닙니다. 순차 표본 확대 규칙은 기존 방식을 따랐으며, 순차·다중 판정 전체에 대한 별도의 신뢰수준 보정은 하지 않았습니다.</li>
</ul><div class="sources">
<a href="{root_link}/criteria.json">고정한 평가 기준</a><a href="{root_link}/method-freeze.json">입력·코드·데이터 해시</a>
<a href="{root_link}/final-verdict.json">종합 판정 원본</a><a href="{root_link}/search/output-intake.json">채점 전 고정한 출력 해시</a>
<a href="{root_link}/quality/judgment.json">의미 품질 판정 원본</a><a href="{root_link}/quality/labels.jsonl">480개 라벨과 판정 이유</a>
<a href="{root_link}/search/summary.json">검색 효과 집계</a><a href="{root_link}/search/results.jsonl">실행·사례별 검색 결과</a>
<a href="{root_link}/search/read-audit.json">입력 읽기 감사와 한계</a><a href="{root_link}/search/rejected-partial-read.json">채점 전 제외한 출력과 이유</a>
<a href="../../apps/codemap-search/experiments/related-terms-poc/README.related.ko.md">CLI 사용법</a><a href="../../.codemap/poc/related-terms/thesaurus-v1/verification-uncapped/verification.json">동작 검증 원본</a>
</div>
<details><summary>직접 관련어를 조회하는 명령</summary><p class="small">저장소 루트에서 실행합니다.</p><pre><code>{escape(cli)}</code></pre></details>
</div></section>
<footer>보고서 본문은 외부 스크립트·폰트·추적 요청 없이 파일 하나로 열 수 있습니다. 근거 링크는 이 저장소의 로컬 산출물을 가리킵니다. 기준·관측·해석·권장사항을 구분해 작성했습니다.</footer>
</main></body></html>"""
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(html)
    print(json.dumps({"html": str(OUTPUT), "effect_complete": complete, "overall": verdict}, ensure_ascii=False))


if __name__ == "__main__":
    main()
