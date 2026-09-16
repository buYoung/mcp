package sample
class Listener { def run(): Unit = () }
class Cell[A](private val item: A) { def read(): A = item }
trait Builder[A, R] { def build(value: A): R }
object Builder {
  implicit def reference[A <: AnyRef]: Builder[A, Cell[A]] = new Builder[A, Cell[A]] {
    def build(value: A): Cell[A] = new Cell(value) // @STORE
  }
}
object Factory {
  def make[A, R](value: A)(implicit builder: Builder[A, R]): R = builder.build(value)
}
class Routes {
  def run(listener: Listener, other: Listener): Unit = {
    val cell = Factory.make(listener)
    val separate = Factory.make(other)
    cell.read().run() // @READ
  }
}
