import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { input, select } from "@inquirer/prompts";
import release, { Config } from "release-it";
import semver from "semver";
import { checkCargoVersionSource, readCargoVersion } from "./cargo-version-plugin.mjs";
import { InquirerPrompt, ReleaseStopped, requireAnswer } from "./release-prompts.mjs";

const root = realpathSync(fileURLToPath(new URL("../../", import.meta.url)));
const guardPluginUrl = pathToFileURL(fileURLToPath(new URL("./release-prompts.mjs", import.meta.url))).href;
const cargoPluginUrl = pathToFileURL(fileURLToPath(new URL("./cargo-version-plugin.mjs", import.meta.url))).href;
const prompt = new InquirerPrompt();

const interactiveOptions = {
    ci: false,
    "only-version": false,
    "release-version": false,
    changelog: false,
    "dry-run": false,
    snapshot: false,
    preRelease: false,
};

const readJSON = (file) => JSON.parse(readFileSync(file, "utf8"));
const git = (...args) =>
    execFileSync("git", args, {
        cwd: root,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
    }).trim();

let headBefore;
let targetDirectory;
let hasReportedReleaseError = false;

function readCurrentVersion(directory) {
    return readCargoVersion(directory);
}

function checkVersionSource(options, directory) {
    checkCargoVersionSource(options, directory, root, git);
}

function resolveUpstreamTarget() {
    let upstreamRef;
    try {
        upstreamRef = git("rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}");
    } catch {
        throw new Error("Current branch has no upstream tracking branch configured.");
    }
    const slashIndex = upstreamRef.indexOf("/");
    if (slashIndex <= 0 || slashIndex === upstreamRef.length - 1) {
        throw new Error(`Could not parse upstream remote and branch from '${upstreamRef}'.`);
    }
    return {
        remote: upstreamRef.slice(0, slashIndex),
        branch: upstreamRef.slice(slashIndex + 1),
    };
}

async function chooseTarget() {
    const rootPackage = readJSON(path.join(root, "package.json"));
    const manifestPath = path.join(root, ".release-targets.json");
    const declarations = Array.isArray(rootPackage.workspaces)
        ? rootPackage.workspaces
        : rootPackage.workspaces?.packages;
    const projects = JSON.parse(
        execFileSync("pnpm", ["list", "--recursive", "--depth", "-1", "--json"], {
            cwd: root,
            encoding: "utf8",
        }),
    );
    const isMonorepo =
        projects.some((project) => realpathSync(project.path) !== root) ||
        Boolean(declarations?.length) ||
        existsSync(manifestPath);
    if (!isMonorepo) return { name: rootPackage.name || "project", path: ".", config: true };

    if (!existsSync(manifestPath)) {
        throw new Error("Define verified service apps in .release-targets.json during setup.");
    }
    const { serviceApps } = readJSON(manifestPath);
    if (
        !Array.isArray(serviceApps) ||
        !serviceApps.length ||
        serviceApps.some(
            (app) => typeof app.name !== "string" || typeof app.path !== "string" || !app.name || !app.path,
        ) ||
        new Set(serviceApps.map((app) => app.path)).size !== serviceApps.length
    ) {
        throw new Error("Expected a nonempty list of distinct service-app paths.");
    }
    return requireAnswer(
        select({
            message: "Select one service app:",
            choices: serviceApps.map((app) => ({ value: app, name: app.name, description: app.path })),
        }),
        "service app",
    );
}

async function chooseVersion(currentVersion) {
    const increments = [
        ["patch"],
        ["minor"],
        ["prepatch", "alpha"],
        ["preminor", "beta"],
        ["prerelease", semver.prerelease(currentVersion)?.[0] || "rc"],
        ["major"],
        ["premajor", "alpha"],
    ];
    const choices = increments
        .map(([increment, identifier]) => {
            const version = semver.inc(currentVersion, increment, String(identifier || ""));
            return { value: version, name: `${increment}: ${currentVersion} → ${version}` };
        })
        .filter((option) => option.value && semver.gt(option.value, currentVersion));
    const selected = await requireAnswer(
        select({
            message: `Select version (current: ${currentVersion}):`,
            choices: [...choices, { value: "custom", name: "Enter an exact version" }],
        }),
        "version",
    );
    if (selected !== "custom") return selected;
    const entered = await requireAnswer(
        input({
            message: `Next version (current: ${currentVersion}):`,
            validate: (value) =>
                !semver.valid(value) || !semver.gt(value, currentVersion)
                    ? `Enter a valid semver greater than ${currentVersion}.`
                    : true,
        }),
        "version",
    );
    return semver.valid(entered);
}

