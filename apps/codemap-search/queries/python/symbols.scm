
;; Class Definitions
(class_definition
  name: (identifier) @symbol.name) @symbol.class

;; Function and Method Definitions
(function_definition
  name: (identifier) @symbol.name) @symbol.fn

;; File-level variables. Function-local assignments remain navigation bindings, not symbols.
(module
  (expression_statement
    (assignment
      left: (identifier) @symbol.name) @symbol.variable))

;; Class attributes remain searchable fields and retain their class owner.
(class_definition
  body: (block
    (expression_statement
      (assignment
        left: (identifier) @symbol.name)) @symbol.field))

;; Literals
(string) @literal.string
(integer) @literal.number
(float) @literal.number
[(true) (false)] @literal.boolean
