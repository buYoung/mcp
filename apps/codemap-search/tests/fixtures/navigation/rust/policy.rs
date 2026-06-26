use std::collections::HashMap;

pub struct Policy<R> {
    rules: Vec<R>,
}

impl<R> Policy<R>
where
    R: Rule,
{
    pub fn evaluate(&self, order: &Order) -> HashMap<String, bool> {
        let failures: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| !rule.check(order))
            .collect();

        if failures.is_empty() {
            return HashMap::new();
        }

        let mut outcomes = HashMap::new();
        for failure in &failures {
            outcomes.insert(failure.name().to_string(), false);
        }
        outcomes
    }
}
