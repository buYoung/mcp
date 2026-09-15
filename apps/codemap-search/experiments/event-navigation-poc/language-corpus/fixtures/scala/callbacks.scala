class Router {
  var primary: () => Unit = () => ()
  var secondary: () => Unit = () => ()
  def setPrimary(cb: () => Unit): Unit = { this.primary = cb } // store_primary
  def setSecondary(cb: () => Unit): Unit = { this.secondary = cb } // store_secondary
  def firePrimary(): Unit = { primary() } // call_primary
  def fireSecondary(): Unit = { secondary() } // call_secondary
}
class Other {
  var primary: () => Unit = () => ()
  def firePrimary(): Unit = { primary() } // call_other
}
