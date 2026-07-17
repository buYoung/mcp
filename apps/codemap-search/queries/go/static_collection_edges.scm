(call_expression
  function: (identifier) @collection.operation
  arguments: (argument_list . (_) @collection.expression (_))) @collection.write

(range_clause
  right: (_) @collection.expression) @collection.read

(return_statement
  (expression_list . (_) @collection.expression)) @collection.read

(index_expression
  operand: (_) @collection.expression) @collection.read
