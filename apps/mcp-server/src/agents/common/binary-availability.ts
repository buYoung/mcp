import { access, constants } from "node:fs/promises";
import { delimiter, isAbsolute, join } from "node:path";

/**
 * Returns true when `command` is invokable from the current process — either an existing
 * absolute path with execute permission, or a name resolvable via PATH. Network and
 * configuration-only lookups are intentionally not performed.
 */
export async function isCommandAvailable(command: string): Promise<boolean> {
    if (command.trim().length === 0) {
        return false;
    }

    if (command.includes("/") || command.includes("\\")) {
        return canExecute(command);
    }

    const pathEnvironmentValue = process.env.PATH ?? "";
    if (pathEnvironmentValue.length === 0) {
        return false;
    }

    const pathExtensions =
        process.platform === "win32" ? (process.env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD").split(";") : [""];

    for (const directory of pathEnvironmentValue.split(delimiter)) {
        if (directory.length === 0) {
            continue;
        }
        for (const extension of pathExtensions) {
            const candidate = join(directory, `${command}${extension}`);
            if (await canExecute(candidate)) {
                return true;
            }
        }
    }
    return false;
}

async function canExecute(absoluteOrRelativePath: string): Promise<boolean> {
    try {
        const mode = process.platform === "win32" ? constants.F_OK : constants.X_OK;
        const resolvedPath = isAbsolute(absoluteOrRelativePath)
            ? absoluteOrRelativePath
            : join(process.cwd(), absoluteOrRelativePath);
        await access(resolvedPath, mode);
        return true;
    } catch {
        return false;
    }
}
