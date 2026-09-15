struct Box<T: ?Sized> { inner: std::boxed::Box<T> }
fn empty() {}
impl<T: ?Sized> Box<T> {
    fn as_ref(&self) -> fn() { return empty; }
}
struct Router { cb: Box<dyn Fn()> }
impl Router {
    fn install(&mut self, f: Box<dyn Fn()>) { self.cb = f; } // @S1
    fn fire(&self) {
        let callback = self.cb.as_ref();
        callback(); // @I1
    }
}
