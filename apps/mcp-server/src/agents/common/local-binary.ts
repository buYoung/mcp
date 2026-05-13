import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

export interface LocalNodeBinaryCommand {
    command: string;
    commandArguments: readonly string[];
}

const requireFromCurrentModule = createRequire(import.meta.url);

export function resolveLocalNodeBinaryCommand(packageName: string, binaryName: string): LocalNodeBinaryCommand {
    const packageJsonPath = requireFromCurrentModule.resolve(`${packageName}/package.json`);
    const packageJsonContent = readFileSync(packageJsonPath, "utf8");
    const packageJson = JSON.parse(packageJsonContent) as { bin?: string | Record<string, string> };
    const binaryRelativePath = readBinaryRelativePath(packageJson, binaryName);

    return {
        command: process.execPath,
        commandArguments: [join(dirname(packageJsonPath), binaryRelativePath)],
    };
}

function readBinaryRelativePath(packageJson: { bin?: string | Record<string, string> }, binaryName: string): string {
    if (typeof packageJson.bin === "string") {
        return packageJson.bin;
    }

    const binaryRelativePath = packageJson.bin?.[binaryName];
    if (typeof binaryRelativePath !== "string" || binaryRelativePath.trim().length === 0) {
        throw new Error(`Package binary not found. binary=${binaryName}`);
    }
    return binaryRelativePath;
}
