const writers = new Map<string, () => void>()
function write(id: string | undefined) {
  writers.get(id!) // selected-consumer
}
function otherWrite(id: string | undefined) {
  writers.get(id!) // other-consumer
}
function ignoredWrite(id: string | undefined) {
  writers.get(id!) // unused-consumer
}
function* protocol(write: (id: string | undefined) => void, other: (id: string | undefined) => void) {
  const pending = new Map<string, string>()
  const unrelated = new Map<string, string>()
  return {
    send(id: string) { pending.set("request", id) }, // stored
    receive() { write(pending.get("request")) },
    receiveOther() { other(unrelated.get("request")) }
  }
}
function* ignored(_write: (id: string | undefined) => void) {
  const pending = new Map<string, string>()
  return { send(id: string) { pending.set("request", id) } } // ignored-store
}
const service = protocol(write, otherWrite)
const unused = ignored(ignoredWrite)

function writeSecond(_first: string | undefined, second: string | undefined) {
  writers.get(second!) // second-argument-consumer
}
function* indexed(write: (first: string | undefined, second: string | undefined) => void) {
  const first = new Map<string, string>()
  const second = new Map<string, string>()
  return {
    sendFirst(id: string) { first.set("request", id) },
    sendSecond(id: string) { second.set("request", id) },
    receive() { write(first.get("request"), second.get("request")) }
  }
}
const indexedService = indexed(writeSecond)
