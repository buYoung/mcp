Find alternative code terms in the local dictionary. The only argument key is `terms`: an array of 1–16 selected strings. Do not use `query` as an argument key.

Read the full description and schema, not a clipped tool preview. For behavior discovery or cross-component flow tracing, apply the selection rules below, then call this tool once before the first search/grep, even when target names are already known. Choose context-relevant alternatives from the response and search for them alongside the original terms. If none fit, continue with the original terms instead of forcing unrelated alternatives.

BEFORE calling:
- Split ONLY a target identifier (camelCase, snake_case) into lowercase words; each selected word gets its own `terms` item. Select useful domain, operation and test-intent terms by context. Never add surrounding instruction/reference-code words or raw aliases.
- Keep a technical term from the request that names one concept as ONE spaced item (`event bus`, `abort signal`). The server looks the phrase up before its words; split words lose its sense (`bus` alone is also a hardware bus).
- Preserve selected words' inflections: do not singularize. Keep useful short operation compounds AND their useful base words (get AND get_all); do not collapse them. Deduplicate exact repeats only.
- Drop irrelevant glue/roles contextually, not via a stop list. For the payment concepts below (also spaced), OMIT from/method/source. Elsewhere retain role words that are the search subject.

Required exact inputs for these concepts, also when spaced; do not append terms or keep phrases:
- test_should_get_all_rule_objects_for_a_store -> {"terms":["test","should","get","get_all","rule","objects","store"]}
- FromStripeBankAccountPaymentMethod -> {"terms":["stripe","bank","account","payment"]}
- PaymentSource -> {"terms":["payment"]}

Do not send whole identifiers or their case/separator-only rewrites (`scrollRect`, `scroll rect`), or invent synonyms/paraphrases as inputs.
Only terms (1–16), never field/fields/topic. Choose field/topic FROM THE RESPONSE; verify with search/grep. With codebase evidence, senses with direct or near relations in this workspace's declarations come first: `code uses "…"` names a sense term found there, `no related words in code` marks senses without such relations, and `N in code` means the first N listed words appear there. Caps are explicit; dictionary/evidence readiness are separate; preparing/unavailable is not a miss.
