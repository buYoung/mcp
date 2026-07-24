(function_statement
  (function_name) @symbol.name) @symbol.ps_function
(class_statement
  (simple_name) @symbol.name) @symbol.class
(enum_statement
  (simple_name) @symbol.name) @symbol.enum
(class_method_definition
  (simple_name) @symbol.name) @symbol.method
(class_property_definition
  (variable) @symbol.name) @symbol.property
(enum_member
  (simple_name) @symbol.name) @symbol.variant
(statement_list
  (pipeline
    (assignment_expression
      (left_assignment_expression) @symbol.name))) @symbol.variable
(string_literal) @literal.string
