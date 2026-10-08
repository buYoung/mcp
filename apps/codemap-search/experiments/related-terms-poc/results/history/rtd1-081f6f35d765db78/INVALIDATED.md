# 무효화: rtd1-081f6f35d765db78 (정규화 rtn-1)

- 무효화 시점: child 02 Stage 1, 평가(child 03) 시작 전.
- 원인: `normalize-check` 결과 133,942개 표현 중 1개에서 Python 빌더와 Rust 판독기의 `token_key`가 달랐다
  (`normalize-check.json`). Python `str.isalnum()`은 Unicode Other_Alphabetic 결합 부호(벵골·아삼 문자 모음 부호 등)를
  영숫자로 보지 않지만 Rust `char::is_alphanumeric()`은 영숫자로 본다. 표준 라이브러리만으로 두 구현을 일치시킬 수 없다.
- 조치: 정규화 규칙을 `rtn-2`로 올렸다. ASCII가 아닌 표현은 `token_key`를 비우고 손실 표시하여 정확 일치만 허용한다.
  새 데이터셋을 다시 빌드하고 인계 파일을 새로 만들었다.
- 이 디렉터리의 `dataset-handoff.json`, `dataset-stats.json`은 무효화된 빌드의 기록이며 이후 단계에서 쓰지 않는다.
