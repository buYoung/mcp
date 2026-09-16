use alloc::boxed::Box;
struct Decoy;
fn unrelated() {}
impl Decoy { fn as_ref(&self) -> fn() { unrelated } }
trait Factory { fn new(callback: fn()) -> Decoy; }
fn generic_route<Box: Factory>(callback: fn()) {
    let boxed = Box::new(callback); // @STORE
    boxed.as_ref()(); // @READ
}
