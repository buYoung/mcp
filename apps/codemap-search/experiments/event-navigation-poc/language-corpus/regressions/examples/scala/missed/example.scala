class Router {
  var callbacks: (() => Unit, () => Unit) = (() => (), () => ())
  def install(f: () => Unit): Unit = {
    callbacks = (f, () => ()) // S
  }
  def fire(): Unit = {
    val (first, _) = callbacks
    first() // I
  }
}
