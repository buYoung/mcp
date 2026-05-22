import { type NormalizedToolCall, normalizeToolCall } from "./tool-call-extraction.js";

export type LayerZeroCategory =
    | "destructive_fs"
    | "destructive_git"
    | "system_modification"
    | "credential_changes"
    | "shell_evasion"
    | "exfiltration"
    | "self_privilege_escalation";

export interface LayerZeroHit {
    category: LayerZeroCategory;
    code: string;
    pattern: string;
    matchedToken?: string;
}

export function layerZeroCheck(input: NormalizedToolCall): LayerZeroHit | null {
    if (input.argv.length === 0) {
        return checkPathOnlyHits(input);
    }
    return (
        checkDestructiveFs(input) ??
        checkDestructiveGit(input) ??
        checkSystemModification(input) ??
        checkCredentialChanges(input) ??
        checkShellEvasion(input) ??
        checkExfiltration(input) ??
        checkPathOnlyHits(input)
    );
}

export function layerZeroCheckFromRawInput(
    rawInput: unknown,
    locations?: ReadonlyArray<{ path?: unknown }> | null,
): LayerZeroHit | null {
    return layerZeroCheck(normalizeToolCall(rawInput, locations));
}

function programName(argv: readonly string[]): string {
    const first = argv[0];
    if (first == null) {
        return "";
    }
    const separatorIndex = Math.max(first.lastIndexOf("/"), first.lastIndexOf("\\"));
    return separatorIndex === -1 ? first : first.slice(separatorIndex + 1);
}

function checkDestructiveFs(input: NormalizedToolCall): LayerZeroHit | null {
    const program = programName(input.argv);
    const rest = input.argv.slice(1);

    if (program === "rm" && hasRecursiveForceFlag(rest)) {
        const target = findRmTarget(rest);
        if (target && isCatastrophicRmTarget(target)) {
            return {
                category: "destructive_fs",
                code: "DESTRUCTIVE_FS_RM_RF_ROOT",
                pattern: "rm -rf <root/home>",
                matchedToken: target,
            };
        }
    }
    if (program === "find") {
        if (rest.includes("-delete")) {
            return { category: "destructive_fs", code: "DESTRUCTIVE_FS_FIND_DELETE", pattern: "find ... -delete" };
        }
        const execIndex = rest.findIndex((token) => token === "-exec" || token === "-execdir");
        if (execIndex !== -1 && rest[execIndex + 1] === "rm") {
            return { category: "destructive_fs", code: "DESTRUCTIVE_FS_FIND_EXEC_RM", pattern: "find ... -exec rm" };
        }
    }
    if (program === "dd") {
        if (rest.some((token) => token.startsWith("of=/dev/"))) {
            return { category: "destructive_fs", code: "DESTRUCTIVE_FS_DD_DEVICE", pattern: "dd of=/dev/..." };
        }
    }
    if (program === "mkfs" || program.startsWith("mkfs.") || program === "fdisk") {
        return { category: "destructive_fs", code: "DESTRUCTIVE_FS_FORMAT", pattern: program };
    }
    if (program === "diskutil" && rest[0] === "eraseDisk") {
        return { category: "destructive_fs", code: "DESTRUCTIVE_FS_FORMAT", pattern: "diskutil eraseDisk" };
    }
    if (program === "shred" || program === "wipe" || program === "srm") {
        return { category: "destructive_fs", code: "DESTRUCTIVE_FS_SECURE_DELETE", pattern: program };
    }
    return null;
}

