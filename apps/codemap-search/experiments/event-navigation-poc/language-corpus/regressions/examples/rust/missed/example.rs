struct Router { cb: Box<dyn Fn()> }
impl Router {
    fn install(&mut self, f: Box<dyn Fn()>) { self.cb = f; } // S
    fn fire(&self) {
        let cb = self.cb.as_ref();
        cb(); // I
    }
}
