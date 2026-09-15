struct Router { cb: fn() }
impl Router {
    fn install(&mut self, f: fn()) { self.cb = f; } // S
    fn fire(&self) { (self.cb)(); } // I
}
