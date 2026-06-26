
;; Classes / interfaces (disambiguated in code) and objects
(class_declaration
  name: (identifier) @symbol.name) @symbol.ktclass
(object_declaration
  name: (identifier) @symbol.name) @symbol.object

;; Enum entries
(enum_entry
  (identifier) @symbol.name) @symbol.variant

;; Functions
(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

;; Properties
(property_declaration
  (variable_declaration
    (identifier) @symbol.name)) @symbol.property

;; Type aliases
(type_alias
  (identifier) @symbol.name) @symbol.type

;; Literals
(string_literal) @literal.string
