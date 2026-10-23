"""저장소별 개념 단위 판정 묶음과 지침을 만든다(gin에서 쓴 u1~u6과 같은 방식).

사용법: python3 prepare_units.py <저장소 이름> [묶음당 항목 수, 기본 486]
입력: <저장소>/unit-input.json, <저장소>/summary.json, unit-brief-repo.md
출력: <작업 폴더>/batches/units-input/<저장소>/uNN.json, <작업 폴더>/batches/briefs/units-<저장소>-uNN.md
판정 결과는 <작업 폴더>/batches/units/<저장소>/uNN.json에 쓰게 한다.
"""
import json
import sys
from pathlib import Path
from word_eval_paths import CHECKOUTS, SCRIPTS, batches_dir, work_dir

WORK = work_dir()
BATCHES = batches_dir()
BATCH_SIZE = 486
DESCRIPTIONS = {
    "express": "Node.js 웹 프레임워크",
    "flask": "Python 웹 프레임워크",
    "redis": "인메모리 데이터 저장소",
    "tokio": "Rust 비동기 런타임",
    "terminal": "Windows 터미널과 콘솔 호스트",
    "vector": "관측 데이터(로그·지표) 수집·변환 파이프라인",
    "bitwarden-server": "비밀번호 관리자 서버",
    "gutenberg": "WordPress 블록 편집기",
    "keycloak": "인증·접근 관리 서버",
    "woocommerce": "WordPress 전자상거래 플러그인",
    "sentry": "오류 추적·성능 모니터링 서비스",
}
LANGUAGE_NAMES = {"go": "Go", "python": "Python", "javascript": "JavaScript", "typescript": "TypeScript", "tsx": "TSX",
                  "java": "Java", "csharp": "C#", "c": "C", "cpp": "C++", "rust": "Rust", "php": "PHP", "ruby": "Ruby"}


def brief_for(repository):
    summary = json.loads((WORK / repository / "summary.json").read_text())
    languages = ", ".join(LANGUAGE_NAMES[language] for language, _ in
                          sorted(summary["languages"].items(), key=lambda item: -item[1]))
    # 지침 본문에 JSON 예시의 중괄호가 있어 format 대신 자리표시만 바꾼다.
    return ((SCRIPTS / "unit-brief-repo.md").read_text().replace("{repo}", repository)
            .replace("{description}", DESCRIPTIONS[repository]).replace("{languages}", languages)
            .replace("{checkouts}", str(CHECKOUTS)))


def main(repository, batch_size=BATCH_SIZE):
    batch_size = int(batch_size)
    items = json.loads((WORK / repository / "unit-input.json").read_text())
    input_directory = Path(f"{BATCHES}/units-input/{repository}")
    input_directory.mkdir(parents=True, exist_ok=True)
    Path(f"{BATCHES}/units/{repository}").mkdir(parents=True, exist_ok=True)
    brief = brief_for(repository)
    batch_names = []
    for start in range(0, len(items), batch_size):
        name = f"u{start // batch_size + 1:02d}"
        (input_directory / f"{name}.json").write_text(json.dumps(items[start:start + batch_size], ensure_ascii=False, indent=1))
        Path(f"{BATCHES}/briefs/units-{repository}-{name}.md").write_text(
            brief + f"\n## 배정\n입력 파일: `{input_directory / name}.json`\n결과 파일: `{BATCHES}/units/{repository}/{name}.json`\n")
        batch_names.append(name)
    print(repository, len(items), "항목", len(batch_names), "묶음")


if __name__ == "__main__":
    main(*sys.argv[1:3])
