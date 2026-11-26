//! The entropy check (stub).

/// Fewest characters a block has to be judged.
pub const MIN_LEN: usize = 0;
/// Longest block judged by the short rule.
pub const SHORT_MAX_LEN: usize = 0;
/// Short rule: bits per character over log2 of the length.
pub const SHORT_RATIO: f64 = 0.0;
/// Fewest characters for the long rule.
pub const LONG_MIN_LEN: usize = 0;
/// Long rule: bits per character.
pub const LONG_THRESHOLD: f64 = 0.0;
/// Fewest characters of an all-hex block.
pub const HEX_MIN_LEN: usize = 0;
/// All-hex rule: bits per character.
pub const HEX_THRESHOLD: f64 = 0.0;

/// Shannon entropy of `block` in bits per character.
pub fn shannon(_block: &str) -> f64 {
    -1.0
}

/// Whether `block` looks like a random token.
#[allow(dead_code)]
pub(crate) fn looks_random(_block: &str) -> bool {
    false
}

#[cfg(test)]
#[path = "entropy.test.rs"]
mod tests;
