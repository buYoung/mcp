(method_definition) @definition.method

[
  (class)
  (class_declaration)
] @definition.class

[
  (function_expression)
  (function_declaration)
  (generator_function)
  (generator_function_declaration)
] @definition.function

(variable_declarator
  value: [
    (arrow_function)
    (function_expression)
  ]) @definition.function

(assignment_expression
  right: [
    (arrow_function)
    (function_expression)
  ]) @definition.function

(pair
  value: [
    (arrow_function)
    (function_expression)
  ]) @definition.function

(call_expression) @reference.call
(new_expression) @reference.class
