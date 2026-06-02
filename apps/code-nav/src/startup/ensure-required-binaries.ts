import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
    CTAGS_BINARY,
    CTAGS_VERSION_TIMEOUT_MS,
    ZOEKT_INDEX_BINARY,
    ZOEKT_WEBSERVER_BINARY,
} from "../config/defaults.js";
import { resolveExecutablePath } from "./binary-availability.js";

const execFileAsync = promisify(execFile);

export interface ResolvedBinaries {
    zoektIndexPath: string;
    zoektWebserverPath: string;
    ctagsPath: string;
}

interface MissingBinary {
    label: string;
    status: string;
}

/**
 * Verifies zoekt and Universal Ctags are installed before the server accepts
 * traffic. On any missing/incompatible binary it prints installation guidance to
 * stderr and exits — there is no fallback (DESIGN 결정 3).
 */
export async function ensureRequiredBinaries(): Promise<ResolvedBinaries> {
    const zoektIndexPath = await resolveExecutablePath(ZOEKT_INDEX_BINARY);
    const zoektWebserverPath = await resolveExecutablePath(ZOEKT_WEBSERVER_BINARY);
    const ctagsPath = await resolveExecutablePath(CTAGS_BINARY);
    const ctagsIsUniversal = ctagsPath != null && (await isUniversalCtags(ctagsPath));

    if (zoektIndexPath == null || zoektWebserverPath == null || ctagsPath == null || !ctagsIsUniversal) {
        const missingBinaries = collectMissingBinaries({
            zoektIndexPath,
            zoektWebserverPath,
            ctagsPath,
            ctagsIsUniversal,
        });
        process.stderr.write(buildInstallationGuidance(missingBinaries));
        process.exit(1);
    }

    return { zoektIndexPath, zoektWebserverPath, ctagsPath };
}

async function isUniversalCtags(ctagsPath: string): Promise<boolean> {
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

function buildInstallationGuidance(missingBinaries: MissingBinary[]): string {
    const missingLines = missingBinaries.map(({ label, status }) => `  - ${label}   상태: ${status}`).join("\n");
    return [
        "[code-nav] 필수 외부 바이너리가 누락되었습니다. 이 MCP는 폴백 없이 zoekt와 Universal Ctags를 모두 요구합니다.",
        "",
        "누락 항목:",
        missingLines,
        "",
        "설치 안내:",
        "  zoekt:",
        "    go install github.com/sourcegraph/zoekt/cmd/...@latest",
        "    (설치 후 zoekt-index, zoekt-webserver 바이너리가 PATH 또는 ~/go/bin 에 있어야 합니다.)",
        "  ctags (universal-ctags):",
        "    macOS:   brew install universal-ctags",
        "    Debian:  apt-get install universal-ctags",
        '    검증:    ctags --version  결과에 "Universal Ctags"가 포함되어야 합니다.',
        "",
        'PATH 에 ~/go/bin 이 없다면 추가하거나, 셸 설정에 export PATH="$PATH:$(go env GOPATH)/bin" 를 넣으세요.',
        "",
        "설치 후 MCP 서버를 다시 시작하세요.",
        "",
    ].join("\n");
}
