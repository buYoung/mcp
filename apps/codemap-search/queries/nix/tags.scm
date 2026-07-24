((binding
  attrpath: (attrpath) @definition.binding)
 (#not-match? @definition.binding "\\$\\{"))

(inherit
  attrs: (inherited_attrs
    attr: (identifier) @definition.inherit))

(inherit_from
  attrs: (inherited_attrs
    attr: (identifier) @definition.inherit))

(variable_expression) @reference.variable

((select_expression) @reference.attribute
 (#not-match? @reference.attribute "\\$\\{"))
