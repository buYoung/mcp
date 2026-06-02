import { access, constants } from "node:fs/promises";
import { homedir } from "node:os";
import { delimiter, isAbsolute, join } from "node:path";

/**
 * Resolves an executable to an absolute path. Searches PATH and, in addition,
 * the Go install locations (`$GOBIN`, `$GOPATH/bin`, `~/go/bin`). `go install`
 * places zoekt binaries in `~/go/bin`, which is frequently absent from PATH —
 * probing PATH alone would report installed binaries as missing (DESIGN §5).
 */
export async function resolveExecutablePath(command: string): Promise<string | undefined> {
    if (command.trim().length === 0) {
        return undefined;
    }

    if (command.includes("/") || command.includes("\\")) {
        return (await canExecute(command)) ? command : undefined;
    }

    const searchDirectories = collectSearchDirectories();
    const executableExtensions =
        process.platform === "win32" ? (process.env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD").split(";") : [""];

    for (const directory of searchDirectories) {
        for (const extension of executableExtensions) {
            const candidate = join(directory, `${command}${extension}`);
            if (await canExecute(candidate)) {
                return candidate;
            }
        }
    }
    return undefined;
}

export async function isCommandAvailable(command: string): Promise<boolean> {
    return (await resolveExecutablePath(command)) != null;
}

function collectSearchDirectories(): string[] {
    const directories: string[] = [];
    const pathEnvironmentValue = process.env.PATH ?? "";
    for (const directory of pathEnvironmentValue.split(delimiter)) {
        if (directory.length > 0) {
            directories.push(directory);
        }
    }
    for (const fallbackDirectory of goBinaryDirectories()) {
        if (!directories.includes(fallbackDirectory)) {
            directories.push(fallbackDirectory);
        }
    }
    return directories;
}

function goBinaryDirectories(): string[] {
    const directories: string[] = [];
    const goBin = process.env.GOBIN;
    if (goBin != null && goBin.trim().length > 0) {
        directories.push(goBin.trim());
    }
    const goPath = process.env.GOPATH;
    if (goPath != null && goPath.trim().length > 0) {
        for (const entry of goPath.split(delimiter)) {
            if (entry.length > 0) {
                directories.push(join(entry, "bin"));
            }
        }
    }
    directories.push(join(homedir(), "go", "bin"));
    return directories;
}

async function canExecute(executablePath: string): Promise<boolean> {
    try {
        const mode = process.platform === "win32" ? constants.F_OK : constants.X_OK;
        const resolvedPath = isAbsolute(executablePath) ? executablePath : join(process.cwd(), executablePath);
        await access(resolvedPath, mode);
        return true;
    } catch {
        return false;
    }
}
