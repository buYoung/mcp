interface Box<in out T> { callback: T; }
function route(first: Box<() => void>, second: Box<() => void>, callback: () => void) {
  first.callback = callback; // @S1
  first.callback(); // @I1
  second.callback(); // @I2
}
