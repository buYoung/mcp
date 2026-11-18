package spooled

import partial.*

// The base is named through a wildcard import, which is not resolved to a declaration.
class SpooledNotifier : Notifier {
    override fun send(count: Int): Boolean = count > 1
}
