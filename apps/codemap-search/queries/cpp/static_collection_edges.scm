(call_expression
  function: (field_expression
    argument: (_) @collection.expression
    field: (field_identifier) @collection.operation)
  arguments: (argument_list)) @collection.write

(for_range_loop
  right: (_) @collection.expression) @collection.read

(return_statement
  (_) @collection.expression) @collection.read

(co_return_statement
  (_) @collection.expression) @collection.read

(subscript_expression
  argument: (_) @collection.expression) @collection.read
