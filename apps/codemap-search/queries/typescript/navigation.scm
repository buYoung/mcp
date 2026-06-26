
;; Runtime navigation observations. Rust derives call/import/local details from these
;; stable parent nodes so the same query compiles for TypeScript and TSX.
(call_expression) @nav.call
(import_statement) @nav.import
(variable_declarator
  name: (identifier) @local.definition) @local.scope
