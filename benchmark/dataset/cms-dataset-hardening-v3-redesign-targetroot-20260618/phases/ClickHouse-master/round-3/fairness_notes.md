# Fairness Notes

- Confirmed: this round is materially different from round 1 and round 2 at the public-prompt level. It removes explicit optimizer language, operator-pair language, and node-mutation language, and asks only about an operational symptom: why unused fallback slots still matter.
- Confirmed: the public question does not expose dictionary function names, tuple syntax, optimizer names, pass names, `LowCardinality`, or the decisive search tokens.
- Confirmed: the obvious answer is false. "Nobody reads those slots, so they are dead" fails because the source still type-checks and can evaluate the whole fallback tuple before field extraction discards unselected elements.
- Confirmed: the private key is pinned by one runtime dictionary implementation site, one analyzer pass, and focused stateless regression tests.
- Confirmed: public-safe phrase probes for `sentinel junk`, `unused fallback slot`, `consumer only reads one returned field`, and `discarded fallback fields remain observable` produced no direct hits under the ClickHouse source root.
- Inferred: a strong solver may still infer that a tuple-valued enrichment primitive is involved, but the prompt does not reveal that this is specifically a dictionary lookup or a tuple-element optimizer guard.
