import demo.Helper

class Worker {
  val name = "scala"

  @deprecated("old", "1")
  def run(): Unit = Helper.start()
}
