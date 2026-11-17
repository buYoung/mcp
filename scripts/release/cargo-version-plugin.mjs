import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Plugin } from "release-it";

const WRITE_CHANGELOG_SCRIPT = fileURLToPath(new URL("./write-changelog.js", import.meta.url));
const PACKAGE_VERSION_RE = /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]*)(")/;
const PACKAGE_NAME_RE = /\[package\][\s\S]*?\nname\s*=\s*"([^"]*)"/;
const CHANGELOG_FILENAMES = ["CHANGELOG.md", "CHANGELOG.ko.md"];
const TRACKED_VERSION_FILES = ["Cargo.toml", "Cargo.lock", ...CHANGELOG_FILENAMES];

function todayIso() {
    const now = new Date();
    const yyyy = now.getFullYear();
    const mm = String(now.getMonth() + 1).padStart(2, "0");
    const dd = String(now.getDate()).padStart(2, "0");
    return `${yyyy}-${mm}-${dd}`;
}

export function readCargoName(directory) {
    const cargoTomlPath = path.join(directory, "Cargo.toml");
    const raw = fs.readFileSync(cargoTomlPath, "utf8");
    const match = raw.match(PACKAGE_NAME_RE);
    if (!match?.[1]) {
        throw new Error(`Could not locate [package].name in ${cargoTomlPath}`);
    }
    return match[1];
}

export function readCargoVersion(directory) {
    const cargoTomlPath = path.join(directory, "Cargo.toml");
    const raw = fs.readFileSync(cargoTomlPath, "utf8");
    const match = raw.match(PACKAGE_VERSION_RE);
    if (!match?.[2]) {
        throw new Error(`Could not locate [package].version in ${cargoTomlPath}`);
    }
    return match[2];
}

export function checkCargoVersionSource(options, directory, root, git) {
    if (options.npm !== false) {
        throw new Error("Cargo-versioned service apps must set npm: false in .release-it.json.");
    }
    for (const name of TRACKED_VERSION_FILES) {
        const file = path.join(directory, name);
        if (!fs.existsSync(file)) {
            throw new Error(`Missing required release file: ${path.relative(root, file)}`);
        }
        try {
            git("ls-files", "--error-unmatch", "--", path.relative(root, file));
        } catch (error) {
            if (error.status !== 1) throw error;
            throw new Error(`Track the release file before releasing: ${path.relative(root, file)}`);
        }
    }
}

function readUnreleasedBody(changelogContent) {
    const match = changelogContent.match(/## \[Unreleased\][ \t]*\r?\n([\s\S]*?)(?=\r?\n## \[|$)/i);
    return match?.[1]?.trim() || "";
}

function promoteUnreleasedSection(filePath, version) {
    const raw = fs.readFileSync(filePath, "utf8");
    const unreleasedBlockRe = /(## \[Unreleased\][ \t]*\r?\n)([\s\S]*?)(?=\r?\n## \[|$)/i;
    const match = raw.match(unreleasedBlockRe);
    if (!match) {
        throw new Error(`${filePath} has no ## [Unreleased] section.`);
    }
    const body = match[2].trim();
    if (!body) return false;

    const replacement = `## [Unreleased]\n\n## [${version}] - ${todayIso()}\n\n${body}\n`;
    fs.writeFileSync(filePath, raw.replace(unreleasedBlockRe, replacement), "utf8");
    return true;
}

function updateCargoManifests(directory, crateName, version) {
    const cargoTomlPath = path.join(directory, "Cargo.toml");
    const cargoLockPath = path.join(directory, "Cargo.lock");

    const tomlRaw = fs.readFileSync(cargoTomlPath, "utf8");
    if (!PACKAGE_VERSION_RE.test(tomlRaw)) {
        throw new Error(`Could not locate [package].version in ${cargoTomlPath}`);
    }
    fs.writeFileSync(cargoTomlPath, tomlRaw.replace(PACKAGE_VERSION_RE, `$1${version}$3`), "utf8");

    const escapedName = crateName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const lockRe = new RegExp(`(name = "${escapedName}"\\r?\\nversion = ")([^"]*)(")`);
    const lockRaw = fs.readFileSync(cargoLockPath, "utf8");
    if (!lockRe.test(lockRaw)) {
        throw new Error(`Could not locate ${crateName} package block in ${cargoLockPath}`);
    }
    fs.writeFileSync(cargoLockPath, lockRaw.replace(lockRe, `$1${version}$3`), "utf8");
}

export default class CargoVersionPlugin extends Plugin {
    static disablePlugin() {
        return "npm";
    }

    getName() {
        return readCargoName(process.cwd());
    }

    getLatestVersion() {
        return readCargoVersion(process.cwd());
    }

    getChangelog(latestVersion) {
        const enChangelog = path.join(process.cwd(), "CHANGELOG.md");
        if (fs.existsSync(enChangelog)) {
            const unreleased = readUnreleasedBody(fs.readFileSync(enChangelog, "utf8"));
            if (unreleased) return unreleased;
        }
        const tag = `codemap-v${latestVersion}`;
        try {
            execFileSync("git", ["show-ref", "--tags", "--quiet", "--verify", "--", `refs/tags/${tag}`], {
                stdio: "ignore",
            });
            return execFileSync("git", ["log", "--no-merges", `--pretty=format:* %s (%h)`, `${tag}..HEAD`, "--", "."], {
                encoding: "utf8",
            }).trim();
        } catch {
            return execFileSync("git", ["log", "--no-merges", "-n", "20", "--pretty=format:* %s (%h)", "--", "."], {
                encoding: "utf8",
            }).trim();
        }
    }

    bump(version) {
        const directory = process.cwd();
        const crateName = readCargoName(directory);
        const previousVersion = readCargoVersion(directory);
        if (this.config.isDryRun) {
            this.log.info(`Would update Cargo.toml, Cargo.lock, and changelogs from ${previousVersion} to ${version}`);
            return;
        }

        updateCargoManifests(directory, crateName, version);

        const changelogPaths = CHANGELOG_FILENAMES.map((name) => path.join(directory, name));
        const allHaveUnreleased = changelogPaths.every((filePath) =>
            Boolean(readUnreleasedBody(fs.readFileSync(filePath, "utf8"))),
        );

        if (allHaveUnreleased) {
            for (const filePath of changelogPaths) {
                promoteUnreleasedSection(filePath, version);
            }
            return;
        }

        execFileSync(process.execPath, [WRITE_CHANGELOG_SCRIPT, `codemap-v${previousVersion}`, version], {
            stdio: "inherit",
        });
    }
}
