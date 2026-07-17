(method_invocation
  object: (_) @collection.expression
  name: (identifier) @collection.operation
  arguments: (argument_list)) @collection.write

(enhanced_for_statement
  value: (_) @collection.expression) @collection.read

(return_statement
  (_) @collection.expression) @collection.read

(array_access
  array: (_) @collection.expression) @collection.read
