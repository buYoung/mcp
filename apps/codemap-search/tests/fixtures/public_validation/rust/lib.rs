pub struct Unrelated;

impl Unrelated {
    pub fn split(&self) {}
    pub fn collect(&self) {}
    pub fn matches(&self) {}
}

pub fn standard_iterator(value: &str) -> Vec<&str> {
    value.split('.').collect()
}

pub fn standard_macro(value: Option<u32>) -> bool {
    matches!(value, Some(1))
}
