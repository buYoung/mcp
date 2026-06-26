
(class_declaration
  name: (identifier) @name) @definition.class

(object_declaration
  name: (identifier) @name) @definition.object

(function_declaration
  name: (identifier) @name) @definition.function

(call_expression) @reference.call
