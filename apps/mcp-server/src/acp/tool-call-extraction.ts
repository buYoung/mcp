export interface NormalizedToolCall {
    /** Argv tokens after shell-wrapper unwrapping. Empty when extraction failed. */
    argv: readonly string[];
    /** Original command string when the rawInput exposed one; useful for substring heuristics. */
    rawCommand?: string;
    /** Paths referenced by the tool call (from locations[] and best-effort rawInput inspection). */
    paths: readonly string[];
}

const SHELL_WRAPPERS = new Set(["sh", "bash", "zsh", "dash", "ksh"]);

export function extractToolCallArgv(rawInput: unknown): { argv: readonly string[]; rawCommand?: string } {
    if (rawInput == null || typeof rawInput !== "object") {
        return { argv: [] };
    }
    const candidate = rawInput as Record<string, unknown>;

    const argv = readArgvCandidate(candidate.command) ?? readArgvCandidate(candidate.argv);
    if (argv && argv.length > 0) {
        return unwrapShellWrapper(argv);
    }

    const stringCommand = readStringCandidate(candidate.command) ?? readStringCandidate(candidate.commandLine);
    if (stringCommand != null) {
        const tokens = tokenizeShellCommand(stringCommand);
        return unwrapShellWrapper(tokens, stringCommand);
    }

    return { argv: [] };
}

export function extractToolCallPaths(
    rawInput: unknown,
    locations?: ReadonlyArray<{ path?: unknown }> | null,
): readonly string[] {
    const paths = new Set<string>();
    if (locations) {
        for (const location of locations) {
            if (typeof location?.path === "string" && location.path.length > 0) {
                paths.add(location.path);
            }
        }
    }
    if (rawInput && typeof rawInput === "object") {
        const candidate = rawInput as Record<string, unknown>;
        for (const key of ["path", "file_path", "filePath", "target", "destination"]) {
            const value = candidate[key];
            if (typeof value === "string" && value.length > 0) {
                paths.add(value);
            }
        }
    }
    return [...paths];
}

export function normalizeToolCall(
    rawInput: unknown,
    locations?: ReadonlyArray<{ path?: unknown }> | null,
): NormalizedToolCall {
    const { argv, rawCommand } = extractToolCallArgv(rawInput);
    const paths = extractToolCallPaths(rawInput, locations);
    const result: NormalizedToolCall = { argv, paths };
    if (rawCommand != null) {
        return { ...result, rawCommand };
    }
    return result;
}

function readArgvCandidate(value: unknown): readonly string[] | null {
    if (!Array.isArray(value)) {
        return null;
    }
    const tokens = value.filter((token): token is string => typeof token === "string" && token.length > 0);
    return tokens.length === value.length ? tokens : null;
}

function readStringCandidate(value: unknown): string | null {
    return typeof value === "string" && value.trim().length > 0 ? value : null;
}

function unwrapShellWrapper(
    tokens: readonly string[],
    originalCommand?: string,
): { argv: readonly string[]; rawCommand?: string } {
    const program = tokens[0];
    if (program == null) {
        return originalCommand != null ? { argv: tokens, rawCommand: originalCommand } : { argv: tokens };
    }
    const programName = basename(program);
    if (SHELL_WRAPPERS.has(programName)) {
        const flagIndex = tokens.findIndex((token, index) => index > 0 && (token === "-c" || token === "--command"));
        if (flagIndex !== -1 && tokens.length > flagIndex + 1) {
            const inner = tokens[flagIndex + 1];
            if (inner != null) {
                const innerTokens = tokenizeShellCommand(inner);
                if (innerTokens.length > 0) {
                    return { argv: innerTokens, rawCommand: inner };
                }
            }
        }
    }
    return originalCommand != null ? { argv: tokens, rawCommand: originalCommand } : { argv: tokens };
}

function basename(commandPath: string): string {
    const separatorIndex = Math.max(commandPath.lastIndexOf("/"), commandPath.lastIndexOf("\\"));
    return separatorIndex === -1 ? commandPath : commandPath.slice(separatorIndex + 1);
}

/**
 * Minimal POSIX-style tokenizer: handles single/double quotes and backslash escapes outside quotes.
 * It is intentionally not a full shell parser — pipes, redirections, and command substitution remain
 * raw tokens so callers can decide whether to treat the command as suspicious.
 */
export function tokenizeShellCommand(input: string): readonly string[] {
    const tokens: string[] = [];
    let current = "";
    let inSingleQuote = false;
    let inDoubleQuote = false;
    let hasCurrentToken = false;
    for (let index = 0; index < input.length; index += 1) {
        const character = input[index];
        if (character == null) {
            continue;
        }
        if (inSingleQuote) {
            if (character === "'") {
                inSingleQuote = false;
                continue;
            }
            current += character;
            hasCurrentToken = true;
            continue;
        }
        if (inDoubleQuote) {
            if (character === '"') {
                inDoubleQuote = false;
                continue;
            }
            if (character === "\\" && index + 1 < input.length) {
                const next = input[index + 1];
                if (next === '"' || next === "\\" || next === "$" || next === "`") {
                    current += next;
                    index += 1;
                    hasCurrentToken = true;
                    continue;
                }
            }
            current += character;
            hasCurrentToken = true;
            continue;
        }
        if (character === "'") {
            inSingleQuote = true;
            hasCurrentToken = true;
            continue;
        }
        if (character === '"') {
            inDoubleQuote = true;
            hasCurrentToken = true;
            continue;
        }
        if (character === "\\" && index + 1 < input.length) {
            const next = input[index + 1];
            if (next != null) {
                current += next;
                index += 1;
                hasCurrentToken = true;
                continue;
            }
        }
        if (/\s/.test(character)) {
            if (hasCurrentToken) {
                tokens.push(current);
                current = "";
                hasCurrentToken = false;
            }
            continue;
        }
        current += character;
        hasCurrentToken = true;
    }
    if (hasCurrentToken) {
        tokens.push(current);
    }
    return tokens;
}
