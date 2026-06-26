
;; Type declarations
(class_declaration
  name: (identifier) @symbol.name) @symbol.class
(interface_declaration
  name: (identifier) @symbol.name) @symbol.interface
(enum_declaration
  name: (identifier) @symbol.name) @symbol.enum
(record_declaration
  name: (identifier) @symbol.name) @symbol.record

;; Enum constants
(enum_constant
  name: (identifier) @symbol.name) @symbol.variant

;; Methods and constructors
(method_declaration
  name: (identifier) @symbol.name) @symbol.method
(constructor_declaration
  name: (identifier) @symbol.name) @symbol.method

;; Fields
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @symbol.name)) @symbol.field

;; Literals
(string_literal) @literal.string
