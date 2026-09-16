use std::ptr::NonNull;

struct Holder { handler: fn() }
struct Handle(NonNull<Holder>);

unsafe fn read_pointer(pointer: NonNull<Holder>) {
    (pointer.as_ref().handler)(); // helper-consumer
}

fn routes(left: &mut Holder, right: &mut Holder, callback: fn()) {
    left.handler = callback; // stored
    let pointer = NonNull::from_mut(left);
    unsafe { (pointer.as_ref().handler)(); } // nonnull-consumer
    let raw = pointer.as_ptr();
    unsafe { ((*raw).handler)(); } // raw-consumer
    let copied = pointer.clone();
    unsafe { (copied.as_ref().handler)(); } // clone-consumer
    let other = NonNull::from_mut(right);
    unsafe { (other.as_ref().handler)(); } // other-consumer
    let wrapper = Handle(pointer);
    unsafe { (wrapper.0.as_ref().handler)(); } // tuple-consumer
    unsafe { read_pointer(pointer); }
    let recovered = unsafe { NonNull::new_unchecked(raw) };
    unsafe { (recovered.as_ref().handler)(); } // recovered-consumer
    let address = 4096usize as *mut Holder;
    unsafe { ((*address).handler)(); } // integer-consumer
}

fn reference_cast(holder: &mut Holder, callback: fn()) {
    holder.handler = callback; // cast-stored
    let raw = holder as *mut Holder;
    unsafe { ((*raw).handler)(); } // cast-consumer
}
