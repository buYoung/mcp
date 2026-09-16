struct First { callback: fn() }
struct Second { callback: fn() }

macro_rules! implement {
    ($kind:ident) => {
        impl $kind {
            fn fire(&self) {
                (self.callback)();
            }
            fn literal(&self) -> &'static str { "$kind" }
        }
    };
}

implement!(First);
implement!(Second);

fn routes(first: fn(), second: fn()) {
    let left = First { callback: first };
    let right = Second { callback: second };
    left.fire();
    right.fire();
    let mut callbacks = std::collections::HashMap::new();
    callbacks.insert("$kind", first);
    callbacks.insert("First", second);
    callbacks.get(left.literal()).unwrap()();
}
