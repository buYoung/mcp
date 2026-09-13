(namespace_declaration name: (_) @symbol.name) @symbol.mod
(file_scoped_namespace_declaration name: (_) @symbol.name) @symbol.mod

(class_declaration name: (identifier) @symbol.name) @symbol.class
(record_declaration name: (identifier) @symbol.name) @symbol.record
(struct_declaration name: (identifier) @symbol.name) @symbol.struct
(interface_declaration name: (identifier) @symbol.name) @symbol.interface
(enum_declaration name: (identifier) @symbol.name) @symbol.enum
(delegate_declaration name: (identifier) @symbol.name) @symbol.type

(enum_member_declaration name: (identifier) @symbol.name) @symbol.variant
(constructor_declaration name: (identifier) @symbol.name) @symbol.method
(method_declaration name: (identifier) @symbol.name) @symbol.method
(operator_declaration operator: _ @symbol.name) @symbol.method
(property_declaration name: (identifier) @symbol.name) @symbol.property
(event_declaration name: (identifier) @symbol.name) @symbol.field

(field_declaration
  (variable_declaration
    (variable_declarator name: (identifier) @symbol.name))) @symbol.field

(event_field_declaration
  (variable_declaration
    (variable_declarator name: (identifier) @symbol.name))) @symbol.field

(string_literal) @literal.string
