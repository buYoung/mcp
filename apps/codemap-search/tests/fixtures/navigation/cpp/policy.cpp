#include "policy.hpp"
#include <vector>

class OrderPolicy {
public:
    std::vector<std::string> evaluate(const Order &order) const {
        std::vector<std::string> failures;
        for (const auto &rule : rules_) {
            if (!rule.check(order)) {
                failures.push_back(rule.name());
            }
        }
        if (failures.empty()) {
            return {};
        }
        return failures;
    }

private:
    std::vector<Rule> rules_;
};
