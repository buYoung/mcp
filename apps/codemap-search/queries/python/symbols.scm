
;; Class Definitions
(class_definition
  name: (identifier) @symbol.name) @symbol.class

;; Function and Method Definitions
(function_definition
  name: (identifier) @symbol.name) @symbol.fn

;; Assignments (Variables)
(assignment
  left: (identifier) @symbol.name) @symbol.variable

;; Literals
(string) @literal.string
(integer) @literal.number
(float) @literal.number
[(true) (false)] @literal.boolean
