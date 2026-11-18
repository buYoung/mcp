package partial

class EmailNotifier : Notifier {
    override fun send(count: Int): Boolean = count > 0
}

// A generic type implements the declaration like any other type.
class BufferedNotifier<T>(private val pending: List<T>) : Notifier {
    override fun send(count: Int): Boolean = pending.size >= count
}