function checkDestructiveGit(input: NormalizedToolCall): LayerZeroHit | null {
    if (programName(input.argv) !== "git") {
        return null;
    }
    const argv = input.argv;
    const tail = argv.slice(1);
    const subcommand = tail.find((token) => !token.startsWith("-"));
    if (tail.includes("--no-verify")) {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_NO_VERIFY", pattern: "git ... --no-verify" };
    }
    if (subcommand === "push") {
        if (
            tail.includes("--force") ||
            tail.includes("-f") ||
            tail.some((token) => token.startsWith("--force-with-lease"))
        ) {
            return { category: "destructive_git", code: "DESTRUCTIVE_GIT_PUSH_FORCE", pattern: "git push --force" };
        }
        if (tail.includes("--mirror")) {
            return { category: "destructive_git", code: "DESTRUCTIVE_GIT_PUSH_MIRROR", pattern: "git push --mirror" };
        }
        if (tail.includes("--delete")) {
            return { category: "destructive_git", code: "DESTRUCTIVE_GIT_PUSH_DELETE", pattern: "git push --delete" };
        }
        if (tail.some((token) => token.startsWith(":") && token.length > 1)) {
            return {
                category: "destructive_git",
                code: "DESTRUCTIVE_GIT_PUSH_DELETE_REFSPEC",
                pattern: "git push origin :branch",
            };
        }
    }
    if (subcommand === "reset" && tail.includes("--hard")) {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_RESET_HARD", pattern: "git reset --hard" };
    }
    if (subcommand === "update-ref" && tail.includes("-d")) {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_UPDATE_REF_DELETE", pattern: "git update-ref -d" };
    }
    if (subcommand === "reflog" && tail.includes("expire")) {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_REFLOG_EXPIRE", pattern: "git reflog expire" };
    }
    if (subcommand === "gc" && tail.some((token) => token === "--prune=now" || token === "--prune=all")) {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_GC_PRUNE", pattern: "git gc --prune=now" };
    }
    if (subcommand === "filter-branch" || subcommand === "filter-repo") {
        return { category: "destructive_git", code: "DESTRUCTIVE_GIT_FILTER", pattern: `git ${subcommand}` };
    }
    return null;
}

function checkSystemModification(input: NormalizedToolCall): LayerZeroHit | null {
    const program = programName(input.argv);
    const tail = input.argv.slice(1);
    if (program === "sudo" || program === "su" || program === "doas") {
        return { category: "system_modification", code: "SYSTEM_PRIVILEGE_ESCALATION", pattern: program };
    }
    if (program === "chown") {
        return { category: "system_modification", code: "SYSTEM_CHOWN", pattern: "chown" };
    }
    if (["apt", "apt-get", "yum", "dnf", "pacman"].includes(program)) {
        return { category: "system_modification", code: "SYSTEM_PACKAGE_MANAGER", pattern: program };
    }
    if (program === "brew" && tail[0] === "install") {
        return { category: "system_modification", code: "SYSTEM_PACKAGE_MANAGER", pattern: "brew install" };
    }
    if ((program === "npm" || program === "pnpm" || program === "yarn") && hasGlobalInstall(program, tail)) {
        return {
            category: "system_modification",
            code: "SYSTEM_NPM_GLOBAL_INSTALL",
            pattern: `${program} global install`,
        };
    }
    if (program === "pip" && tail[0] === "install" && tail.includes("--user")) {
        return { category: "system_modification", code: "SYSTEM_PIP_USER", pattern: "pip install --user" };
    }
    if (program === "pipx" && tail[0] === "install") {
        return { category: "system_modification", code: "SYSTEM_PIPX_INSTALL", pattern: "pipx install" };
    }
    if (program === "uv" && tail[0] === "tool" && tail[1] === "install") {
        return { category: "system_modification", code: "SYSTEM_UV_TOOL_INSTALL", pattern: "uv tool install" };
    }
    if (program === "cargo" && tail[0] === "install") {
        return { category: "system_modification", code: "SYSTEM_CARGO_INSTALL", pattern: "cargo install" };
    }
    if (program === "go" && tail[0] === "install" && tail.slice(1).some((token) => token.includes("@"))) {
        return { category: "system_modification", code: "SYSTEM_GO_INSTALL", pattern: "go install pkg@" };
    }
    return null;
}

function checkCredentialChanges(input: NormalizedToolCall): LayerZeroHit | null {
    const program = programName(input.argv);
    const tail = input.argv.slice(1);
    if (program === "git" && tail[0] === "config" && tail.includes("--global")) {
        return { category: "credential_changes", code: "CREDENTIAL_GIT_CONFIG_GLOBAL", pattern: "git config --global" };
    }
    if (program === "ssh-keygen") {
        return { category: "credential_changes", code: "CREDENTIAL_SSH_KEYGEN", pattern: "ssh-keygen" };
    }
    if (program === "gh" && tail[0] === "auth" && (tail[1] === "login" || tail[1] === "logout")) {
        return { category: "credential_changes", code: "CREDENTIAL_GH_AUTH", pattern: `gh auth ${tail[1]}` };
    }
    if (program === "aws" && tail[0] === "configure") {
        return { category: "credential_changes", code: "CREDENTIAL_AWS_CONFIGURE", pattern: "aws configure" };
    }
    if (program === "gcloud" && tail[0] === "auth") {
        return { category: "credential_changes", code: "CREDENTIAL_GCLOUD_AUTH", pattern: "gcloud auth" };
    }
    if (program === "docker" && tail[0] === "login") {
        return { category: "credential_changes", code: "CREDENTIAL_DOCKER_LOGIN", pattern: "docker login" };
    }
    return null;
}

