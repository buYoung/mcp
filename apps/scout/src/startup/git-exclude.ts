import { execFile } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { promisify } from "node:util";
import { SCOUT_DIRECTORY_NAME } from "../config/defaults.js";

const execFileAsync = promisify(execFile);

/**
 * `.git/info/exclude`에 등록할 항목(`.scout/`). 디렉터리 형태로 적어 레포 안의
 * scout 산출물 전체가 git에 잡히지 않도록 한다.
 */
const SCOUT_EXCLUDE_ENTRY = `${SCOUT_DIRECTORY_NAME}/`;

/**
 * exclude 파일 끝에 덧붙일 블록. 사람이 봤을 때 자동 추가분임을 알 수 있게
 * 한국어 주석을 함께 남긴다.
 */
const SCOUT_EXCLUDE_BLOCK = `\n# scout 도구 산출물(자동 추가)\n${SCOUT_EXCLUDE_ENTRY}\n`;

/**
 * `<repo>/.scout/`를 해당 레포의 `.git/info/exclude`에 등록해 git 추적에서 숨긴다.
 *
 * 설계 결정(SPEC §5, never-exit 철학):
 * - git 저장소가 아니거나 git이 설치돼 있지 않으면 **조용히** 반환한다(throw·exit 금지).
 * - 멱등성: 이미 `.scout/`가 등록돼 있으면 아무 것도 하지 않는다.
 * - 모든 파일/프로세스 오류는 catch해 stderr 경고만 남기고 반환한다.
 *   exclude 등록은 부가 기능이므로 실패해도 부팅을 막지 않는다.
 */
export async function registerScoutInGitExclude(repositoryRoot: string): Promise<void> {
    // 실제 exclude 파일 경로를 git에게 직접 물어 해석한다. worktree·서브모듈 등
    // 비표준 레이아웃에서도 git이 알려주는 경로가 정답이기 때문이다.
    let excludeFilePath: string;
    try {
        const { stdout } = await execFileAsync("git", ["rev-parse", "--git-path", "info/exclude"], {
            cwd: repositoryRoot,
        });
        const reportedPath = stdout.trim();
        if (reportedPath.length === 0) {
            return;
        }
        // git이 상대경로를 줄 수 있으므로 repositoryRoot 기준으로 절대화한다.
        excludeFilePath = isAbsolute(reportedPath) ? reportedPath : resolve(repositoryRoot, reportedPath);
    } catch {
        // 비-git 디렉터리이거나 git 미설치 — 조용히 반환한다.
        return;
    }

    try {
        // 파일이 없으면(ENOENT) 빈 내용으로 간주하고 새로 만든다.
        let existingContent = "";
        try {
            existingContent = await readFile(excludeFilePath, "utf8");
        } catch (error) {
            if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
                throw error;
            }
        }

        // 멱등 체크: 앞/뒤 슬래시를 정규화해 `.scout`·`.scout/`·`/.scout/` 등 동등 표기를
        // 모두 같은 것으로 본다. 자동 추가분이든 사용자 수기 항목이든 이미 등록돼 있으면 no-op.
        const normalizeEntry = (value: string): string => value.trim().replace(/^\/+/, "").replace(/\/+$/, "");
        const targetEntry = normalizeEntry(SCOUT_EXCLUDE_ENTRY);
        const alreadyRegistered = existingContent.split(/\r?\n/).some((line) => normalizeEntry(line) === targetEntry);
        if (alreadyRegistered) {
            return;
        }

        // 기존 내용이 개행으로 끝나지 않으면 한 줄 띄워 블록을 덧붙인다.
        const separator = existingContent.length > 0 && !existingContent.endsWith("\n") ? "\n" : "";
        await writeFile(excludeFilePath, `${existingContent}${separator}${SCOUT_EXCLUDE_BLOCK}`, "utf8");
    } catch (error) {
        // fs 오류는 부팅을 막지 않도록 경고만 남기고 흡수한다.
        const message = error instanceof Error ? error.message : String(error);
        process.stderr.write(`[scout] .git/info/exclude 등록 실패: ${excludeFilePath}: ${message} — 무시\n`);
    }
}
