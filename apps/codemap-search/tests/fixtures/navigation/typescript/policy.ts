import type { Rule } from "./rules";

type RuleResult<T> = T extends { name: infer Name } ? Name : never;

export class OrderPolicy<T extends Rule> {
  constructor(private readonly rules: readonly T[]) {}

  evaluate(order: Order): RuleResult<T>[] {
    const active = this.rules.filter((rule) => rule.check(order));
    const names = active.map((rule) => {
      const { name: ruleName, severity = "low", ...metadata } = rule as T & {
        severity?: string;
      };
      return `${ruleName}:${severity}:${Object.keys(metadata).length}`;
    });
    return names as RuleResult<T>[];
  }
}
