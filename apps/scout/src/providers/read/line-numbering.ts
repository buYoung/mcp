/**
 * read_file의 줄 번호 직렬화. Claude Code Read의 compact 포맷을 모사한다(DESIGN §4.1):
 * 각 줄을 `<번호><TAB><내용>`으로 만들어 `\n`으로 잇는다. 레거시 `padStart(6) + "→"`
 * 포맷은 미구현(GrowthBook 게이팅 제거).
 */

/**
 * 본문 줄 배열을 compact 줄번호 텍스트로 직렬화한다.
 *
 * @param lines 출력할 본문 줄(이미 offset/limit로 슬라이스된 상태).
 * @param startLineNumber 첫 줄에 붙일 번호(raw offset 기준, 1-기반).
 * @returns `<번호><TAB><내용>`을 `\n`으로 이은 문자열.
 */
export function formatLinesWithNumbers(lines: readonly string[], startLineNumber: number): string {
    return lines.map((line, index) => `${startLineNumber + index}\t${line}`).join("\n");
}
