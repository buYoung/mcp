
(class_declaration
  name: (type_identifier) @name) @definition.class

(abstract_class_declaration
  name: (type_identifier) @name) @definition.class

(function_declaration
  name: (identifier) @name) @definition.function

(function_signature
  name: (identifier) @name) @definition.function

(method_definition
  name: [
    (property_identifier)
    (private_property_identifier)
  ] @name) @definition.method

(method_signature
  name: (property_identifier) @name) @definition.method

(abstract_method_signature
  name: (property_identifier) @name) @definition.method

(module
  name: (identifier) @name) @definition.module

(
  (type_annotation
    (_) @reference.type)
  (#not-match? @reference.type "^(\\{|\\(|infer[[:space:]]|any$|boolean$|bigint$|never$|number$|object$|string$|symbol$|undefined$|unknown$|void$|this$)")
)

(new_expression
  constructor: (_) @name) @reference.class

(call_expression) @reference.call
