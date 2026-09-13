(package_clause name: (package_identifier) @symbol.name) @symbol.mod
(class_definition name: (identifier) @symbol.name) @symbol.class
(trait_definition name: (identifier) @symbol.name) @symbol.trait
(object_definition name: (identifier) @symbol.name) @symbol.object
(enum_definition name: (identifier) @symbol.name) @symbol.enum
(simple_enum_case name: (identifier) @symbol.name) @symbol.variant
(full_enum_case name: (identifier) @symbol.name) @symbol.variant
(function_definition name: [(identifier) (operator_identifier)] @symbol.name) @symbol.fn
(function_declaration name: [(identifier) (operator_identifier)] @symbol.name) @symbol.fn
(given_definition name: (identifier) @symbol.name) @symbol.variable
(type_definition name: (type_identifier) @symbol.name) @symbol.type
(val_definition pattern: (identifier) @symbol.name) @symbol.const
(var_definition pattern: (identifier) @symbol.name) @symbol.variable
(val_declaration name: (identifier) @symbol.name) @symbol.const
(var_declaration name: (identifier) @symbol.name) @symbol.variable
(extension_definition) @symbol.scala_extension
(string) @literal.string
