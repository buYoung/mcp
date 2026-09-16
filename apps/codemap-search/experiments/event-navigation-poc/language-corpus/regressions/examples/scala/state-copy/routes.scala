package routes
case class State(items: Array[() => Unit], other: Array[() => Unit]) {
  def refreshed: State = copy(other = Array.empty[() => Unit])
}
class Cell(initial: State) {
  private var ref: State = initial
  def read(): State = ref
  def replace(next: State): Unit = { ref = next }
}
class Router {
  val cell = new Cell(State(Array.empty[() => Unit], Array.empty[() => Unit]))
  def install(callback: () => Unit): Unit = {
    val update = State(Array(callback), Array.empty[() => Unit]) // @S1
    cell.replace(update)
  }
  def fire(): Unit = { cell.read().items(0)() } // @I1
  def other(): Unit = { cell.read().other(0)() } // @I2
  def copies(callback: () => Unit): Unit = {
    val original = State(Array(callback), Array.empty[() => Unit]) // @S2
    val kept = original.refreshed
    kept.items(0)() // @I3
    kept.other(0)() // @I4
  }
  def replaced(callback: () => Unit): Unit = {
    val original = State(Array(callback), Array.empty[() => Unit]) // @S3
    val changed = original.copy(items = Array.empty[() => Unit])
    changed.items(0)() // @I5
    original.items(0)() // @I6
  }
}
