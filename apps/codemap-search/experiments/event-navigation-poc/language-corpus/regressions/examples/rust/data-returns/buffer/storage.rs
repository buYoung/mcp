pub struct Entry { pub message: u32, pub id: u32 }
pub struct Store { pub a: Vec<Entry>, pub b: Vec<Entry> }
impl Store {
    pub fn write(&mut self, value: u32) {
        let entry = Entry {
            message: value, // @S1
            id: 5,
        };
        self.b.push(entry);
    }
}
