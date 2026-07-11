# Private Answer Key

## Candidate

- id: `deno-compile-graph-eszip-v2`
- fixture: `deno v2.3.0` at `61574bb9c9c255d5c661add6c7464af30475c197`
- status: validated

## Correct Answer

The compile command enters `compile`, validates the entrypoint resolution mode, and delegates to `compile_eszip`. That function constructs the module graph through `create_graph_with_options`, whose graph builder applies npm resolution/build options and lockfile state. The completed graph and compile metadata are then passed to the compile binary writer, which serializes the ESZip-backed executable.

An npm-specifier entrypoint using the unsupported resolution mode is rejected before packaging with `UnsupportedNpmSpecifierEntrypointResolutionWay`; it must not be described as a failure discovered only by the final writer.

## Canonical Evidence

1. `cli/main.rs:142` dispatches to the compile command.
2. `cli/tools/compile.rs:31-121` defines `compile` and the entrypoint validation, including `UnsupportedNpmSpecifierEntrypointResolutionWay`.
3. `cli/tools/compile.rs:158-263` defines `compile_eszip` and the final ESZip write flow.
4. `cli/graph_util.rs:495-503` provides `create_graph_with_options`.
5. `cli/graph_util.rs:788-833` provides `build_graph_with_npm_resolution_and_build_options`, including npm and lockfile handling.
6. `cli/factory.rs:1157-1185` provides `create_compile_binary_writer`.

## Pinned Facts

- The flow starts at command dispatch and enters `compile` before `compile_eszip`.
- `compile_eszip` obtains a module graph through `create_graph_with_options` and the npm/lockfile-aware graph builder.
- The graph is handed to `create_compile_binary_writer` and written as an ESZip-backed executable.
- Unsupported npm entrypoint resolution is rejected before graph packaging by `UnsupportedNpmSpecifierEntrypointResolutionWay`.
- A complete answer connects these stages in order; citing only the final `write_all` is insufficient.
