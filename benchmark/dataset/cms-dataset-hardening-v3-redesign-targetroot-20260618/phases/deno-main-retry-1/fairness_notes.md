# Fairness Notes

- Confirmed: the public question is symptom-first and business-realistic. It is framed as a release-packaging problem, not as a source-navigation puzzle.
- Confirmed: the public question avoids file paths, symbol names, internal helper names, exact subsystem names, and checklist-shaped wording.
- Confirmed: the prompt does not expose `bundle`, `worker`, `snapshot`, `node_modules`, `package.json`, or the decisive fallback function name.
- Confirmed: the obvious answers are wrong, not merely partial. "The helper is just included in the artifact" and "the nearest manifest decides everything" both fail to explain the repo-local rescue path.
- Confirmed: the answer is statically pinned by `cli/tools/bundle/mod.rs`, `libs/resolver/graph.rs`, and the contrast note in `cli/tools/compile.rs`.
- Confirmed: the asked consequence about "the wrong installed version" is in-scope because the source explicitly documents the name-only multi-version limitation.
- Inference: the phrase "nearby manifest" is a weak fairness hint that the local package boundary matters, but it does not reveal the rescue mechanism or the search token that jumps directly to it.
