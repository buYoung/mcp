import java.util.List;

class OrderPolicy {
    private final List<Rule> rules;

    OrderPolicy(List<Rule> rules) {
        this.rules = rules;
    }

    List<String> evaluate(Order order) {
        List<String> failures = rules.stream()
                .filter(rule -> !rule.check(order))
                .map(Rule::name)
                .toList();
        if (failures.isEmpty()) {
            return List.of();
        }
        return failures;
    }
}
