package demo

class Report(val title: String) {
  fun toBuilder(): Builder = Builder(title)

  class Builder
  internal constructor(
    internal var title: String,
  ) {
    fun rename(next: String): Builder = apply { title = next }

    fun build(): Report = Report(title)
  }

  class Cache
  @Inject
  private constructor(
    private val limit: Int,
  ) {
    fun capacity(): Int = limit
  }

  class Marker(val id: Int)
  constructor(title: String, id: Int) : this(title)

  fun describe(): String = title
}

class Session
@Inject
internal constructor(
  private val user: String,
) {
  fun owner(): String = user
}

class Audit
@Suppress("unused", "long name") @Inject constructor(
  private val user: String,
) {
  fun reviewer(): String = user
}

fun afterSession(): Int = 1
