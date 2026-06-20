# 전체 transcript (raw transcripts)

## 이게 뭔가

`transcripts.tar.gz`는 **180개 episode 각각의 전체 실행 기록(`stdout.txt`)**을 압축한 것입니다. 각 `stdout.txt`는 모델이 과제를 푸는 동안 런타임(claude/codex/opencode)이 내보낸 **편집 안 된 JSONL 이벤트 스트림 전문**입니다. 안에는:

- 모델이 호출한 **모든 도구**(검색 쿼리, 읽은 파일, grep 패턴 등)와 그 **도구 결과 전문**(대상 코드베이스에서 실제 반환된 코드)
- 모델의 **추론 텍스트·중간 메시지**
- **최종 답변**
- 토큰 사용량 이벤트

즉 "이 모델이 실제로 무엇을 보고 무엇을 답했는지"의 가장 직접적인 증거입니다. 점수가 의심되면 여기서 episode 하나를 열어 직접 검증할 수 있습니다.

## 압축한 이유

원본은 약 37MB(180개)라 저장소가 비대해지므로 `tar.gz`(약 7.5MB)로 묶었습니다. 압축 증거(점수·도구호출 분포·명령·최종답변)는 `compact/`에 **압축 없이** 그대로 있으니, 빠른 확인은 `compact/`로, 전체 검증이 필요할 때만 아래로 풀어 보세요.

## 압축 해제

```bash
tar -xzf transcripts.tar.gz
# → transcripts/<arm_id>/<codebase>/round-N/stdout.txt
```

예시:
```bash
tar -xzf transcripts.tar.gz
cat transcripts/claude-sonnet-codemap/deno-main/round-1/stdout.txt | head
# 특정 도구 호출만 보기 (JSONL이라 라인 단위)
grep '"tool_use"' transcripts/claude-sonnet-serena/ClickHouse-master/round-1/stdout.txt
```

특정 episode만 추출:
```bash
tar -xzf transcripts.tar.gz transcripts/codex-gpt54-codemap/deno-main/round-1/stdout.txt
```

## 구조

```
transcripts/
  <arm_id>/                # 예: claude-sonnet-codemap, codex-gpt54-serena, opencode-mimo-no-mcp
    <codebase>/            # ClickHouse-master | deno-main | angular-main
      round-1/stdout.txt
      round-2/stdout.txt
      round-3/stdout.txt
```

## 스크럽 고지

홈 절대경로(`/Users/...`)만 `<REPO_ROOT>`/`<HOME>`으로 정규화했습니다. **모델의 검색·답변·도구 결과 내용은 일절 변경하지 않았습니다.** (프로젝트/패키지명 `buyong-mcp`처럼 경로가 아닌 식별자는 그대로 둡니다 — 공개 저장소 이름이므로.)

## 참고: compact/ 와의 관계

| 위치 | 내용 | 크기 |
|---|---|---|
| `compact/<arm>/<codebase>/round-N/` | `result_metrics.json`(도구분포·토큰·점수), `scorer_output.json`(judge fact별 채점), `raw_answer.txt`(최종답변), `exact_command.json`(실행명령), `tool_events.json` | episode당 ~14KB, 압축 안 함 |
| `transcripts.tar.gz` | 위 전체 원천인 `stdout.txt` 전문 | 합쳐 7.5MB |
