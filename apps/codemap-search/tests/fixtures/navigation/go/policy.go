package checkout

import "errors"

type Rule interface {
	Check(OrderDTO) bool
}

func Evaluate(order OrderDTO, rules []Rule) error {
	failures := []string{}
	for _, rule := range rules {
		if !rule.Check(order) {
			failures = append(failures, "failed")
		}
	}
	if len(failures) > 0 {
		return errors.New("policy failed")
	}
	return nil
}
