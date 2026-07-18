
(struct_item
  name: (type_identifier) @name) @definition.struct

(enum_item
  name: (type_identifier) @name) @definition.enum

(union_item
  name: (type_identifier) @name) @definition.union

(trait_item
  name: (type_identifier) @name) @definition.trait

(function_item
  name: (identifier) @name) @definition.function

(macro_definition
  name: (identifier) @name) @definition.macro

(call_expression) @reference.call
(macro_invocation) @reference.call
