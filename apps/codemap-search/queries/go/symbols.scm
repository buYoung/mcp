
;; Functions and methods
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn
(method_declaration
  name: (field_identifier) @symbol.name) @symbol.fn

;; Named types (struct / interface / alias resolved in code)
(type_spec
  name: (type_identifier) @symbol.name) @symbol.gotype
(type_alias
  name: (type_identifier) @symbol.name) @symbol.type

;; Struct fields
(field_declaration
  name: (field_identifier) @symbol.name) @symbol.field

;; Interface methods
(method_elem
  name: (field_identifier) @symbol.name) @symbol.fn

;; Package-level constants and variables
(const_spec
  name: (identifier) @symbol.name) @symbol.const
(var_spec
  name: (identifier) @symbol.name) @symbol.variable

;; Literals (only strings are kept downstream)
(interpreted_string_literal) @literal.string
(raw_string_literal) @literal.string
