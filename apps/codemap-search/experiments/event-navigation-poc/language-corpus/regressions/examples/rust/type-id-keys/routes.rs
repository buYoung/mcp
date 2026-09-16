use std::any::TypeId;
use std::collections::HashMap;
struct Tag<T>(T);

fn routes<T: 'static>(entries: &mut HashMap<TypeId, fn()>, other: &mut HashMap<TypeId, fn()>, callback: fn()) {
    entries.insert(TypeId::of::<u32>(), callback); // stored
    (entries.get(&TypeId::of::<u32>()).unwrap())(); // same-type
    (entries.get(&TypeId::of::<u64>()).unwrap())(); // other-type
    (other.get(&TypeId::of::<u32>()).unwrap())(); // other-map
    (entries.get(&TypeId::of::<T>()).unwrap())(); // unknown-generic
    entries.insert(TypeId::of::<Tag<u32>>(), callback); // generic-stored
    (entries.get(&TypeId::of::<Tag<u32>>()).unwrap())(); // same-generic
    (entries.get(&TypeId::of::<Tag<u64>>()).unwrap())(); // other-generic
}
