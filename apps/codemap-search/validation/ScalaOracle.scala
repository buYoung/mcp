import java.nio.file.{Files, Paths}
import java.nio.charset.StandardCharsets
import java.util.Base64
import scala.meta._

// Parse with Scalameta only. No compiler phases or repository code evaluation.
object ScalaOracle {
  def main(args: Array[String]): Unit = {
    val text = Files.readString(Paths.get(args(0)), StandardCharsets.UTF_8)
    val input = Input.VirtualFile(args(0), text)
    val attempts = List(dialects.Scala213, dialects.Scala3).map(d => (d, d(input).parse[Source]))
    val (dialect, parsed) = attempts.find(_._2.isInstanceOf[Parsed.Success[_]])
      .getOrElse(throw new IllegalArgumentException(attempts.map(_._2.toString).mkString("\n")))
    val tree = parsed.get
    println("Scalameta " + dialect.toString + "\t4.17.3")
    def visit(node: Tree, depth: Int): Unit = {
      val name = node match {
        case value: Defn.Def => Some(value.name)
        case value: Decl.Def => Some(value.name)
        case value: Defn.Macro => Some(value.name)
        case _ => None
      }
      if (name.nonEmpty && depth == 0) {
        val encoded = Base64.getEncoder.encodeToString(name.get.value.getBytes(StandardCharsets.UTF_8))
        val nativeEnd = node.pos.endLine + (if (node.pos.endColumn == 0) 0 else 1)
        // Definition ranges end at the final code token. Scalameta may include
        // trailing indented comments in a Scala 3 declaration's position.
        val last = node.tokens.reverseIterator.find(token =>
          token.end > token.start && !token.isInstanceOf[scala.meta.tokens.Token.Trivia])
        val end = last.map(token => token.pos.endLine + (if (token.pos.endColumn == 0) 0 else 1)).getOrElse(nativeEnd)
        println(encoded + "\t" + (node.pos.startLine + 1) + "\t" + (name.get.pos.startLine + 1) + "\t" + end + "\t" + nativeEnd)
      }
      val nextDepth = depth + (if (name.nonEmpty || node.isInstanceOf[Term.Function]) 1 else 0)
      node.children.foreach(visit(_, nextDepth))
    }
    visit(tree, 0)
  }
}
