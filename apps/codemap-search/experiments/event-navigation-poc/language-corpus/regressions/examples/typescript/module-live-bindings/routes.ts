function callback() {}
const left = new Map<string, () => void>()
const right = new Map<string, () => void>()
left.set("ready", callback) // left-store
right.set("ready", callback) // right-store
let selected = left
function makeReader() { return () => selected.get("ready")! }
const reader = makeReader()
reader()() // before
selected = right
reader()() // after
