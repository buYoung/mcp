function apply(factory: () => (() => void)) { return factory() }
function read(this: { entries: Map<string, () => void> }) {
  return this.entries.get("ready")!
}
function routes(callback: () => void) {
  const left = new Map<string, () => void>()
  const right = new Map<string, () => void>()
  left.set("ready", callback) // stored
  const first = { factory: () => left.get("ready")! }
  const second = { factory: () => right.get("ready")! }
  apply(first.factory)() // stored-function
  apply(second.factory)() // other-function
  const one = { entries: left, read }
  const two = { entries: right, read }
  one.read()() // bound-receiver
  two.read()() // other-receiver
}
