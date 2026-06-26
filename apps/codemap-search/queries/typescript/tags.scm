
(class_declaration
  name: (type_identifier) @name) @definition.class

(function_declaration
  name: (identifier) @name) @definition.function

(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @name) @definition.method

(call_expression) @reference.call