function checkShellEvasion(input: NormalizedToolCall): LayerZeroHit | null {
    const program = programName(input.argv);
    const tail = input.argv.slice(1);
    if (program === "eval") {
        return { category: "shell_evasion", code: "SHELL_EVAL", pattern: "eval" };
    }
    if (INTERPRETER_PROGRAMS.has(program)) {
        const flagIndex = tail.findIndex((token) => token === "-c" || token === "-e");
        const script = flagIndex !== -1 ? tail[flagIndex + 1] : undefined;
        if (script != null && INTERPRETER_DANGER_PATTERN.test(script)) {
            return {
                category: "shell_evasion",
                code: "SHELL_INTERPRETER_ONELINER",
                pattern: `${program} -c with FS/network mutation`,
                matchedToken: script,
            };
        }
    }
    if (program === "base64" && tail.includes("-d")) {
        return { category: "shell_evasion", code: "SHELL_BASE64_DECODE", pattern: "base64 -d" };
    }
    return null;
}

function checkExfiltration(input: NormalizedToolCall): LayerZeroHit | null {
    const program = programName(input.argv);
    const tail = input.argv.slice(1);
    if (program === "curl") {
        if (
            tail.some(
                (token) =>
                    token === "-d" ||
                    token === "--data" ||
                    token === "--data-binary" ||
                    token === "--data-raw" ||
                    token === "--data-urlencode" ||
                    token === "-F" ||
                    token === "--form",
            )
        ) {
            return { category: "exfiltration", code: "EXFIL_CURL_DATA", pattern: "curl --data/--form" };
        }
        const methodFlagIndex = tail.findIndex((token) => token === "-X" || token === "--request");
        const method = methodFlagIndex !== -1 ? tail[methodFlagIndex + 1]?.toUpperCase() : undefined;
        if (method != null && WRITE_HTTP_METHODS.has(method)) {
            return { category: "exfiltration", code: "EXFIL_CURL_WRITE_METHOD", pattern: `curl -X ${method}` };
        }
        if (tail.some((token) => GIT_CREDENTIAL_URL_PATTERN.test(token))) {
            return {
                category: "exfiltration",
                code: "EXFIL_CREDENTIAL_IN_URL",
                pattern: "url with embedded credentials",
            };
        }
    }
    if (program === "wget") {
        if (tail.some((token) => token.startsWith("--post-data") || token.startsWith("--post-file"))) {
            return { category: "exfiltration", code: "EXFIL_WGET_POST", pattern: "wget --post-*" };
        }
    }
    if (program === "scp") {
        return { category: "exfiltration", code: "EXFIL_SCP", pattern: "scp" };
    }
    if (program === "rsync") {
        if (tail.some((token) => REMOTE_HOST_PATH_PATTERN.test(token))) {
            return { category: "exfiltration", code: "EXFIL_RSYNC_REMOTE", pattern: "rsync host:path" };
        }
    }
    if (program === "nc" || program === "ncat") {
        return { category: "exfiltration", code: "EXFIL_NETCAT", pattern: program };
    }
    if (program === "ssh") {
        if (tail.some((token) => /^[^-/][^/]*@[^/]+$/u.test(token))) {
            return { category: "exfiltration", code: "EXFIL_SSH_REMOTE", pattern: "ssh user@host" };
        }
    }
    if (program === "git") {
        if (tail.some((token) => GIT_CREDENTIAL_URL_PATTERN.test(token))) {
            return {
                category: "exfiltration",
                code: "EXFIL_CREDENTIAL_IN_URL",
                pattern: "git with embedded credentials",
            };
        }
    }
    return null;
}

function checkPathOnlyHits(input: NormalizedToolCall): LayerZeroHit | null {
    for (const path of input.paths) {
        if (isGitHookPath(path)) {
            return {
                category: "destructive_git",
                code: "DESTRUCTIVE_GIT_HOOKS_WRITE",
                pattern: ".git/hooks/** write",
                matchedToken: path,
            };
        }
        if (isHuskyPath(path)) {
            return {
                category: "destructive_git",
                code: "DESTRUCTIVE_GIT_HUSKY_WRITE",
                pattern: ".husky/** write",
                matchedToken: path,
            };
        }
        const escalation = classifySelfPrivilegeEscalationPath(path);
        if (escalation != null) {
            return {
                category: "self_privilege_escalation",
                code: escalation.code,
                pattern: escalation.pattern,
                matchedToken: path,
            };
        }
    }
    return null;
}

