(assignment_expression
  left: (subscript_expression
    argument: (_) @collection.expression)) @collection.write

(return_statement
  (subscript_expression
    argument: (_) @collection.expression)) @collection.read
