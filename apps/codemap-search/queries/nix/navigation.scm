(apply_expression) @nav.call
(apply_expression) @nav.import

(formal) @local.scope
(function_expression
  universal: (identifier) @local.scope)

(variable_expression) @local.reference
(select_expression) @local.reference
(inherit) @local.reference
(inherit_from) @local.reference
