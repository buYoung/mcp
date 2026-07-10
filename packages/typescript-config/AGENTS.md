# AGENTS.md

## 1. Overview

`@repo/typescript-config` is a configuration-only workspace package that centralizes TypeScript compiler presets for the TypeScript MCP apps. It has no runtime entry point and affects consumers only through `tsconfig` inheritance.

## 2. Ownership Map

### Stable Ownership Boundaries

- **Strict baseline preset**: Start in `base.json` when changing compiler behavior that should apply to all TypeScript apps. It owns NodeNext module settings, ES2022 target/lib, declaration output, strict mode, isolated modules, JSON module resolution, and `noUncheckedIndexedAccess`; verify through the root TypeScript type-check because both apps consume the preset.
- **Node app preset**: Start in `node.json` when changing Node-specific app defaults. It extends `base.json` and owns `outDir`, `rootDir`, source maps, and Node types; preserve package-level overrides in consuming app `tsconfig.json` files.

## 3. Core Behaviors & Patterns

- **Layered presets**: `node.json` extends `base.json`; `apps/mcp-server/tsconfig.json` and `apps/scout/tsconfig.json` extend `@repo/typescript-config/node.json` and override package-local include/exclude and path settings.
- **Configuration-only boundary**: This package has no `src`, runtime exports, generated output, tests, or package scripts. Changes should stay limited to strict JSON compiler presets and package metadata.
- **Strict defaults propagate outward**: Changes to `strict`, `isolatedModules`, `moduleResolution: "NodeNext"`, `noUncheckedIndexedAccess`, declaration output, or ES2022 defaults can break both TypeScript apps even when this package itself has no source lines.

## 4. Conventions

- **File shape**: Keep presets as small strict JSON files. Put `$schema` first, then `extends` when present, then `compilerOptions`.
- **Scope discipline**: Put broadly applicable TypeScript defaults in `base.json`; put Node app behavior in `node.json`; leave app-specific include/exclude and source/output paths in consuming package configs.
- **Compiler options**: Use canonical TypeScript option names and JSON values. Do not add comments because these files are JSON, not JSONC.
- **Package metadata**: Keep `package.json` focused on package identity and publishing metadata; do not add runtime entry points unless the package stops being configuration-only.

## 5. Working Agreements

See root `/AGENTS.md` for common working agreements.
