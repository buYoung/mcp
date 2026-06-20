# AGENTS.md

## 1. Overview

`@repo/typescript-config` is a configuration-only workspace package that centralizes TypeScript compiler presets used by the TypeScript MCP apps.

## 2. Folder Structure

- `package.json`: package identity and publishing metadata only; no runtime entrypoint, scripts, or source files.
- `base.json`: shared strict compiler baseline, including NodeNext modules, declaration output, ES2022 target/lib, `strict`, `isolatedModules`, and `noUncheckedIndexedAccess`.
- `node.json`: Node app preset extending `base.json` with `outDir`, `rootDir`, source maps, and Node types.

## 3. Core Behaviors & Patterns

- **Layered presets**: `node.json` extends `base.json`; app `tsconfig.json` files extend `@repo/typescript-config/node.json` and override package-local paths such as `outDir`, `rootDir`, `include`, and `exclude`.
- **Configuration-only boundary**: this package has no `src`, runtime exports, generated output, or app logic. Changes should stay limited to JSON compiler options and package metadata.
- **Strict defaults propagate outward**: `strict`, `isolatedModules`, `moduleResolution: "NodeNext"`, `noUncheckedIndexedAccess`, declaration output, and ES2022 defaults affect every TypeScript app that extends the preset.

## 4. Conventions

- **File shape**: keep presets as small strict JSON files with `$schema` first, then `extends` when present, then `compilerOptions`.
- **Scope discipline**: put broadly applicable TypeScript defaults in `base.json`; put Node-specific `src`/`dist`/types behavior in `node.json`; leave package-specific include/exclude paths in consuming app configs.
- **Compiler options**: use canonical TypeScript option names and JSON values. Do not add comments because these files are strict JSON, not JSONC.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.
