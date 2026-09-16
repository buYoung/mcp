use std::marker::PhantomData;

struct HashMap<K, V> {
    value: V,
    _marker: PhantomData<K>,
}

impl<K, V> HashMap<K, V> {
    fn new(value: V) -> Self {
        Self { value, _marker: PhantomData }
    }

    fn insert(&mut self, _key: K, value: V) {
        self.value = value;
    }

    fn get(&self, _key: K) -> Option<&V> { None }
    fn actual(&self) -> &V { &self.value }
}

fn routes(first: fn(), second: fn()) {
    let mut map: HashMap<u32, fn()> = HashMap::new(first);
    map.insert(1, second);
    (map.actual())();
    if let Some(value) = map.get(1) {
        value();
    }
}
