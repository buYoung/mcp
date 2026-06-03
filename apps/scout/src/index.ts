#!/usr/bin/env node
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { SERVER_NAME, SERVER_VERSION } from "./config/defaults.js";
import { readGitignoreDirectoryNames } from "./config/gitignore-excludes.js";
import { loadScoutConfig, type ResolvedScoutConfig } from "./config/scout-config.js";
import { TextSearchProvider } from "./providers/text-search/text-search-provider.js";
import { installManagedBinaries } from "./startup/binary-installer.js";
import {
    buildInstallationGuidance,
    prependManagedBinToPath,
    type ResolvedBinaries,
    resolveBinaries,
} from "./startup/ensure-required-binaries.js";
import { registerScoutInGitExclude } from "./startup/git-exclude.js";
import { registerTools, type SearchProviderResolution } from "./tools/index.js";

async function main(): Promise<void> {
    const repositoryRoot = process.cwd();

    // 설정 로드 직후 gitignore 디렉터리-이름을 union 한다(replace 아님 — 별개 소스).
    // provider/lifecycle를 동기·단순하게 유지하기 위해, gitignore union은 여기서 끝낸다.
    const loadedConfig = await loadScoutConfig(repositoryRoot);
    const gitignoreNames = loadedConfig.index.respectGitignore ? await readGitignoreDirectoryNames(repositoryRoot) : [];
    const effectiveExcluded = [...new Set([...loadedConfig.index.excludedDirectories, ...gitignoreNames])];
    // excludedDirectories를 effectiveExcluded로 치환한 불변 복사본을 만들어 provider에 넘긴다.
    const config: ResolvedScoutConfig = {
        ...loadedConfig,
        index: {
            ...loadedConfig.index,
            excludedDirectories: effectiveExcluded,
        },
    };

    // .scout/ 산출물을 .git/info/exclude에 등록해 숨긴다(전역 설정으로만 토글, default on).
    if (config.index.registerGitExclude) {
        await registerScoutInGitExclude(repositoryRoot);
    }

    let textSearchProvider: TextSearchProvider | null = null;
    let installInFlight: Promise<string> | null = null;

    const buildProviderFrom = (binaries: ResolvedBinaries): void => {
        const previous = textSearchProvider;
        // 받은(또는 발견된) bin 디렉터리를 PATH 앞에 붙여, zoekt-index가 색인 시
        // 내부 호출하는 ctags도 자식 프로세스가 찾을 수 있게 한다.
        prependManagedBinToPath();
        textSearchProvider = new TextSearchProvider({
            zoektIndexPath: binaries.zoektIndexPath,
            zoektWebserverPath: binaries.zoektWebserverPath,
            repositoryRoot,
            // config를 클로저로 캡처해 provider에 전달(effectiveExcluded 반영본).
            config,
        });
        // 재구성 시 이전 provider의 webserver 자식을 반드시 정리한다(고아 프로세스 방지).
        previous?.shutdown();
    };

    // 부팅 시 1회 해석. 누락이어도 종료하지 않고(저하 모드) 안내만 stderr에 남긴 뒤,
    // install_binaries 도구 호출로 복구할 수 있게 한다. 조용한 폴백이 아니라,
    // search_text가 호출되면 명시적으로 누락을 보고한다(DESIGN §5).
    const initialResolution = await resolveBinaries();
    if (initialResolution.status === "ready") {
        buildProviderFrom(initialResolution.binaries);
    } else {
        process.stderr.write(buildInstallationGuidance(initialResolution.missing));
    }

    // search_text 한 호출당 단 한 번 해석한다(저하 시 중복 ctags --version 점검 방지).
    const resolveSearchProvider = async (): Promise<SearchProviderResolution> => {
        // 설치 진행 중이면 끝날 때까지 대기한다 — 관리형 dir 교체와 검색의 자식 spawn이
        // 겹쳐 ENOENT/부분 파일 실행이 나는 것을 막는다.
        if (installInFlight != null) {
            await installInFlight.catch(() => undefined);
        }
        if (textSearchProvider != null) {
            return { kind: "ready", provider: textSearchProvider };
        }
        // 런타임 중 (수동 설치 등으로) 바이너리가 생겼을 수 있으니 다시 해석한다.
        const resolution = await resolveBinaries();
        if (resolution.status === "missing") {
            return { kind: "missing", guidance: buildInstallationGuidance(resolution.missing) };
        }
        buildProviderFrom(resolution.binaries);
        return textSearchProvider != null
            ? { kind: "ready", provider: textSearchProvider }
            : { kind: "missing", guidance: buildInstallationGuidance([]) };
    };

    const runInstall = async (): Promise<string> => {
        // 관리형 dir을 비우기 전에 그 안의 바이너리를 쓰는 provider/webserver를 먼저 정리한다
        // (실행 중 파일 삭제·교체 충돌 방지; Windows는 실행 중 .exe 삭제가 실패한다).
        const previous = textSearchProvider;
        textSearchProvider = null;
        previous?.shutdown();

        const outcome = await installManagedBinaries();
        if (outcome.status === "unsupported-platform") {
            return [
                `[scout] 자동 설치를 지원하지 않는 플랫폼입니다: ${outcome.platform}/${outcome.architecture}.`,
                "사전 빌드 자산이 없어 수동 설치가 필요합니다.",
                "",
                buildInstallationGuidance([{ label: "zoekt + Universal Ctags", status: "자동 설치 미지원 플랫폼" }]),
            ].join("\n");
        }
        if (outcome.status === "failed") {
            return [
                `[scout] 바이너리 설치에 실패했습니다: ${outcome.message}`,
                "",
                "수동 설치로 진행할 수 있습니다:",
                buildInstallationGuidance([{ label: "zoekt + Universal Ctags", status: "다운로드 실패" }]),
            ].join("\n");
        }

        const resolution = await resolveBinaries();
        if (resolution.status === "ready") {
            buildProviderFrom(resolution.binaries);
            return [
                `[scout] 바이너리 설치 완료 (릴리스 ${outcome.tag}).`,
                `  위치: ${outcome.directory}`,
                `  설치됨: ${outcome.binaries.join(", ")}`,
                "이제 search_text 를 사용할 수 있습니다.",
            ].join("\n");
        }
        return [
            `[scout] 바이너리를 내려받았으나(릴리스 ${outcome.tag}) 검증을 통과하지 못했습니다.`,
            "",
            buildInstallationGuidance(resolution.missing),
        ].join("\n");
    };

    const installBinaries = async (): Promise<string> => {
        // 동시 호출은 단일 설치로 합친다(coalesce).
        if (installInFlight != null) {
            return installInFlight;
        }
        installInFlight = runInstall();
        try {
            return await installInFlight;
        } finally {
            installInFlight = null;
        }
    };

    let alreadyShutDown = false;
    const shutdown = (): void => {
        if (alreadyShutDown) {
            return;
        }
        alreadyShutDown = true;
        textSearchProvider?.shutdown();
    };
    const shutdownAndExit = (exitCode: number): void => {
        shutdown();
        process.exit(exitCode);
    };
    process.on("SIGINT", () => shutdownAndExit(0));
    process.on("SIGTERM", () => shutdownAndExit(0));
    process.on("exit", shutdown);

    // 고수준 McpServer 사용: registerTool이 capabilities를 추론하므로 명시 불필요.
    const server = new McpServer({
        name: SERVER_NAME,
        version: SERVER_VERSION,
    });

    registerTools(server, { resolveSearchProvider, installBinaries });

    // A client that closes stdin (instead of signalling) must still shut the
    // webserver child down — otherwise the live child keeps the event loop alive
    // and the process leaks an orphaned zoekt-webserver. StdioServerTransport only
    // listens for stdin "data"/"error" (never "end"/"close"), so its onclose does
    // not fire on EOF — we listen for stdin end/close ourselves.
    process.stdin.on("end", () => shutdownAndExit(0));
    process.stdin.on("close", () => shutdownAndExit(0));

    const transport = new StdioServerTransport();
    transport.onclose = () => shutdownAndExit(0);
    await server.connect(transport);
}

main().catch((error) => {
    console.error("[scout] fatal:", error);
    process.exit(1);
});
