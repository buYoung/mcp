
;; Structs
(struct_item
  name: (type_identifier) @symbol.name) @symbol.struct

;; Enums
(enum_item
  name: (type_identifier) @symbol.name) @symbol.enum

;; Enum Variants — error/state variants ("TxReadonly") are the names agents search
;; for; without them an error enum's file is unreachable via symbol search.
(enum_variant
  name: (identifier) @symbol.name) @symbol.variant

;; Traits
(trait_item
  name: (type_identifier) @symbol.name) @symbol.trait

;; Modules
(mod_item
  name: (identifier) @symbol.name) @symbol.mod

;; Functions and Methods
(function_item
  name: (identifier) @symbol.name) @symbol.fn

;; Type Aliases
(type_item
  name: (type_identifier) @symbol.name) @symbol.type

;; Constants
(const_item
  name: (identifier) @symbol.name) @symbol.const

;; Statics
(static_item
  name: (identifier) @symbol.name) @symbol.static

;; Struct Fields
(field_declaration
  name: (field_identifier) @symbol.name) @symbol.field

;; Literals
(string_literal) @literal.string
(raw_string_literal) @literal.string
(integer_literal) @literal.number
(float_literal) @literal.number
(boolean_literal) @literal.boolean
