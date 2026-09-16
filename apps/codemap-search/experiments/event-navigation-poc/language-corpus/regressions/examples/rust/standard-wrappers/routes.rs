use alloc::boxed::Box;
fn box_route(left: fn(), right: fn()) {
    let boxed = Box::new(left); // @BOX_STORE
    let other = Box::new(right); // @OTHER_STORE
    boxed.as_ref()(); // @BOX_READ
}
fn option_route(callback: fn(), callbacks: &mut Vec<fn()>) {
    let original = Some(callback);
    let Some(value) = original else { return; };
    callbacks.push(Some(value).unwrap()); // @OPTION_STORE
    callbacks[0]() // @OPTION_READ
}
fn shadow_route<Some>(callback: fn(), callbacks: &mut Vec<fn()>, Some: fn(fn()) -> Decoy) {
    callbacks.push(callback); // @SHADOW_STORE
    let output = Some(callbacks[0]);
    output.unwrap()(); // @SHADOW_READ
}
struct Decoy;
