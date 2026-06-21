# Self Check

- `rg -n "single distributable|generated output|neighboring source tree|nearby manifest|wrong installed version|overall release already brought"` over the target root returned only an unrelated generic TypeScript declaration hit for `generated output`; the decisive code comments were not surfaced directly.
- `rg -n "plain package import|packaging flow|quietly bind|own nearby manifest"` over the target root returned no hits.
- A token-overlap check between the public question and direct routing terms such as `bundle`, `worker`, `snapshot`, `node_modules`, `package.json`, `entrypoint`, and `npm` returned no shared direct-routing tokens from the public question text.
- Confirmed judgment: the candidate is not trivially enumerable by a single literal grep or simple bash set subtraction. Solving it still requires choosing between the release/npm packaging decoy and the deeper bundle-snapshot fallback route.
