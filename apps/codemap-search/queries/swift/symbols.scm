(class_declaration
  name: (type_identifier) @symbol.name) @symbol.swift_type
(class_declaration
  name: (user_type) @symbol.name) @symbol.swift_type

(protocol_declaration
  name: (type_identifier) @symbol.name) @symbol.interface

(typealias_declaration
  name: (type_identifier) @symbol.name) @symbol.type

(function_declaration
  "func" name: _ @symbol.name "(") @symbol.fn

(protocol_function_declaration
  "func" name: _ @symbol.name "(") @symbol.method

(init_declaration
  name: "init" @symbol.name) @symbol.method

(property_declaration
  name: (pattern (simple_identifier) @symbol.name)) @symbol.property

(protocol_property_declaration
  name: (pattern (simple_identifier) @symbol.name)) @symbol.property

(enum_entry
  name: (simple_identifier) @symbol.name) @symbol.variant

(line_string_literal) @literal.string
(multi_line_string_literal) @literal.string
