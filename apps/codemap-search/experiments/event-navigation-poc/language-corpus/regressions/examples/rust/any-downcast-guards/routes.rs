use std::any::Any;
use std::boxed::Box;

struct First { callback: fn() }
struct Second { callback: fn() }

fn standard(first: fn()) {
    let boxed: Box<dyn Any> = Box::new(First { callback: first });
    if let Some(value) = boxed.downcast_ref::<First>() {
        (value.callback)();
    }
    if let Some(value) = boxed.downcast_ref::<Second>() {
        (value.callback)();
    }
}

mod custom {
    pub trait Any {}
    impl dyn Any {
        pub fn downcast_ref<T>(&self) -> Option<&T> { None }
    }
    pub struct Holder { pub callback: fn() }
    impl Any for Holder {}
}

fn custom_trait(first: fn()) {
    let boxed: Box<dyn custom::Any> = Box::new(custom::Holder { callback: first });
    if let Some(value) = boxed.downcast_ref::<custom::Holder>() {
        (value.callback)();
    }
}

fn scoped(boxed: &Box<dyn Any>, first: fn()) {
    let value = First { callback: first };
    if let Some(value) = boxed.downcast_ref::<First>() {
        (value.callback)();
    } else {
        (value.callback)();
    }
    (value.callback)();
}