const INTERPRETER_PROGRAMS = new Set(["python", "python3", "perl", "ruby", "node"]);
const INTERPRETER_DANGER_PATTERN =
    /(subprocess|os\.system|os\.remove|os\.rmdir|shutil\.rmtree|requests\.(?:post|put|patch|delete)|urllib\.request|child_process|fs\.(?:unlink|rmdir|rm|writeFile)|socket\.|net\.connect|http\.request|fetch\()/u;
const WRITE_HTTP_METHODS = new Set(["POST", "PUT", "PATCH", "DELETE"]);
const REMOTE_HOST_PATH_PATTERN = /^(?:[^/\s]+@[^/\s]+:|[A-Za-z][A-Za-z0-9.-]*:[^/])/u;
const GIT_CREDENTIAL_URL_PATTERN = /^https?:\/\/[^@/\s]+:[^@/\s]+@/u;

function classifySelfPrivilegeEscalationPath(path: string): { code: string; pattern: string } | null {
    const normalized = path.replace(/\\/gu, "/");
    if (/(?:^|\/)\.claude\/(?:settings\.json|settings\.local\.json)$/u.test(normalized)) {
        return { code: "SELF_PRIV_CLAUDE_SETTINGS", pattern: ".claude/settings*.json" };
    }
    if (/(?:^|\/)\.claude\//u.test(normalized)) {
        return { code: "SELF_PRIV_CLAUDE_DIR", pattern: ".claude/**" };
    }
    if (/(?:^|\/)\.(?:bashrc|zshrc|profile|bash_profile)$/u.test(normalized)) {
        return { code: "SELF_PRIV_SHELL_RC", pattern: "shell rc file" };
    }
    if (/(?:^|\/)\.config\/fish\/config\.fish$/u.test(normalized)) {
        return { code: "SELF_PRIV_FISH_CONFIG", pattern: "fish config" };
    }
    if (/(?:^|\/)\.config\/git\//u.test(normalized)) {
        return { code: "SELF_PRIV_GIT_CONFIG", pattern: "~/.config/git/**" };
    }
    if (/(?:^|\/)\.npmrc$/u.test(normalized) || /(?:^|\/)pip\.conf$/u.test(normalized)) {
        return { code: "SELF_PRIV_PACKAGE_CONFIG", pattern: ".npmrc / pip.conf" };
    }
    return null;
}

function hasRecursiveForceFlag(tail: readonly string[]): boolean {
    const flagTokens = tail.filter((token) => token.startsWith("-") && !token.startsWith("--"));
    const hasShortRecursive = flagTokens.some((token) => /[rR]/.test(token));
    const hasShortForce = flagTokens.some((token) => /f/.test(token));
    const longFlags = tail.filter((token) => token.startsWith("--"));
    const hasLongRecursive = longFlags.includes("--recursive");
    const hasLongForce = longFlags.includes("--force");
    return (hasShortRecursive || hasLongRecursive) && (hasShortForce || hasLongForce);
}

function findRmTarget(tail: readonly string[]): string | undefined {
    for (const token of tail) {
        if (token.startsWith("-")) {
            continue;
        }
        return token;
    }
    return undefined;
}

function isCatastrophicRmTarget(target: string): boolean {
    const collapsedTrailingSlash = target.length > 1 ? target.replace(/\/+$/u, "") : target;
    return (
        collapsedTrailingSlash === "/" ||
        collapsedTrailingSlash === "~" ||
        collapsedTrailingSlash === "$HOME" ||
        collapsedTrailingSlash === `$${"{"}HOME}` ||
        collapsedTrailingSlash === "/*" ||
        collapsedTrailingSlash === "~/*"
    );
}

function hasGlobalInstall(program: string, tail: readonly string[]): boolean {
    if (program === "npm" || program === "pnpm") {
        const installVerb = tail.find((token) => token === "install" || token === "i" || token === "add");
        if (installVerb == null) {
            return false;
        }
        return tail.includes("-g") || tail.includes("--global");
    }
    if (program === "yarn") {
        return tail[0] === "global" && (tail[1] === "add" || tail[1] === "install");
    }
    return false;
}

function isGitHookPath(path: string): boolean {
    return /(?:^|[\\/])\.git[\\/]hooks[\\/]/u.test(path);
}

function isHuskyPath(path: string): boolean {
    return /(?:^|[\\/])\.husky[\\/]/u.test(path);
}
