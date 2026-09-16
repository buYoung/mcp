use bridge::relocate;

struct Holder { handler: fn() }

macro_rules! local {
    ($value:ident) => {
        let mut $value = ::core::mem::MaybeUninit::new($value);
        let $value = $value.as_mut_ptr();
    };
}

fn nested(first: fn(), second: fn()) {
    let left = Holder { handler: first }; // nested-left-store
    let right = Holder { handler: second }; // nested-right-store
    bridge::twice!(left);
    bridge::twice!(right);
    (left.0.0.handler)(); // nested-left-call
    (right.0.0.handler)(); // nested-right-call
}
macro_rules! unsafe_hygiene {
    ($value:ident) => {
        let $value = unrelated;
    };
}
macro_rules! repeated {
    ($($value:ident),*) => { $(let $value = $value;)* };
}

fn routes(first: fn(), second: fn()) {
    let mut left = Holder { handler: first }; // left-store
    let mut right = Holder { handler: second }; // right-store
    local!(left);
    relocate!(right);
    unsafe { (left.read().handler)(); } // left-call
    unsafe { (right.read().handler)(); } // right-call
    let unrelated = left;
    let mut blocked = Holder { handler: second };
    unsafe_hygiene!(blocked);
    unsafe { (blocked.read().handler)(); } // unsupported-hygiene
    let mut multi = Holder { handler: second };
    repeated!(multi);
    unsafe { (multi.read().handler)(); } // unsupported-repetition
    let mut uninitialized = ::core::mem::MaybeUninit::<Holder>::uninit();
    unsafe { (uninitialized.as_mut_ptr().read().handler)(); } // uninitialized
}
