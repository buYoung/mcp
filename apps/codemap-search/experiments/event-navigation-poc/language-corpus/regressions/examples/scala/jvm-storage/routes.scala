package sample
class Routes {
  def run(callback: () => Unit, decoy: () => Unit): Unit = {
    val left = new Cell(null)
    val right = new Cell(null)
    left.write(callback)
    right.write(decoy)
    left.read().asInstanceOf[() => Unit]() // @READ
    left.other().asInstanceOf[() => Unit]() // @OTHER
  }
}
class GuardRoutes {
  def run(callback: () => Unit): Unit = {
    val cell = new GuardCell()
    cell.writeFinal(callback)
    cell.readFinal().asInstanceOf[() => Unit]() // @FINAL_READ
    cell.writeWrong(callback)
    cell.readWrong().asInstanceOf[() => Unit]() // @WRONG_READ
  }
}
