pub struct Reading(pub String);

impl From<&str> for Reading {
    // The only indexed `from`.
    fn from(text: &str) -> Self {
        Reading(text.to_string())
    }
}

// The path names a type that this repository does not declare.
pub fn label() -> String {
    String::from("idle")
}

// A trait path can reach any implementation, so this call stays unconfirmed.
pub fn through_trait(text: &str) -> Reading {
    From::from(text)
}

// A type parameter can stand for the declaring type, so this call stays unconfirmed.
pub fn through_parameter<T: for<'a> From<&'a str>>(text: &str) -> T {
    T::from(text)
}

impl TryFrom<Reading> for i32 {
    type Error = std::num::ParseIntError;

    // The only indexed `try_from`, written for a type of another crate.
    fn try_from(reading: Reading) -> Result<Self, Self::Error> {
        reading.0.parse()
    }
}

// The path names the type the implementation is written for, so this call stays listed.
pub fn level(reading: Reading) -> i32 {
    i32::try_from(reading).unwrap_or(0)
}

// The path names another type that this repository does not declare.
pub fn narrow(level: i32) -> u8 {
    u8::try_from(level).unwrap_or(0)
}
