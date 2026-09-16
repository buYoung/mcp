use std::any::TypeId;
use std::collections::HashMap;
#[allow(non_camel_case_types)]
struct u32;

fn routes(entries: &mut HashMap<TypeId, fn()>, callback: fn()) {
    entries.insert(TypeId::of::<u32>(), callback); // stored
    (entries.get(&TypeId::of::<u32>()).unwrap())(); // same-custom
    (entries.get(&TypeId::of::<std::primitive::u32>()).unwrap())(); // standard-is-distinct
    entries.insert(TypeId::of::<std::primitive::u32>(), callback); // native-stored
    (entries.get(&TypeId::of::<core::primitive::u32>()).unwrap())(); // native-matches
}
