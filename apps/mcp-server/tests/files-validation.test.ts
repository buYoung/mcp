import { mkdtemp, realpath, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, sep } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { isPathWithinBoundary, validateFilesWithinCwd } from "../src/tools/files-validation.js";

describe("isPathWithinBoundary", () => {
    const root = sep === "/" ? "/tmp/project" : "C:\\project";

    it("accepts identical paths", () => {
        expect(isPathWithinBoundary(root, root)).toBe(true);
    });

    it("accepts nested paths", () => {
        expect(isPathWithinBoundary(`${root}${sep}src${sep}index.ts`, root)).toBe(true);
    });

    it("rejects parent paths", () => {
        expect(isPathWithinBoundary(sep === "/" ? "/tmp" : "C:\\", root)).toBe(false);
    });

    it("rejects sibling paths", () => {
        expect(isPathWithinBoundary(`${root}-other`, root)).toBe(false);
    });
});

describe("validateFilesWithinCwd", () => {
    let temporaryDirectory: string;
    let outsideDirectory: string;

    beforeEach(async () => {
        temporaryDirectory = await mkdtemp(join(tmpdir(), "files-validation-cwd-"));
        outsideDirectory = await mkdtemp(join(tmpdir(), "files-validation-outside-"));
    });

    afterEach(async () => {
        // mkdtemp under tmpdir is auto-collected by OS; no aggressive cleanup needed for short test runs.
        void temporaryDirectory;
        void outsideDirectory;
    });

    it("returns undefined for nullish or empty input", async () => {
        await expect(validateFilesWithinCwd(undefined, temporaryDirectory)).resolves.toBeUndefined();
        await expect(validateFilesWithinCwd([], temporaryDirectory)).resolves.toBeUndefined();
    });

    it("accepts a file inside cwd", async () => {
        const filePath = join(temporaryDirectory, "inside.txt");
        await writeFile(filePath, "x");
        const canonicalFilePath = await realpath(filePath);
        const validated = await validateFilesWithinCwd([filePath], temporaryDirectory);
        expect(validated).toEqual([canonicalFilePath]);
    });

    it("rejects a file outside cwd", async () => {
        const filePath = join(outsideDirectory, "outside.txt");
        await writeFile(filePath, "x");
        await expect(validateFilesWithinCwd([filePath], temporaryDirectory)).rejects.toThrow(/outside/);
    });

    it("rejects relative paths", async () => {
        await expect(validateFilesWithinCwd(["relative.txt"], temporaryDirectory)).rejects.toThrow(/absolute/);
    });

    it("rejects a symlink within cwd pointing outside", async () => {
        const outsideFile = join(outsideDirectory, "secret.txt");
        await writeFile(outsideFile, "shh");
        const symlinkPath = join(temporaryDirectory, "link.txt");
        await symlink(outsideFile, symlinkPath);
        await expect(validateFilesWithinCwd([symlinkPath], temporaryDirectory)).rejects.toThrow(/outside/);
    });
});
