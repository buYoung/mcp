import com.example.Rule

class OrderPolicy(private val rules: List<Rule>) {
    fun evaluate(order: Order): List<String> {
        val failures = rules.filter { rule -> !rule.check(order) }
        val names = failures.map { rule -> rule.name }
        if (failures.isEmpty()) {
            return emptyList()
        }
        return names
    }
}
