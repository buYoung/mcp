local rule_module = require("./rule")
function allow(rule)
  return rule.check()
end

