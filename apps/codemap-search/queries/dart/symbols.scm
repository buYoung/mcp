(class_declaration name: (identifier) @symbol.name) @symbol.class
(mixin_declaration (identifier) @symbol.name) @symbol.class
(extension_declaration name: (identifier) @symbol.name) @symbol.class
(extension_type_declaration
  name: (extension_type_name (identifier) @symbol.name)) @symbol.type
(enum_declaration name: (identifier) @symbol.name) @symbol.enum
(enum_constant name: (identifier) @symbol.name) @symbol.variant
(type_alias (type_identifier) @symbol.name) @symbol.type

(function_declaration
  signature: (function_signature name: (identifier) @symbol.name)) @symbol.fn
(method_signature
  (function_signature name: (identifier) @symbol.name)) @symbol.method
(constructor_signature name: (identifier) @symbol.name) @symbol.method
(constant_constructor_signature (identifier) @symbol.name) @symbol.method
(factory_constructor_signature (identifier) @symbol.name) @symbol.method
(redirecting_factory_constructor_signature (identifier) @symbol.name) @symbol.method

(top_level_variable_declaration
  (initialized_identifier_list
    (initialized_identifier name: (identifier) @symbol.name))) @symbol.field
(initialized_variable_definition
  name: (identifier) @symbol.name) @symbol.field

(string_literal) @literal.string
