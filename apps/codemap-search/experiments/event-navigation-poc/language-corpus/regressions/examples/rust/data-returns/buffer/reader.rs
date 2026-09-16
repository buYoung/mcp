use crate::buffer::{Entry, Store};
use core::iter::Chain;
use core::slice::Iter;
pub struct Reader<'a> { chain: Chain<Iter<'a, Entry>, Iter<'a, Entry>> }
impl<'a> Reader<'a> {
    pub fn new(input: &'a Store) -> Self {
        let a = input.a.get(0..).unwrap_or_default();
        let b = input.b.get(0..).unwrap_or_default();
        Self { chain: a.iter().chain(b.iter()) }
    }
    pub fn next_value(&mut self) -> Option<u32> {
        match self.chain.next().map(|entry| entry.message) {
            Some(value) => Some(value), // @I5
            None => None,
        }
    }
}
