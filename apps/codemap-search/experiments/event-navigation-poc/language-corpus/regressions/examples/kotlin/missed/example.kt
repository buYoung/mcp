class Router {
    private var stored: () -> Unit = {}
    val callback: () -> Unit
        get() = stored
    fun install(f: () -> Unit) { stored = f } // S
    fun fire() { callback() } // I
}
