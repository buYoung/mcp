
(class_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(method_declaration
  name: (identifier) @name) @definition.method

(method_invocation) @reference.call

(type_list
  (_) @reference.implementation)

(object_creation_expression
  type: (_) @reference.class)

(superclass
  (_) @reference.class)
