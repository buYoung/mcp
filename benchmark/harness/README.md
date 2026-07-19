# 검색 품질 벤치마크 하네스

이 하네스는 같은 14개 질문으로 B1과 B2를 각각 3회 실행하고, 도구 호출·토큰·최초 실패 지점·정답 사용을 분리해 기록하도록 만든 실행 코드다.

## 저장소에 포함된 것

- `config/`: OpenCode B1·B2 설정과 실행 제한
- `scripts/`: 격리, 실행, 복구, 기록, 채점, 집계 코드
- `schemas/`: 결과와 판정 JSON 계약
- `templates/`: 질문 및 macOS 격리 템플릿
- `provenance/`: B2 기준 실행 파일을 만들 때 사용한 패치와 측정 식별 정보

## 별도로 준비할 것

실제 실행에는 저장소에 커밋하지 않은 다음 파일이 필요하다.

```text
benchmark/runtime/opencode
benchmark/b2/source/
benchmark/b2/target/release/codemap-search
benchmark/corpus/directus-index-golden/.codemap/
```

`config/b2-runtime.json`의 실행 파일 경로와 `provenance/`의 고정 해시가 실제 준비 파일과 일치해야 한다. 인증 파일은 사용자의 OpenCode 자료 폴더에서 실행할 때만 읽으며 저장소에 복사하지 않는다.

## 모델 호출 없는 확인

저장소에 포함된 채점·분석 코드는 저장소 루트에서 다음처럼 확인한다.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/scoring_selftest.py
PYTHONDONTWRITEBYTECODE=1 python3 benchmark/harness/scripts/analysis_tools_selftest.py
```

전체 실행 자체검사인 `selftest.py`는 위의 B2 소스, 동일 환경에서 만든 2회 빌드 증거와 실행 파일까지 검증한다. 이 자료를 준비하지 않은 소스 저장소만으로는 통과하지 않는다.

외부 모델 실행은 별도 런타임 입력을 준비한 뒤 `preflight.py`의 모든 필수 항목이 통과한 경우에만 허용한다.
