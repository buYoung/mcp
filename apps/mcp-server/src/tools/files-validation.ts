import { realpath } from "node:fs/promises";
import { isAbsolute, relative, resolve, sep } from "node:path";

export async function validateFilesWithinCwd(
    files: readonly string[] | undefined,
    baseDirectory: string = process.cwd(),
): Promise<string[] | undefined> {
    if (files == null || files.length === 0) {
        return undefined;
    }

    const cwdBoundary = await resolveBoundary(baseDirectory);
    const validatedFiles: string[] = [];

    for (const rawFilePath of files) {
        if (!isAbsolute(rawFilePath)) {
            throw new Error(`File path must be absolute: ${rawFilePath}`);
        }
        const absoluteFilePath = resolve(rawFilePath);
        const canonicalFilePath = await resolveBoundary(absoluteFilePath);
        if (!isPathWithinBoundary(canonicalFilePath, cwdBoundary)) {
            throw new Error(
                `File path is outside the server cwd and was rejected: ${rawFilePath} (resolved=${canonicalFilePath}, cwd=${cwdBoundary})`,
            );
        }
        validatedFiles.push(canonicalFilePath);
    }

    return validatedFiles;
}

async function resolveBoundary(absolutePath: string): Promise<string> {
    try {
        return await realpath(absolutePath);
    } catch {
        return resolve(absolutePath);
    }
}

export function isPathWithinBoundary(candidatePath: string, boundaryPath: string): boolean {
    if (candidatePath === boundaryPath) {
        return true;
    }
    const relativePath = relative(boundaryPath, candidatePath);
    if (relativePath.length === 0) {
        return true;
    }
    if (relativePath.startsWith("..")) {
        return false;
    }
    if (isAbsolute(relativePath)) {
        return false;
    }
    return !relativePath.split(sep).includes("..");
}
