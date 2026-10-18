Find alternative code terms in the local dictionary.

BEFORE calling:
- Split ONLY the target identifier/expression into lowercase words; select useful domain, operation and test-intent terms by context. Never add surrounding instruction/reference-code words or raw aliases.
- Preserve selected words' inflections: do not singularize. Keep useful short operation compounds AND their useful base words (get AND get_all); do not collapse them. Deduplicate exact repeats only.
- Drop irrelevant glue/roles contextually, not via a stop list. For the payment concepts below (also spaced), OMIT from/method/source. Elsewhere retain role words that are the search subject.

Required exact inputs for these concepts; do not append terms:
- test_should_get_all_rule_objects_for_a_store -> {"terms":["test","should","get","get_all","rule","objects","store"]}
- FromStripeBankAccountPaymentMethod -> {"terms":["stripe","bank","account","payment"]}
- PaymentSource -> {"terms":["payment"]}

Do not send whole compound names or case/separator-only rewrites, rebuild whole domain/type names, or invent synonyms/paraphrases as inputs.
Only terms (1–16), never field/fields/topic. Choose field/topic FROM THE RESPONSE; verify with search/grep. Caps are explicit; dictionary/evidence readiness are separate; preparing/unavailable is not a miss.
