
(class_declaration
  name: (identifier) @name) @definition.class

(object_declaration
  name: (identifier) @name) @definition.object

(function_declaration
  name: (identifier) @name) @definition.function

(call_expression) @reference.call

;; Constructor invocation denotes a superclass; a bare delegated type denotes an interface.
(delegation_specifier
  (constructor_invocation
    (user_type) @reference.class))

(delegation_specifier
  (user_type) @reference.implementation)
