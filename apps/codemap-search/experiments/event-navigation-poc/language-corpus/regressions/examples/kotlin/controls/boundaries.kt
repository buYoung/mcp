class Router {
    private var stored: () -> Unit = {}
    private var secondary: () -> Unit = {}
    val callback: () -> Unit
        get() = stored
    val other: () -> Unit
        get() = secondary
    fun install(f: () -> Unit, g: () -> Unit) {
        stored = f // @S1
        secondary = g // @S2
    }
    fun fire() { callback() } // @I1
    fun fireOther() { other() } // @I2
}
