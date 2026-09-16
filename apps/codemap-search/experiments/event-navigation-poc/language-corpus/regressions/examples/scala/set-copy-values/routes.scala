class Listener { def run(): Unit = () }
class Routes {
  def route(existing: Set[Listener], fresh: Listener): Unit = {
    val copied = existing + fresh // @STORE
    val snapshot = copied.toArray
    snapshot(0).run() // @READ
  }
}
