#!/usr/bin/env node

// Generate the codemap-search release-notes section (English + Korean) with the
// codex CLI and insert it under [Unreleased] in apps/codemap-search/CHANGELOG*.
// Runs in release-it's `after:bump` hook, before the release commit, so the
// changelog is part of the same commit the user confirms. Commits are scoped to
// apps/codemap-search so unrelated acp-bridge/scout work never leaks into the
// per-crate changelog. If codex is unavailable or fails, the script exits
// non-zero before any file is written, aborting the release before commit/tag.

const { spawnSync, execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..", "..");
const CRATE_DIR = path.join(ROOT, "apps", "codemap-search");
const CRATE_PATHSPEC = "apps/codemap-search";
const PROMPT_TEMPLATE_PATH = path.join(__dirname, "changelog-prompt.md");

const CHANGELOG_FILES = [
    { language: "en", file: path.join(CRATE_DIR, "CHANGELOG.md") },
    { language: "ko", file: path.join(CRATE_DIR, "CHANGELOG.ko.md") },
];

function fail(message) {
    process.stderr.write(`[write-changelog] ${message}\n`);
    process.exit(1);
}

function ensureCodexAvailable() {
    const result = spawnSync("codex", ["--version"], {
        stdio: ["ignore", "pipe", "pipe"],
        encoding: "utf8",
    });
    if (result.error && result.error.code === "ENOENT") {
        fail(
            "codex CLI not found on PATH. Install OpenAI Codex CLI and re-run release. " +
                "(release was aborted before any commit/tag.)",
        );
    }
    if (result.status !== 0) {
        fail(
            `codex --version exited with status ${result.status}. ` +
                `stderr: ${(result.stderr || "").trim() || "(empty)"}`,
        );
    }
}

function git(args) {
    return execFileSync("git", args, { cwd: ROOT, encoding: "utf8" });
}

function collectCommits(previousTag) {
    const range = previousTag ? `${previousTag}..HEAD` : null;
    const args = ["log", "--no-merges", "--reverse", "--pretty=format:- %s%n%b%n----COMMIT-END----"];
    if (range) args.push(range);
    // Limit to the codemap-search crate so a monorepo release of one app does not
    // pull in commits that only touched acp-bridge, scout, docs, or tooling.
    args.push("--", CRATE_PATHSPEC);
    const raw = git(args);
    const trimmed = raw
        .split("----COMMIT-END----")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .join("\n\n");
    return trimmed || "(no commits found in range)";
}

function readChangelogSample(file) {
    if (!fs.existsSync(file)) return "(no prior changelog available)";
    const content = fs.readFileSync(file, "utf8");
    const versionSections = content.match(/## \[[^\]]+\][\s\S]*?(?=\n## \[|$)/g);
    if (!versionSections) return "(no prior version sections in changelog)";
    const realSections = versionSections.filter((section) => !/^## \[Unreleased\]/i.test(section.trim()));
    if (realSections.length === 0) return "(this will be the first tagged release)";
    return realSections.slice(0, 2).join("\n\n");
}

function buildPrompt({ template, version, previousTag, commits, sample, language }) {
    return template
        .replaceAll("${VERSION}", version)
        .replaceAll("${PREVIOUS_TAG}", previousTag || "(none — first release)")
        .replaceAll("${COMMITS}", commits)
        .replaceAll("${SAMPLE}", sample)
        .replaceAll("${LANGUAGE}", language);
}

function runCodex(prompt) {
    const tmpFile = path.join(
        os.tmpdir(),
        `codex-changelog-${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2)}.md`,
    );
    try {
        const result = spawnSync(
            "codex",
            [
                "exec",
                "--cd",
                ROOT,
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "--ephemeral",
                "--color",
                "never",
                "--output-last-message",
                tmpFile,
                "-",
            ],
            {
                input: prompt,
                encoding: "utf8",
                stdio: ["pipe", "pipe", "inherit"],
            },
        );
        if (result.status !== 0) {
            fail(`codex exec exited with status ${result.status}. ` + `Release aborted before any commit/tag.`);
        }
        if (!fs.existsSync(tmpFile)) {
            fail("codex exec finished but no output file was written. Release aborted.");
        }
        const body = fs.readFileSync(tmpFile, "utf8").trim();
        if (!body) {
            fail("codex exec produced an empty changelog body. Release aborted.");
        }
        return body;
    } finally {
        if (fs.existsSync(tmpFile)) {
            try {
                fs.unlinkSync(tmpFile);
            } catch {
                /* ignore */
            }
        }
    }
}

function todayIso() {
    const d = new Date();
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${yyyy}-${mm}-${dd}`;
}

// Keep a Changelog header with an [Unreleased] anchor, used to bootstrap a fresh
// changelog on the first release (or for a brand-new crate). The intro language
// follows the file's OUTPUT_LANGUAGE so CHANGELOG.md and CHANGELOG.ko.md stay
// consistent with the bodies codex generates for each.
function bootstrapChangelog(language) {
    const intro =
        language === "ko"
            ? "codemap-search의 주요 변경 사항을 이 파일에 기록합니다.\n형식은 [Keep a Changelog](https://keepachangelog.com/ko/1.1.0/)를 따르며,\n[유의적 버전](https://semver.org/lang/ko/)을 준수합니다."
            : "All notable changes to codemap-search are documented in this file.\n" +
              "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\n" +
              "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).";
    return `# Changelog\n\n${intro}\n\n## [Unreleased]\n`;
}

function insertVersionSection(filePath, version, body, language) {
    const date = todayIso();
    const newSectionHeader = `## [${version}] - ${date}`;
    const fresh = `## [Unreleased]\n\n${newSectionHeader}\n\n${body}\n`;

    let content = fs.existsSync(filePath) ? fs.readFileSync(filePath, "utf8") : "";

    // First release / new crate: no changelog yet. Seed a header that carries the
    // [Unreleased] anchor so the insert below has somewhere to land, instead of
    // aborting the release. A file that exists with content but no anchor is a
    // different (corrupted) case and still fails via the guard below.
    if (!content.trim()) {
        content = bootstrapChangelog(language);
    }

    const unreleasedRegex = /## \[Unreleased\]\s*\n*/i;
    if (!unreleasedRegex.test(content)) {
        fail(
            `${path.relative(ROOT, filePath)} has no [Unreleased] section. ` +
                `Add "## [Unreleased]" so the script knows where to insert new releases.`,
        );
    }

    content = content.replace(unreleasedRegex, fresh + "\n");
    fs.writeFileSync(filePath, content, "utf8");
}

function main() {
    const [, , latestTagArg, versionArg] = process.argv;

    const version = (versionArg || "").trim();
    if (!version) {
        fail("Usage: write-changelog.js <latestTag> <version>");
    }

    const previousTag = (latestTagArg || "").trim();
    const previousTagIsAbsent =
        !previousTag || previousTag === "null" || previousTag === "undefined" || previousTag === "0.0.0";
    const previousTagForGit = previousTagIsAbsent ? null : previousTag;

    ensureCodexAvailable();

    const template = fs.readFileSync(PROMPT_TEMPLATE_PATH, "utf8");
    const commits = collectCommits(previousTagForGit);

    for (const { language, file } of CHANGELOG_FILES) {
        const sample = readChangelogSample(file);
        const prompt = buildPrompt({ template, version, previousTag: previousTagForGit, commits, sample, language });
        process.stdout.write(`[write-changelog] generating ${language} via codex exec...\n`);
        const body = runCodex(prompt);
        insertVersionSection(file, version, body, language);
        process.stdout.write(`[write-changelog] wrote ${path.relative(ROOT, file)}\n`);
    }

    process.stdout.write("[write-changelog] done. release-it will commit these files next.\n");
}

main();
