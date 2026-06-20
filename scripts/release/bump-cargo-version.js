#!/usr/bin/env node

// Sync the codemap-search crate version to the release version that release-it
// is about to tag. Runs in `after:bump`, before release-it commits, so the
// version line and the changelog land in the same release commit. Pure file
// edits (no `cargo` invocation) keep this offline and free of dependency churn:
// only the crate's own version line moves, never a transitive lock entry.

const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "..", "..");
const CRATE_DIR = path.join(ROOT, "apps", "codemap-search");
const CARGO_TOML = path.join(CRATE_DIR, "Cargo.toml");
const CARGO_LOCK = path.join(CRATE_DIR, "Cargo.lock");

function fail(message) {
    process.stderr.write(`[bump-cargo-version] ${message}\n`);
    process.exit(1);
}

function rel(filePath) {
    return path.relative(ROOT, filePath);
}

// Replace the `version = "..."` line inside the leading `[package]` table only,
// so dependency version requirements elsewhere in Cargo.toml stay untouched.
function bumpCargoToml(version) {
    if (!fs.existsSync(CARGO_TOML)) {
        fail(`${rel(CARGO_TOML)} not found.`);
    }
    const raw = fs.readFileSync(CARGO_TOML, "utf8");
    const packageRegex = /(\[package\][\s\S]*?\nversion\s*=\s*")([^"]*)(")/;
    const match = raw.match(packageRegex);
    if (!match) {
        fail(`could not locate [package].version in ${rel(CARGO_TOML)}`);
    }
    const oldVersion = match[2];
    if (oldVersion === version) {
        process.stdout.write(`[bump-cargo-version] ${rel(CARGO_TOML)} already at ${version}\n`);
        return;
    }
    fs.writeFileSync(CARGO_TOML, raw.replace(packageRegex, `$1${version}$3`), "utf8");
    process.stdout.write(`[bump-cargo-version] ${rel(CARGO_TOML)}: ${oldVersion} -> ${version}\n`);
}

// The lockfile has exactly one `name = "codemap-search"` package block; rewrite
// the `version = "..."` line immediately after that name so the lock matches the
// manifest. Anchoring on the unique name avoids touching any other package.
function bumpCargoLock(version) {
    if (!fs.existsSync(CARGO_LOCK)) {
        fail(`${rel(CARGO_LOCK)} not found.`);
    }
    const raw = fs.readFileSync(CARGO_LOCK, "utf8");
    const lockRegex = /(name = "codemap-search"\nversion = ")([^"]*)(")/;
    const match = raw.match(lockRegex);
    if (!match) {
        fail(`could not locate the codemap-search package block in ${rel(CARGO_LOCK)}`);
    }
    const oldVersion = match[2];
    if (oldVersion === version) {
        process.stdout.write(`[bump-cargo-version] ${rel(CARGO_LOCK)} already at ${version}\n`);
        return;
    }
    fs.writeFileSync(CARGO_LOCK, raw.replace(lockRegex, `$1${version}$3`), "utf8");
    process.stdout.write(`[bump-cargo-version] ${rel(CARGO_LOCK)}: ${oldVersion} -> ${version}\n`);
}

function main() {
    const [, , versionArg] = process.argv;
    const version = (versionArg || "").trim();
    if (!version) {
        fail("Usage: bump-cargo-version.js <version>");
    }
    if (!/^[0-9A-Za-z.\-+]+$/.test(version)) {
        fail(`refusing unsafe version string: ${version}`);
    }

    bumpCargoToml(version);
    bumpCargoLock(version);
    process.stdout.write("[bump-cargo-version] done. release-it will commit these files next.\n");
}

main();
