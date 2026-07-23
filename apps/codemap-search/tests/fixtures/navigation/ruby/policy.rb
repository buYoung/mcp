require_relative "rule"
def allow(rule)
  rule.check()
end

