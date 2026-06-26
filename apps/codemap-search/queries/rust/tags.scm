
(struct_item
  name: (type_identifier) @name) @definition.struct

(enum_item
  name: (type_identifier) @name) @definition.enum

(trait_item
  name: (type_identifier) @name) @definition.trait

(function_item
  name: (identifier) @name) @definition.function

(call_expression) @reference.call
