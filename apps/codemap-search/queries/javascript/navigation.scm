(call_expression) @nav.call
(new_expression) @nav.call
(import_statement) @nav.import
(variable_declarator) @local.scope
(catch_clause
  parameter: (_) @local.scope)
(for_in_statement
  kind: "const"
  left: (_) @local.scope)
(for_in_statement
  kind: "let"
  left: (_) @local.scope)
(for_in_statement
  kind: "var"
  left: (_) @local.scope)
