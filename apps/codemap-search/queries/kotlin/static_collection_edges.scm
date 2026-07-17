(call_expression
  (navigation_expression
    (_) @collection.expression
    (identifier) @collection.operation)
  (value_arguments)) @collection.write

(for_statement
  (variable_declaration)
  (_) @collection.expression
  (block)) @collection.read

(return_expression
  (_) @collection.expression) @collection.read

(index_expression
  . (_) @collection.expression) @collection.read
