(call_expression
  function: (field_expression
    value: (_) @collection.expression
    field: (field_identifier) @collection.operation)
  arguments: (arguments)) @collection.write

(for_expression
  value: (_) @collection.expression) @collection.read

(return_expression
  (_) @collection.expression) @collection.read

(index_expression
  . (_) @collection.expression) @collection.read
