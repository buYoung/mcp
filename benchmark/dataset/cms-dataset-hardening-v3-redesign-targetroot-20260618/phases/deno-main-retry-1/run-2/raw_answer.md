Now I have the full picture. Here is the precise analysis.

---

## The packaging flow

**The pieces:**

| Role | File |
|---|---|
| Build script / distributable generator | `tools/release/npm/build.ts` |
| Generated package manifest | `dist/deno/package.json` (written by `build.ts`) |
| Startup file (bin entry) | `tools/release/npm/bin.cjs` → copied into `dist/deno/` |
| Helper from neighboring source tree | `tools/release/npm/install_api.cjs` → copied into `dist/deno/` |

`install_api.cjs` is the key file. It has no nearby npm `package.json` in the source tree (`tools/release/npm/` contains no `package.json`; the only manifest nearby is `tools/deno.json`, a Deno config). Yet at line 26 it makes a bare package import:

```js
const sourcePackagePath = path.dirname(
  require.resolve("@deno/" + target + "/package.json"),  // line 26
);
```

---

## Why the build still works

`build.ts` generates the `dist/deno/package.json` and populates its `optionalDependencies` with the platform-specific packages at the exact same version (`build.ts:100-102`):

```ts
optionalDependencies: packages
  .map((pkg) => `@deno/${getPackageNameNoScope(pkg)}`)
  .reduce((obj, pkgName) => ({ ...obj, [pkgName]: version }), {}),
```

So when an end user runs `npm install deno`, npm installs `@deno/linux-x64-glibc` (etc.) alongside. When the `postinstall` script fires `node ./install.cjs` → `install_api.cjs`, Node.js's `require.resolve` walks **upward** from `node_modules/deno/install_api.cjs`, finds `node_modules/@deno/<platform>/package.json`, and resolves correctly. The import has no basis in `install_api.cjs`'s own source-tree manifest, but is fully justified by the generated distributable manifest that surrounds it at runtime.

---

## When it silently binds the wrong version

The resolution is purely positional — `require.resolve` walks the directory tree upward from the caller's location. It has no awareness of which `package.json` originally declared the dependency.

**The failure case:** a monorepo or workspace where an **ancestor** `node_modules` already contains a `@deno/<platform>` at a different version (hoisted there by another package), and the package manager's deduplication logic does **not** create a nested copy under `node_modules/deno/node_modules/@deno/`:

```
/project/
  node_modules/
    @deno/
      linux-x64-glibc/   ← version 1.44.0, installed by workspace root
    packages/
      app/
        node_modules/
          deno/
            install_api.cjs   ← this file runs postinstall
```

Node.js checks `node_modules/deno/node_modules/@deno/linux-x64-glibc/` first (not found), then walks up to `node_modules/@deno/linux-x64-glibc/` and finds **1.44.0** — even though the `deno` package in the sub-workspace expected **2.0.0**. The `postinstall` copies that older binary into `node_modules/deno/deno` with no error or warning, because `require.resolve` succeeds. The user then runs a mismatched `deno` binary.

This is most likely to occur with npm workspaces, Yarn Berry workspaces with hoisting, or pnpm with `shamefully-hoist=true`, where the package manager deduplicates across workspace members rather than nesting per-package copies.
