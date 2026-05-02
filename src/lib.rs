//! A small image IO crate built step by step with the Rust standard library.
//!
//! The crate is currently in the project setup phase.

/// The crate name used by basic setup tests.
pub const CRATE_NAME: &str = "kimage";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_kimage() {
        assert_eq!(CRATE_NAME, "kimage");
    }
}
