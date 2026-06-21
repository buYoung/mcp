I have a complete picture. Here's the analysis:

---

## The Packaging Flow

**Files involved:**
- `tools/release/npm/` — the "neighboring source tree" (no `package.json` of its own)
- `tools/release/npm/bin.cjs` — the startup file (generated output, copied into the dist)
- `tools/release/npm/install_api.cjs` — the helper module
- `tools/release/npm/build.ts:104-106` — where all three are copied into `dist/deno/`

`build.ts` produces the `deno` npm package with this `package.json` shape:

```json
{
  "name": "deno",
  "optionalDependencies": {
    "@deno/linux-x64-glibc": "<version>",
    "@deno/darwin-arm64":    "<version>",
    ...
  }
}
```

The helper `install_api.cjs` contains the critical bare package import at `install_api.cjs:26`:

```javascript
const sourcePackagePath = path.dirname(
  require.resolve("@deno/" + target + "/package.json"),
);
```

`tools/release/npm/` has **no `package.json`** listing `@deno/*` — so standing alone, that `require.resolve` call has no manifest justification whatsoever.

---

## Resolution Path That Makes It Work

When a user runs `npm install deno`, npm places the package at `node_modules/deno/` and co-installs the `optionalDependencies` at the sibling scope:

```
node_modules/
  deno/
    install_api.cjs   ← __dirname here
  @deno/
    linux-x64-glibc/
      package.json    ← found via sibling walk
```

At postinstall time, `install_api.cjs` runs from `node_modules/deno/`. Node's CJS `require.resolve` walks up the `node_modules` chain:

1. `node_modules/deno/node_modules/@deno/<target>/` — doesn't exist
2. `node_modules/@deno/<target>/package.json` — **found** (sibling scope directory)

The source-tree copy has no manifest for this, but at runtime the distributed `deno` package's `optionalDependencies` is what causes `@deno/<target>` to be installed into that sibling slot. The helper works because it runs in the context of the distributable, not the source tree.

---

## When It Silently Binds the Wrong Version

In a **monorepo / hoisted workspace**, if `@deno/<target>` was already installed at an ancestor `node_modules` at a different version:

```
workspace-root/
  node_modules/
    @deno/
      linux-x64-glibc@1.40.0/  ← hoisted by another package in the workspace
  packages/
    my-app/
      node_modules/
        deno@1.41.0/
          install_api.cjs       ← __dirname here
          (no nested @deno/ because npm deduped or hoisted)
```

Node's upward walk from `node_modules/deno/` crosses the `my-app/node_modules/` boundary and finds `workspace-root/node_modules/@deno/linux-x64-glibc@1.40.0/` — the **wrong version**. npm won't nest a second copy of `@deno/<target>` under `node_modules/deno/node_modules/` unless there is a hard version conflict it cannot reconcile.

The result: `install_api.cjs:28` builds `sourceExecutablePath` pointing to the 1.40.0 binary, which it then hard-links or copies in silently (`install_api.cjs:46`). No error is raised, so the user ends up running a mismatched Deno binary with no indication anything went wrong.
