import { execFile } from "node:child_process";
import { delimiter } from "node:path";
import { promisify } from "node:util";
import {
    CTAGS_BINARY,
    CTAGS_RELEASE_BINARY,
    CTAGS_VERSION_TIMEOUT_MS,
    ZOEKT_INDEX_BINARY,
    ZOEKT_WEBSERVER_BINARY,
} from "../config/defaults.js";
import { resolveExecutablePath } from "./binary-availability.js";
import { resolveManagedBinDirectory } from "./managed-bin-storage.js";

const execFileAsync = promisify(execFile);

export interface ResolvedBinaries {
    zoektIndexPath: string;
    zoektWebserverPath: string;
    ctagsPath: string;
}

export interface MissingBinary {
    label: string;
    status: string;
}

export type BinaryResolution =
    | { status: "ready"; binaries: ResolvedBinaries }
    | { status: "missing"; missing: MissingBinary[] };

/**
 * zoekt와 Universal Ctags가 모두 갖춰졌는지 점검한다. 과거에는 누락 시 즉시
 * `process.exit(1)` 했으나, 이제는 종료하지 않고 결과만 돌려준다 — 호출부가
 * "저하 모드"로 부팅해 `install_binaries` 도구로 받게 한다(조용한 폴백이 아니라,
 * 검색 도구가 명시적으로 누락을 보고). 완전 결손이면 안내 텍스트를 출력하되 종료는
 * 호출부 판단에 맡긴다(DESIGN §5).
 */
export async function resolveBinaries(): Promise<BinaryResolution> {
    const zoektIndexPath = await resolveExecutablePath(ZOEKT_INDEX_BINARY);
    const zoektWebserverPath = await resolveExecutablePath(ZOEKT_WEBSERVER_BINARY);
    const ctagsPath = await resolveCtagsExecutablePath();
    const ctagsIsUniversal = ctagsPath != null && (await isUniversalCtags(ctagsPath));

    if (zoektIndexPath == null || zoektWebserverPath == null || ctagsPath == null || !ctagsIsUniversal) {
        return {
            status: "missing",
            missing: collectMissingBinaries({ zoektIndexPath, zoektWebserverPath, ctagsPath, ctagsIsUniversal }),
        };
    }

    return { status: "ready", binaries: { zoektIndexPath, zoektWebserverPath, ctagsPath } };
}

/**
 * 관리형 bin 디렉터리를 자식 프로세스의 PATH 앞에 붙인다. `zoekt-index`가 색인 시
 * ctags를 내부 호출하는데(DESIGN §6.1), 받은 ctags가 PATH에 없으면 못 찾는다.
 * 부모 `process.env.PATH`를 바꾸면 inherit 하는 zoekt-index/webserver 모두에 적용된다.
 */
export function prependManagedBinToPath(): void {
    const managedBinDirectory = resolveManagedBinDirectory();
    const currentPath = process.env.PATH ?? "";
    const segments = currentPath.split(delimiter).filter((segment) => segment.length > 0);
    if (segments.includes(managedBinDirectory)) {
        return;
    }
    process.env.PATH =
        currentPath.length > 0 ? `${managedBinDirectory}${delimiter}${currentPath}` : managedBinDirectory;
}

/** `ctags` 우선, 없으면 릴리스 원래 이름 `universal-ctags`도 시도한다. */
async function resolveCtagsExecutablePath(): Promise<string | undefined> {
    return (await resolveExecutablePath(CTAGS_BINARY)) ?? (await resolveExecutablePath(CTAGS_RELEASE_BINARY));
}

/**
 * 주어진 ctags 경로가 Universal Ctags 변형인지 검증한다(`ctags --version` 출력에
 * "Universal Ctags" 포함). BSD/Exuberant ctags는 `--output-format=json`을 지원하지
 * 않아 SymbolProvider에서 런타임 실패하므로, 폴백 탐색으로 찾은 경로도 이 함수로
 * 게이트해야 한다(DESIGN §3.2). 부팅 점검과 lookup_symbol 폴백이 공유한다.
 */
export async function isUniversalCtags(ctagsPath: string): Promise<boolean> {
    try {
        const { stdout, stderr } = await execFileAsync(ctagsPath, ["--version"], {
            timeout: CTAGS_VERSION_TIMEOUT_MS,
        });
        return `${stdout}${stderr}`.includes("Universal Ctags");
    } catch {
        return false;
    }
}

function collectMissingBinaries(state: {
    zoektIndexPath: string | undefined;
    zoektWebserverPath: string | undefined;
    ctagsPath: string | undefined;
    ctagsIsUniversal: boolean;
}): MissingBinary[] {
    const missingBinaries: MissingBinary[] = [];
    if (state.zoektIndexPath == null) {
        missingBinaries.push({ label: "zoekt-index (텍스트 검색 색인기)", status: "미설치" });
    }
    if (state.zoektWebserverPath == null) {
        missingBinaries.push({ label: "zoekt-webserver (텍스트 검색 질의 서버)", status: "미설치" });
    }
    if (state.ctagsPath == null) {
        missingBinaries.push({ label: "ctags (Universal Ctags, 심볼 색인)", status: "미설치" });
    } else if (!state.ctagsIsUniversal) {
        missingBinaries.push({
            label: "ctags (Universal Ctags, 심볼 색인)",
            status: "설치됨이나 Universal 변형 아님",
        });
    }
    return missingBinaries;
}

/**
 * 누락 항목 + 설치 경로 안내를 만든다. 1순위는 자동 설치 도구(`install_binaries`),
 * 2순위는 수동 설치다. stderr 출력과 `search_text` 저하 응답, `install_binaries`
 * 실패 응답에서 공통으로 쓴다.
 */
export function buildInstallationGuidance(missingBinaries: MissingBinary[]): string {
    const missingLines = missingBinaries.map(({ label, status }) => `  - ${label}   상태: ${status}`).join("\n");
    return [
        "[scout] 필수 외부 바이너리가 누락되었습니다. 이 MCP는 폴백 없이 zoekt와 Universal Ctags를 모두 요구합니다.",
        "",
        "누락 항목:",
        missingLines,
        "",
        "해결 방법:",
        "  1) (권장) install_binaries 도구를 호출하면 사전 빌드된 바이너리를 자동으로 내려받습니다.",
        "     - 사용자에게 다운로드 동의를 먼저 구한 뒤 호출하세요. SHA-256으로 무결성을 검증합니다.",
        "  2) 수동 설치:",
        "     zoekt:",
        "       go install github.com/sourcegraph/zoekt/cmd/...@latest",
        "       (설치 후 zoekt-index, zoekt-webserver 가 PATH 또는 ~/go/bin 에 있어야 합니다.)",
        "     ctags (universal-ctags):",
        "       macOS:   brew install universal-ctags",
        "       Debian:  apt-get install universal-ctags",
        '       검증:    ctags --version  결과에 "Universal Ctags"가 포함되어야 합니다.',
        "",
        'PATH 에 ~/go/bin 이 없다면 추가하거나, 셸 설정에 export PATH="$PATH:$(go env GOPATH)/bin" 를 넣으세요.',
        "",
    ].join("\n");
}
