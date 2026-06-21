# Private Answer Key

## Candidate

- id: `deno-bundle-snapshot-fallback-v1`
- status: pending validation

## Correct Answer

The obvious explanations are incomplete. It is not enough that the helper file is shipped into the final artifact, and it is not simply "the nearest manifest wins." The decisive path is a special fallback in the bundle resolver: when normal graph resolution fails on a plain package import coming from a module outside the entrypoint's package scope, the bundling path retries against the managed npm snapshot by package name alone.

That fallback keeps the build working because the build-wide snapshot already knows the package, even though the helper's own nearby manifest would not justify the import. The quiet risk is that the fallback carries no version constraint. If the snapshot contains more than one installed version of that package, the fallback can bind whichever snapshot entry matches by name, rather than the version the helper's own nearest package tree would have selected.

## Why

1. The packaging path first tries normal graph resolution. Only after that fails does the bundler enter the special rescue path. In `cli/tools/bundle/mod.rs:1558-1580`, `op_bundle_resolve` catches the resolution error and then conditionally retries instead of failing immediately.
2. The retry is gated to plain package imports only. In `cli/tools/bundle/mod.rs:1567-1573`, the fallback runs only when `looks_like_bare_specifier(path)` is true.
3. The repo-local reason for this retry is modules pulled from outside the entrypoint's package scope during `deno compile --bundle`. `libs/resolver/graph.rs:404-415` documents that these extra modules can resolve fine elsewhere in the same build while failing normal mapping from their own local package scope.
4. The fallback ignores whether the referrer's `package.json` declared the dependency. In `libs/resolver/graph.rs:404-406`, the helper is explicitly described as best-effort resolution "regardless of whether the referrer's `package.json` declares it."
5. The implementation converts the plain package name to `npm:{raw_specifier}` and resolves it through the managed npm resolver. In `libs/resolver/graph.rs:426-445`, `resolve_bare_specifier_in_npm_snapshot` constructs `NpmPackageReqReference::from_str(&format!("npm:{raw_specifier}"))` and then calls `resolve_managed_npm_req_ref(...)`.
6. The fallback is not universal. In `libs/resolver/graph.rs:424-445`, it returns `None` unless npm resolution is managed and the package is present in the build snapshot.
7. The important failure mode is multi-version ambiguity. In `libs/resolver/graph.rs:417-422`, the code comments pin that the fallback resolves `name@*`, so a snapshot with multiple versions may choose by name alone instead of the version the referrer's own nearest `node_modules` tree would have selected.
8. A tempting but wrong explanation is "the compile path just carries the helper along in the artifact." `cli/tools/compile.rs:245-260` shows that referenced files and worker bundles are included for shipping after resolution, but that inclusion step does not explain how the plain package import was mapped in the first place.

## Pinned Facts

- The decisive mechanism is a bundler fallback after normal graph resolution fails.
- The fallback is restricted to plain package imports, not arbitrary specifiers.
- The scenario exists because compile/bundle can pull modules from outside the entrypoint's package scope.
- The fallback ignores whether the helper's own nearby `package.json` declared the dependency.
- The fallback resolves through the managed npm snapshot by synthesizing an `npm:` reference.
- The fallback stops working when npm resolution is not managed or the package is absent from the snapshot.
- The silent risk is multi-version ambiguity: the fallback can bind by package name alone instead of the helper's nearest package-local version.
- Shipping the helper file in the artifact is a separate transport step, not the reason the package import resolves.
