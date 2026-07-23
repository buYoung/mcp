import { Rule } from "./rules";

export class OrderPolicy {
  constructor(rules) {
    this.rules = rules;
  }

  evaluate(order) {
    const active = this.rules.filter((rule) => rule.check(order));
    const names = active.map((rule) => {
      const { name: ruleName, severity = "low", ...metadata } = rule;
      return `${ruleName}:${severity}:${Object.keys(metadata).length}`;
    });
    return names;
  }
}
