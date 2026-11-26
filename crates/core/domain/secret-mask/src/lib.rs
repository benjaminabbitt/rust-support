//! Masks secrets in a short text before it is recorded. Pure: no I/O, no
//! clock, no environment, no dependency.
//!
//! The text is split on whitespace and on `=`; each block is judged by its
//! Shannon entropy (see [`MIN_LEN`] and the constants, and the rules in the
//! `entropy` module's documentation) and a block that looks random is
//! replaced by `[masked]`. Whitespace, the `=` and every unmasked block are
//! kept byte for byte. The mask never carries a byte of what it hid, and
//! masked text masks to itself. There are no provider-specific or keyword
//! rules: a secret of low entropy (`password=hunter2`) is not seen.

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

/// Masks `text`: it is split on whitespace and on `=`, each block of
/// [`MIN_LEN`] characters or more that looks like a random token (see the
/// rules in the crate documentation) becomes [`MASK`], and every other byte,
/// the separators included, is kept exactly.
pub fn mask(text: &str) -> Masked {
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() || c == '=' {
            count += push_block(&mut out, &text[start..i]);
            out.push(c);
            start = i + c.len_utf8();
        }
    }
    count += push_block(&mut out, &text[start..]);
    Masked { text: out, count }
}

/// Pushes `block` or its mask; 1 when it was masked.
fn push_block(out: &mut String, block: &str) -> usize {
    if entropy::looks_random(block) {
        out.push_str(MASK);
        1
    } else {
        out.push_str(block);
        0
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
