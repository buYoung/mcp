class Router {
    var primary: (() -> Unit)? = null
    var secondary: (() -> Unit)? = null
    fun setPrimary(cb: () -> Unit) { this.primary = cb } // store_primary
    fun setSecondary(cb: () -> Unit) { this.secondary = cb } // store_secondary
    fun firePrimary() { primary?.invoke() } // call_primary
    fun fireSecondary() { secondary?.invoke() } // call_secondary
}
class Other {
    var primary: (() -> Unit)? = null
    fun firePrimary() { primary?.invoke() } // call_other
}
