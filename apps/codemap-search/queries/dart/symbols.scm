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
(external_function_declaration
  signature: (function_signature name: (identifier) @symbol.name)) @symbol.fn
(method_declaration
  signature: (method_signature
    (function_signature name: (identifier) @symbol.name))) @symbol.method
(method_declaration
  signature: (method_signature
    (constructor_signature name: (identifier) @symbol.name))) @symbol.method
(method_declaration
  signature: (method_signature
    (factory_constructor_signature (identifier) @symbol.name))) @symbol.method
(class_member
  (declaration
    (function_signature name: (identifier) @symbol.name))) @symbol.method
(class_member
  (declaration
    (constructor_signature name: (identifier) @symbol.name))) @symbol.method
(class_member
  (declaration
    (constant_constructor_signature (identifier) @symbol.name))) @symbol.method
(class_member
  (declaration
    (factory_constructor_signature (identifier) @symbol.name))) @symbol.method
(class_member
  (declaration
    (redirecting_factory_constructor_signature (identifier) @symbol.name))) @symbol.method

(getter_declaration
  signature: (getter_signature name: (identifier) @symbol.name)) @symbol.property
(setter_declaration
  signature: (setter_signature name: (identifier) @symbol.name)) @symbol.property
(method_declaration
  signature: (method_signature
    [(getter_signature name: (identifier) @symbol.name)
     (setter_signature name: (identifier) @symbol.name)])) @symbol.property
(class_member
  (declaration
    [(getter_signature name: (identifier) @symbol.name)
     (setter_signature name: (identifier) @symbol.name)])) @symbol.property
(method_declaration
  signature: (method_signature
    (operator_signature operator: _ @symbol.name))) @symbol.method
(class_member
  (declaration
    (operator_signature operator: _ @symbol.name))) @symbol.method

(top_level_variable_declaration
  (initialized_identifier_list
    (initialized_identifier name: (identifier) @symbol.name))) @symbol.field
(initialized_variable_definition
  name: (identifier) @symbol.name) @symbol.field

(top_level_variable_declaration
  "const"
  (static_final_declaration_list
    (static_final_declaration name: (identifier) @symbol.name) @symbol.const))
(declaration
  "const"
  (static_final_declaration_list
    (static_final_declaration name: (identifier) @symbol.name) @symbol.const))

(string_literal) @literal.string
