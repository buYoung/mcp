/** Fixture exercising Kotlin branch-sensitive extraction. */
class Repository {

    /** Fetch a record. */
    fun fetch(): String {
        val query = "select * from users"
        return query
    }

    /** Old fetcher. */
    @Deprecated("use fetch instead")
    fun legacyFetch(): String {
        return "legacy"
    }

    private fun privateHelper(): Int {
        return 0
    }

    @Test
    fun testFetch() {
        fetch()
    }
}

/** An exported top-level function. */
fun publicHelper(): Int {
    return 1
}
