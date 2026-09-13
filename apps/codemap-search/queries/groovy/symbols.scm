(package_declaration
  [(identifier) (scoped_identifier)] @symbol.name) @symbol.mod
(class_declaration name: (identifier) @symbol.name) @symbol.class
(interface_declaration name: (identifier) @symbol.name) @symbol.interface
(annotation_type_declaration name: (identifier) @symbol.name) @symbol.interface
(record_declaration name: (identifier) @symbol.name) @symbol.record
(enum_declaration name: (identifier) @symbol.name) @symbol.enum
(enum_constant name: (identifier) @symbol.name) @symbol.variant
(method_declaration name: (_) @symbol.name) @symbol.method
(annotation_type_element_declaration name: (_) @symbol.name) @symbol.method
(function_definition name: (_) @symbol.name) @symbol.fn
; The grammar can recover a quoted method as a constructor named `def`.
((constructor_declaration name: (identifier) @symbol.name) @symbol.method
 (#not-eq? @symbol.name "def"))
(compact_constructor_declaration name: (identifier) @symbol.name) @symbol.method
(field_declaration
  declarator: (variable_declarator name: (identifier) @symbol.name)) @symbol.field
(string_literal) @literal.string
