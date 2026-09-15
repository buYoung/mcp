struct Router { primary: fn(), secondary: fn() }
impl Router {
    fn set_primary(&mut self, cb: fn()) { self.primary = cb; } // store_primary
    fn set_secondary(&mut self, cb: fn()) { self.secondary = cb; } // store_secondary
    fn fire_primary(&self) { (self.primary)(); } // call_primary
    fn fire_secondary(&self) { (self.secondary)(); } // call_secondary
}
struct Other { primary: fn() }
impl Other {
    fn fire_primary(&self) { (self.primary)(); } // call_other
}
