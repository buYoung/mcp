#include "policy.h"

int evaluate_policy(struct order *order, struct rule *rules, int count) {
    struct failure_list failures = {0};
    int passed = 1;

    for (int i = 0; i < count; i++) {
        if (!rule_check(&rules[i], order)) {
            append_failure(&failures, rules[i].name);
            passed = 0;
        }
    }

    return passed && failure_count(&failures) == 0;
}
