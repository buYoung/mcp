function routes(left: Array<() => void>, right: Array<() => void>, callback: () => void) {
  left.push(callback) // stored
  const fromLeft = left.pop()
  fromLeft!() // same-stack
  const fromRight = right.pop()
  fromRight!() // other-stack
}
