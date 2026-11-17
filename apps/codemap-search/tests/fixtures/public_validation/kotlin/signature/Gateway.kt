package pay

interface Gateway {
    fun charge(amount: Int, note: String): Boolean

    fun refund(receipt: Receipt)

    fun audit(entry: Entry)
}

class Receipt

class Entry
