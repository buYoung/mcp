use std::ptr::NonNull as StandardPointer;
struct Holder { handler: fn() }
struct NonNull;
impl NonNull {
    fn from_mut(_value: &mut Holder) -> StandardPointer<Holder> {
        panic!("opaque custom constructor")
    }
}
fn routes(holder: &mut Holder, callback: fn()) {
    holder.handler = callback; // stored
    let standard = StandardPointer::from_mut(holder);
    unsafe { (standard.as_ref().handler)(); } // positive
    let custom = NonNull::from_mut(holder);
    unsafe { (custom.as_ref().handler)(); } // shadowed
}
