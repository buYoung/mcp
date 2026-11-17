package pay.impl

import pay.Gateway
import pay.Receipt

class Entry

abstract class PaypalGateway : Gateway {
    override fun charge(amount: Int, note: String): Boolean = true

    override fun refund(receipt: Receipt) {}

    fun audit(entry: Entry) {}
}
