//! Fixture exercising Rust branch-sensitive extraction.

/// A configuration container.
pub struct Config {
    pub port: u16,
}

impl Config {
    /// Build a default configuration.
    pub fn load() -> Self {
        let label = "default config";
        Config { port: 8080 }
    }

    /// Old loader.
    #[deprecated(note = "use load instead")]
    pub fn legacy_load() -> Self {
        Config { port: 9090 }
    }
}

/// An internal helper that is not exported.
fn internal_helper() -> &'static str {
    "internal helper"
}

#[test]
fn test_loads_config() {
    let _ = Config::load();
}
