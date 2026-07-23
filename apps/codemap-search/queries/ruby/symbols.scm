(class name: [(constant) (scope_resolution)] @symbol.name) @symbol.class
(module name: [(constant) (scope_resolution)] @symbol.name) @symbol.mod
(method name: (_) @symbol.name) @symbol.method
(singleton_method name: (_) @symbol.name) @symbol.method
(alias name: (_) @symbol.name) @symbol.alias

(assignment left: (constant) @symbol.name) @symbol.const

((call
  method: (identifier) @attribute_method) @symbol.property
 (#match? @attribute_method "^attr_(reader|writer|accessor)$"))

(string) @literal.string
