type Handler = () => void
interface Holder { handler: Handler }

function* relay(handler: Handler) {
  const resumed = yield handler
  return resumed
}
function* delegated(handler: Handler) {
  return yield* relay(handler)
}
function* unknownDelegate(handler: Handler) {
  return yield* opaqueIterator(handler)
}
declare function opaqueIterator(handler: Handler): Generator<Handler>
function advance(iterator: Generator) { return iterator.next() }

function routes(left: Holder, right: Holder, callback: Handler) {
  left.handler = callback // stored
  const iterator = relay(left.handler)
  iterator.next().value() // yielded
  iterator.next(left.handler).value() // resumed
  iterator.next(right.handler).value() // completed
  const unstarted = relay(left.handler)
  ;(unstarted as any).value() // unstarted
  const other = relay(right.handler)
  other.next().value() // other
  const outer = delegated(left.handler)
  outer.next().value() // delegated-yield
  outer.next(left.handler).value() // delegated-return
  const forwarded = relay(left.handler)
  advance(forwarded).value() // helper
  const unknown = unknownDelegate(left.handler)
  unknown.next().value() // opaque-delegate
  const returned = returnBeforeYield(left.handler, right.handler)
  returned.next().value() // early-return
  const throwing = throwBeforeYield(left.handler)
  throwing.next().value() // early-throw
}

function* returnBeforeYield(left: Handler, right: Handler) {
  if (true) return right
  yield left
}
function* throwBeforeYield(handler: Handler) {
  throw new Error("stop before suspension")
  yield handler
}
