package typedlocal

class Meter {
    fun read(): Int = 1
}

class Dial {
    fun read(): Int = 2
}

class Station(private val first: Meter) {
    private val stored: Meter = Meter()

    // The receiver is a property declared in the class body.
    fun fromProperty(): Int = stored.read()

    // The receiver is a local value with a written type.
    fun fromLocal(): Int {
        val picked: Meter = first
        return picked.read()
    }

    // The local value hides the property and has no written type.
    fun hidden(): Int {
        val stored = Dial()
        return stored.read()
    }
}
