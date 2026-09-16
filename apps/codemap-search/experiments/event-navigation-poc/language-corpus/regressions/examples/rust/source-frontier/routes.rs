mod visible {
    pub fn helper() {}
}
use visible::*;

pub fn take_once(callback: fn()) {
    let mut saved = Some(Box::new(callback));
    let first = saved.take().ok_or(()).unwrap();
    first.as_ref()();
    let second = saved.take().unwrap();
    second.as_ref()();
}

struct Holder {
    saved: Option<Box<dyn Fn()>>,
}

pub fn take_field(callback: fn(), other: fn()) {
    let mut holder = Holder { saved: Some(Box::new(callback)) };
    let first = holder.saved.take().ok_or(()).unwrap();
    first.as_ref()();
    if let Some(empty) = holder.saved.take() {
        empty.as_ref()(); }
    holder.saved = Some(Box::new(other));
    let replacement = holder.saved.take().unwrap();
    replacement.as_ref()();
}

mod foreign {
    pub struct Box;
    impl Box {
        pub fn new(value: fn()) -> Self { Self }
    }
    pub fn Some(value: Box) -> Box { value }
}

mod shadowed {
    use super::foreign::*;
    pub fn run(callback: fn()) {
        let saved = Some(Box::new(callback));
        saved.as_ref()();
    }
}

trait Convert { fn convert(self); }
impl<T> Convert for T {
    fn convert(self) { opaque::<T>(self); }
}

pub fn distinct_generic<T>(value: T) { opaque::<T>(value); }

pub fn call_operands(callback: fn()) {
    factory(Box::new(callback), Some(callback), (callback, 1), callback,
            2, 3, 4, 5, 6, 7).id();
}

pub fn different_contexts(callback: fn()) {
    make(callback);
    make(callback);
}

fn make(callback: fn()) { missing(callback); }

fn block_value(first: fn(), second: fn()) -> Box<dyn Fn()> {
    let temporary = unsafe { Box::new(first) };
    Box::new(second)
}

pub fn block_route(first: fn(), second: fn()) {
    let returned = block_value(first, second);
    returned.as_ref()();
}

pub fn tail_take(callback: fn()) -> Box<dyn Fn()> {
    let mut saved = Some(Box::new(callback));
    saved.take().unwrap()
}

pub fn tail_route(callback: fn()) {
    let returned = tail_take(callback);
    returned.as_ref()();
}

// A generic T must also remain distinct from a real, same-named module type.
struct T;
