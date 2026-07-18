(
  (call_expression
    function: (member_expression
      object: (member_expression
        object: (_) @collection.owner
        property: (property_identifier) @collection.field)
      property: (property_identifier) @collection.operation)
    arguments: (arguments (_)) @collection.arguments) @collection.push
  (#eq? @collection.operation "push")
)

(for_in_statement
  right: (member_expression
    object: (_) @collection.owner
    property: (property_identifier) @collection.field)) @collection.iteration

(return_statement
  (member_expression
    object: (_) @collection.owner
    property: (property_identifier) @collection.field)) @collection.read

(subscript_expression
  object: (member_expression
    object: (_) @collection.owner
    property: (property_identifier) @collection.field)) @collection.read
