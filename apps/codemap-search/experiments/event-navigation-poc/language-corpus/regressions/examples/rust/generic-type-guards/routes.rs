use std::any::TypeId;
use std::collections::HashMap;

struct Plain;
struct Tag<T> { _value: T }

fn register<T: 'static>(map: &mut HashMap<TypeId, fn()>, _tag: T, callback: fn()) {
    map.insert(TypeId::of::<T>(), callback);
}

fn invoke<T: 'static>(map: &HashMap<TypeId, fn()>, _tag: T) {
    map.get(&TypeId::of::<T>()).unwrap()();
}

fn routes(plain: fn(), generic: fn()) {
    let mut map = HashMap::new();
    register(&mut map, Plain, plain);
    invoke(&map, Plain);
    register(&mut map, Tag { _value: 1u32 }, generic);
    invoke(&map, Tag { _value: 2u64 });
}
