class Router {
  var callbacks: (() => Unit, () => Unit) = (() => (), () => ())
  def install(f: () => Unit, g: () => Unit): Unit = {
    callbacks = (f, g) // @S1
  }
  def first(): Unit = {
    val (callback, _) = callbacks
    callback() // @I1
  }
  def second(): Unit = {
    val (_, callback) = callbacks
    callback() // @I2
  }
}
