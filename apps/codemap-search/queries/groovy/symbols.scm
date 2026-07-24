(package_declaration
  [(identifier) (scoped_identifier)] @symbol.name) @symbol.mod
(class_declaration name: (identifier) @symbol.name) @symbol.class
(interface_declaration name: (identifier) @symbol.name) @symbol.interface
(annotation_type_declaration name: (identifier) @symbol.name) @symbol.interface
(record_declaration name: (identifier) @symbol.name) @symbol.record
(enum_declaration name: (identifier) @symbol.name) @symbol.enum
(enum_constant name: (identifier) @symbol.name) @symbol.variant
(method_declaration name: (identifier) @symbol.name) @symbol.method
(constructor_declaration name: (identifier) @symbol.name) @symbol.method
(compact_constructor_declaration name: (identifier) @symbol.name) @symbol.method
(field_declaration
  declarator: (variable_declarator name: (identifier) @symbol.name)) @symbol.field
(string_literal) @literal.string
