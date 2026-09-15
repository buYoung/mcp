struct Router { cb: Box<dyn Fn()>, other: Box<dyn Fn()> }
impl Router {
    fn install(&mut self, f: Box<dyn Fn()>, g: Box<dyn Fn()>) {
        self.cb = f; // @S1
        self.other = g; // @S2
    }
    fn fire(&self) {
        let callback = self.cb.as_ref();
        callback(); // @I1
    }
}
