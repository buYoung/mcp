
(call_expression) @nav.call
(macro_invocation) @nav.call
(use_declaration) @nav.import
(let_declaration) @local.scope
(let_condition
  pattern: (_) @local.scope)
(for_expression
  pattern: (_) @local.scope)
(match_arm
  pattern: (_) @local.scope)