function reportState() {
    if (!headBefore) return;
    try {
        console.info(`HEAD before: ${headBefore}\nHEAD now: ${git("rev-parse", "HEAD")}`);
        console.info(`Remaining index/worktree changes:\n${git("status", "--short") || "(clean)"}`);
        if (targetDirectory) {
            console.info(`Version on disk: ${readCurrentVersion(targetDirectory)}`);
        }
        if (prompt.tagName) {
            let tagRef;
            const ref = `refs/tags/${prompt.tagName}`;
            try {
                git("show-ref", "--verify", "--quiet", "--", ref);
                tagRef = git("show-ref", "--verify", "--", ref);
            } catch (error) {
                if (error.status !== 1) throw error;
                tagRef = "(not present locally)";
            }
            console.info(`Local tag ${prompt.tagName}: ${tagRef}`);
        }
        const pushState = prompt.completed.includes("push")
            ? "push command completed"
            : prompt.attempted.includes("push")
              ? "push attempted; remote state requires inspection"
              : "push not attempted";
        console.info(pushState);
    } catch (error) {
        console.warn(`Could not fully inspect remaining state: ${error.message}`);
    }
}

try {
    if (!process.stdin.isTTY || !process.stdout.isTTY) {
        throw new Error("An interactive terminal is required; no release work was started.");
    }
    if (process.argv.length > 2) {
        throw new Error("Run pnpm release without arguments; choose the version in the prompt.");
    }
    process.chdir(root);
    if (git("status", "--porcelain", "--untracked-files=no")) {
        throw new Error("Tracked files and the index must be clean across the repository before releasing.");
    }
    headBefore = git("rev-parse", "HEAD");
    const target = await chooseTarget();
    targetDirectory = realpathSync(path.resolve(root, target.path));
    const relativeTarget = path.relative(root, targetDirectory);
    if (relativeTarget === ".." || relativeTarget.startsWith(`..${path.sep}`) || path.isAbsolute(relativeTarget)) {
        throw new Error("The selected service app must be inside this project.");
    }
    process.chdir(targetDirectory);
    const config = new Config({ config: target.config ?? true, ...interactiveOptions });
    await config.init();
    const options = config.getContext();
    if (!options.git?.commit || !options.git.tag || !options.git.push) {
        throw new Error("The interactive flow requires git commit, tag, and push to be enabled.");
    }
    if ((options.npm && options.npm.publish !== false) || options.github?.release || options.gitlab?.release) {
        throw new Error("This workflow requires npm publishing and hosted releases to be disabled.");
    }
    checkVersionSource(options, targetDirectory);
    if (
        options.git.addUntrackedFiles &&
        git("ls-files", "--others", "--exclude-standard", "--", relativeTarget || ".")
    ) {
        throw new Error("Untracked files would enter the release commit; handle them before releasing.");
    }
    const upstream = resolveUpstreamTarget();
    const currentVersion = readCurrentVersion(targetDirectory);
    if (!semver.valid(currentVersion)) throw new Error("The selected version source needs a valid semver.");
    const selectedVersion = await chooseVersion(currentVersion);
    const resolvedTagName = options.git.tagName
        ? options.git.tagName.replace(/\$\{version\}/g, selectedVersion)
        : `v${selectedVersion}`;

    try {
        await release(
            {
                ...options,
                config: false,
                extends: false,
                ...interactiveOptions,
                increment: selectedVersion,
                git: {
                    ...options.git,
                    requireCleanWorkingDir: false,
                    push: true,
                    pushRepo: "",
                    requireUpstream: true,
                    pushArgs: [
                        "--atomic",
                        "--no-follow-tags",
                        upstream.remote,
                        `HEAD:refs/heads/${upstream.branch}`,
                        `refs/tags/${resolvedTagName}:refs/tags/${resolvedTagName}`,
                    ],
                },
                plugins: {
                    [guardPluginUrl]: ["selected-version-guard", { currentVersion, selectedVersion }],
                    [cargoPluginUrl]: ["cargo-version", {}],
                    ...options.plugins,
                },
            },
            { prompt },
        );
    } catch (error) {
        hasReportedReleaseError = true;
        throw error;
    }
    console.info(`Released ${target.name} ${selectedVersion}.`);
} catch (error) {
    if (!hasReportedReleaseError) {
        if (error instanceof ReleaseStopped) console.info(error.message);
        else console.error(error.message);
    }
    process.exitCode = 1;
} finally {
    process.chdir(root);
    reportState();
}
