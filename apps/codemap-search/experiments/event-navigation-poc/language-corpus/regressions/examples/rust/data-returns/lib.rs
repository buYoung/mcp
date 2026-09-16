mod buffer;
use crate::buffer::Store;
fn read_b(input: &Store) -> Option<u32> {
    input.b.get(0..).unwrap_or_default().iter().next().map(|entry| entry.message) // @I1
}
fn read_a(input: &Store) -> Option<u32> {
    input.a.get(0..).unwrap_or_default().iter().next().map(|entry| entry.message) // @I2
}
fn read_id(input: &Store) -> Option<u32> {
    input.b.iter().next().map(|entry| entry.id) // @I3
}
fn distinct(left: &mut Store, right: &Store, value: u32) -> Option<u32> {
    left.b.push(buffer::Entry { message: value, id: 0 }); // @S2
    right.b.iter().next().map(|entry| entry.message) // @I4
}
fn same(input: &mut Store, value: u32) -> Option<u32> {
    input.b.push(buffer::Entry { message: value, id: 0 }); // @S3
    input.b.iter().next().map(|entry| entry.message) // @I6
}
