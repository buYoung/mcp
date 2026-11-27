package nullable

class Gauge {
    fun level(): Int = 1
}

interface Sink {
    fun accept(item: String?): Boolean
}

// The parameter of the declaration and of its implementation is nullable.
class MemorySink : Sink {
    override fun accept(item: String?): Boolean = item != null
}

class Panel(private val first: Gauge?) {
    private var spare: Gauge? = null

    // The receivers are declared with a nullable type.
    fun safe(gauge: Gauge?): Int = gauge?.level() ?: 0

    fun asserted(gauge: Gauge?): Int = gauge!!.level()

    // The receiver is a property declared in the class body.
    fun stored(): Int = spare?.level() ?: 0

    // The receiver is a local value with a written type.
    fun local(): Int {
        val picked: Gauge? = spare
        return picked?.level() ?: 0
    }
}
