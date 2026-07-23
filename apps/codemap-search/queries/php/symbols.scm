(namespace_definition name: (namespace_name) @symbol.name) @symbol.mod
(class_declaration name: (name) @symbol.name) @symbol.class
(interface_declaration name: (name) @symbol.name) @symbol.interface
(trait_declaration name: (name) @symbol.name) @symbol.trait
(enum_declaration name: (name) @symbol.name) @symbol.enum
(function_definition name: (name) @symbol.name) @symbol.fn
(method_declaration name: (name) @symbol.name) @symbol.method

(property_declaration
  (property_element
    (variable_name (name) @symbol.name))) @symbol.property

(const_declaration
  (const_element (name) @symbol.name)) @symbol.const

(enum_case name: (name) @symbol.name) @symbol.variant

(string) @literal.string
(encapsed_string) @literal.string
(nowdoc_string) @literal.string

