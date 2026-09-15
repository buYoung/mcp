class Router {
    var cb: (() -> Unit)? = null
    fun install(f: () -> Unit) { cb = f } // S
    fun fire() { cb?.invoke() } // I
}
