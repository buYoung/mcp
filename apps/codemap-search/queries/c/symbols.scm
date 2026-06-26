
;; Function definitions — name is the innermost identifier inside the declarator chain.
;; The `@symbol.cfn` capture signals the C-specific extract arm to do the name walk.
(function_definition) @symbol.cfn

;; Function prototypes in headers: `declaration` whose declarator contains a
;; function_declarator. Same `@symbol.cfn` arm handles the name extraction.
(declaration
  declarator: (function_declarator)) @symbol.cfn

;; Structs, unions, enums with a name (skip anonymous: no `name` field).
(struct_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

(union_specifier
  name: (type_identifier) @symbol.name) @symbol.struct

(enum_specifier
  name: (type_identifier) @symbol.name) @symbol.enum

;; Enum constants (enumerators).
(enumerator
  name: (identifier) @symbol.name) @symbol.variant

;; typedef — simple alias: `typedef struct {...} Point` has type_identifier as declarator.
(type_definition
  declarator: (type_identifier) @symbol.name) @symbol.type

;; typedef function-pointer: `typedef int (*cb)(void)` has function_declarator as declarator.
;; The @symbol.cfn arm walks the declarator chain to dig out the type_identifier name.
(type_definition
  declarator: (function_declarator)) @symbol.cfn

;; Object-like macros (#define NAME value) → const.
(preproc_def
  name: (identifier) @symbol.name) @symbol.const

;; Function-like macros (#define NAME(args) body) → fn.
(preproc_function_def
  name: (identifier) @symbol.name) @symbol.fn

;; String literals for BM25 index.
(string_literal) @literal.string
