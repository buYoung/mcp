use core::ops::{Deref as ReadStorage, DerefMut as WriteStorage};
struct Bag<T> { values: Vec<T> }
impl<T> ReadStorage for Bag<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target { &self.values }
}
impl<T> WriteStorage for Bag<T> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.values }
}
struct Router { bag: Bag<fn()>, other: Bag<fn()> }
impl Router {
    fn install(&mut self, callback: fn()) { self.bag.push(callback); } // @S1
    fn fire(&self) { (self.bag[0])(); } // @I1
    fn fire_other(&self) { (self.other[0])(); } // @I2
}
struct Custom { values: Vec<fn()>, own: Vec<fn()> }
impl ReadStorage for Custom {
    type Target = Vec<fn()>;
    fn deref(&self) -> &Self::Target { &self.values }
}
impl Custom {
    fn push(&mut self, value: fn()) { self.own.push(value); } // @S2
    fn install(&mut self, value: fn()) { self.push(value); }
    fn fire(&self) { (self.own[0])(); } // @I3
    fn fire_deref(&self) { (self[0])(); } // @I4
}
