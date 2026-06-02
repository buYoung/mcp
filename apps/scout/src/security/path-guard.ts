import { homedir } from "node:os";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

/**
 * Normalizes a user-supplied path: trims, expands a leading `~`, and resolves a
 * relative path against the current working directory.
 */
export function expandPath(inputPath: string): string {
    const trimmedPath = inputPath.trim();
    if (trimmedPath === "~") {
        return homedir();
    }
    if (trimmedPath.startsWith("~/") || trimmedPath.startsWith("~\\")) {
        return join(homedir(), trimmedPath.slice(2));
    }
    return isAbsolute(trimmedPath) ? trimmedPath : resolve(process.cwd(), trimmedPath);
}

/**
 * Resolves `targetPath` and asserts it stays inside `rootPath`, returning the
 * repository-relative prefix in POSIX form (zoekt `file:` filters use `/`).
 */
export function resolveRelativePathWithinRoot(targetPath: string, rootPath: string): string {
    const resolvedRoot = resolve(rootPath);
    const resolvedTarget = expandPath(targetPath);
    const relativePath = relative(resolvedRoot, resolvedTarget);
    if (relativePath.startsWith("..") || isAbsolute(relativePath)) {
        throw new Error(`Path escapes the repository root: ${targetPath}`);
    }
    return relativePath.split(sep).join("/");
}
