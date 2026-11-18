package generic

abstract class Codec<T> {
    abstract fun decode(raw: String): T

    fun width(): Int = 1
}

class TextCodec : Codec<String>() {
    override fun decode(raw: String): String = raw.trim()
}

// The receiver's declared type carries a type argument.
fun measure(codec: Codec<String>): Int = codec.width()

fun parse(codec: Codec<String>): String = codec.decode("a")
