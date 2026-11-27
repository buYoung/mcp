package receiver

// The only indexed `appendLine`, a top-level function.
fun appendLine(text: String): String = text + "\n"

// The receiver is declared with a type that this repository does not declare.
fun writeTitle(builder: StringBuilder) {
    builder.appendLine("title")
}

// A package-qualified call names the function itself, so it stays listed.
fun writeBody(): String = receiver.appendLine("body")

// The only indexed `appendHeading`: it extends a type that this repository does not declare.
fun StringBuilder.appendHeading(): StringBuilder = append("heading")

// Member syntax on a value of the extended type reaches the extension function.
fun writeHeading(builder: StringBuilder) {
    builder.appendHeading()
}
