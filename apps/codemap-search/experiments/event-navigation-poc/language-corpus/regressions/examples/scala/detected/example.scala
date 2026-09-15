class Router {
  var cb: () => Unit = () => ()
  def install(f: () => Unit): Unit = { cb = f } // S
  def fire(): Unit = { cb() } // I
}
