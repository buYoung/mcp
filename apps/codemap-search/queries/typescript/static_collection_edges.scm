;; Statically identifiable TypeScript/JavaScript collection writes and reads.
;; Only direct, named member chains are accepted. Computed properties and aliases are
;; deliberately excluded because they cannot be joined reliably without runtime information.
(
  (call_expression
    function: (member_expression
      object: (member_expression
        object: (_) @collection.owner
        property: [(property_identifier) (private_property_identifier)] @collection.field)
      property: (property_identifier) @collection.operation)
    arguments: (arguments (_)) @collection.arguments) @collection.push
  (#eq? @collection.operation "push")
)

(for_in_statement
  right: (member_expression
    object: (_) @collection.owner
    property: [(property_identifier) (private_property_identifier)] @collection.field)) @collection.iteration

(return_statement
  (member_expression
    object: (_) @collection.owner
    property: [(property_identifier) (private_property_identifier)] @collection.field)) @collection.read

(subscript_expression
  object: (member_expression
    object: (_) @collection.owner
    property: [(property_identifier) (private_property_identifier)] @collection.field)) @collection.read
