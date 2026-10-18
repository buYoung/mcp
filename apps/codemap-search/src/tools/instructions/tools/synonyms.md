Find alternative words for selected semantic search terms. Perform semantic selection BEFORE calling this tool; do not delegate that selection to the server.

Split the target identifier/expression into lowercase words, then use the code context and search intent to select the useful domain, operation and test-intent terms. Preserve useful word forms such as objects. Keep a short operation compound when it adds meaning: get and get_all may both be useful. Remove context-irrelevant glue and structural role labels, not words from a universal stop list: test, should and get remain useful in the test-name example, while from/method and source are only wrappers in the payment examples.

Required input transformations:
- test_should_get_all_rule_objects_for_a_store -> {"terms":["test","should","get","get_all","rule","objects","store"]}
- FromStripeBankAccountPaymentMethod -> {"terms":["stripe","bank","account","payment"]}
- PaymentSource -> {"terms":["payment"]}

Do NOT use an unchanged compound identifier or a case/separator-only rewrite of its whole name as a shortcut for semantic selection. Do not pass full phrases such as "Stripe bank account payment method" or several raw aliases for the same concept. Short operation compounds are allowed; reconstructed whole domain/type names are not. Do not invent broader paraphrases or synonyms as inputs. Deduplicate identical selected terms across inputs; do not collapse useful distinct units such as get and get_all. Send 1–16 selected terms.

The only input property is terms; field, fields and topic are NOT input parameters. Choose the relevant field/topic FROM THE RESPONSE, then verify suggestions with search or grep. Answers come only from the local dictionary. Display caps are explicit. Dictionary readiness and codebase-evidence readiness are separate; preparing/unavailable is not a dictionary miss.
