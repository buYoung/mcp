import demo.Helper

class ScalaFlow {
  def targetScala(): Unit = ()
  def callerScala(): Unit = targetScala()
}
