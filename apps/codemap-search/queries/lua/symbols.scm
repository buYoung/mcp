(function_declaration
  name: (identifier) @symbol.name) @symbol.fn

(function_declaration
  name: (dot_index_expression field: (identifier) @symbol.name)) @symbol.method

(function_declaration
  name: (method_index_expression method: (identifier) @symbol.name)) @symbol.method

(assignment_statement
  (expression_list
    value: (function_definition) @symbol.fn))

(assignment_statement
  (variable_list name: (identifier) @symbol.name)) @symbol.variable

(table_constructor
  (field name: (identifier) @symbol.name) @symbol.field)

(string) @literal.string
