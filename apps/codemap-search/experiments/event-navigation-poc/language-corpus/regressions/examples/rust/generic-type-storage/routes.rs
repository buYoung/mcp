use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::boxed::Box;

pub struct First {
    callback: fn(),
}

pub struct Second {
    callback: fn(),
}

pub struct Arena {
    slots: HashMap<(u32, TypeId), Box<dyn Any>>,
}

impl Arena {
    fn new() -> Self {
        Self { slots: HashMap::new() }
    }

    fn put<T: 'static>(&mut self, key: u32, value: T) {
        self.slots.insert((key, TypeId::of::<T>()), Box::new(value));
    }

    fn get<T: 'static>(&self, key: u32) -> &T {
        self.slots.get(&(key, TypeId::of::<T>())).unwrap().downcast_ref::<T>().unwrap()
    }
}

pub fn routes(first: fn(), second: fn()) {
    let mut left = Arena::new();
    let mut right = Arena::new();
    left.put(1, First { callback: first });
    left.put(2, First { callback: second });
    left.put(1, Second { callback: second });
    right.put(1, First { callback: second });
    (left.get::<First>(1).callback)();
    (left.get::<First>(2).callback)();
    (left.get::<Second>(1).callback)();
    (right.get::<First>(1).callback)();
}
