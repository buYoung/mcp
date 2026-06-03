import { readFile } from "node:fs/promises";
import { join } from "node:path";

/**
 * glob 메타 문자 집합. 이 중 하나라도 포함된 라인은 디렉터리-이름 수준 항목이
 * 아니라 패턴 매칭이므로 제외 집합 추출 대상에서 제외한다.
 */
const GLOB_META_CHARACTERS = ["*", "?", "[", "]"];

/**
 * `<repo>/.gitignore`에서 "디렉터리-이름 수준" 항목만 추출한다.
 *
 * 추출 규칙(SPEC §5):
 * - trim 후 빈 줄과 `#` 주석 라인은 skip.
 * - 부정(`!`)·slash 포함(`a/b`, `/x`)·glob 메타(`* ? [ ]`) 포함 라인은 skip.
 *   이런 라인은 단순 디렉터리 이름이 아니라 경로/패턴이므로 zoekt 제외 집합에
 *   안전하게 매핑할 수 없다.
 * - 남은 라인이 `name` 또는 `name/` 형태이면 끝의 `/`를 제거하고 수집.
 * - 중복은 제거한다. 반환 목록은 호출부에서 excluded 집합에 union 한다(별개 소스이므로
 *   replace 가 아님).
 *
 * 파일이 없거나 읽기 오류가 발생해도 절대 throw 하지 않고 `[]`를 반환한다
 * (scout 의 never-exit 철학).
 */
export async function readGitignoreDirectoryNames(repositoryRoot: string): Promise<string[]> {
    let content: string;

    try {
        content = await readFile(join(repositoryRoot, ".gitignore"), "utf8");
    } catch {
        // 파일 없음·권한 오류 등 모든 경우 빈 목록으로 처리한다.
        return [];
    }

    const collected = new Set<string>();

    for (const rawLine of content.split(/\r?\n/)) {
        const line = rawLine.trim();

        // 빈 줄·주석 skip.
        if (line.length === 0 || line.startsWith("#")) {
            continue;
        }

        // 부정 패턴은 "제외 해제"이므로 디렉터리 제외 집합에 합칠 수 없다.
        if (line.startsWith("!")) {
            continue;
        }

        // glob 메타가 포함되면 단순 이름이 아니므로 skip.
        if (GLOB_META_CHARACTERS.some((meta) => line.includes(meta))) {
            continue;
        }

        // 끝의 디렉터리 표시 `/` 하나만 떼어 낸 형태가 name 인지 확인한다.
        const withoutTrailingSlash = line.endsWith("/") ? line.slice(0, -1) : line;

        // 경로 구분자가 남아 있으면(`a/b`, `/x`, Windows식 `a\b`) 디렉터리-이름 수준이
        // 아니므로 skip. `.`/`..` 같은 비실용 이름도 디렉터리 제외 집합에 넣지 않는다.
        if (
            withoutTrailingSlash.length === 0 ||
            withoutTrailingSlash.includes("/") ||
            withoutTrailingSlash.includes("\\") ||
            withoutTrailingSlash === "." ||
            withoutTrailingSlash === ".."
        ) {
            continue;
        }

        collected.add(withoutTrailingSlash);
    }

    return [...collected];
}
