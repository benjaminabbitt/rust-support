//! Masks secrets in a short text before it is recorded. Pure: no I/O, no
//! clock, no environment, no dependency.
//!
//! Stub: nothing is masked yet.

mod entropy;

pub use entropy::{
    shannon, HEX_MIN_LEN, HEX_THRESHOLD, LONG_MIN_LEN, LONG_THRESHOLD, MIN_LEN, SHORT_MAX_LEN,
    SHORT_RATIO,
};

/// What replaces a masked block.
pub const MASK: &str = "[masked]";

/// The result of masking one text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Masked {
    /// The text, each masked block replaced by [`MASK`], every other byte as given.
    pub text: String,
    /// How many blocks were masked.
    pub count: usize,
}

impl Masked {
    /// Whether anything was masked.
    pub fn changed(&self) -> bool {
        self.count > 0
    }
}

/// Masks `text`.
pub fn mask(text: &str) -> Masked {
    Masked {
        text: text.to_owned(),
        count: 0,
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
