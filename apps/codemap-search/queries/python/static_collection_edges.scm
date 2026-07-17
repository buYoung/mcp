(call
  function: (attribute
    object: (_) @collection.expression
    attribute: (identifier) @collection.operation)
  arguments: (argument_list (_))) @collection.write

(for_statement
  right: (_) @collection.expression) @collection.read

(return_statement
  (_) @collection.expression) @collection.read

(subscript
  value: (_) @collection.expression) @collection.read
