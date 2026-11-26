//! The entropy check: whether one block of text looks like a random token.
//!
//! **Why not one number.** The Shannon entropy of an `n`-character block is
//! at most `log2(n)` bits per character (2.58 for 6 characters, 4.32 for 20),
//! so a fixed threshold such as gitleaks' 3.5 (its `generic-api-key` rule, on
//! values of 10 to 150 characters it has already cut out with a regular
//! expression) could never fire below 12 characters, and detect-secrets'
//! defaults (base64 4.5, hex 3.0, on quoted strings) miss most tokens under
//! 30. A whole whitespace-delimited block is a path, a URL or a flag as often
//! as a token, and paths and URLs of 40 to 70 characters measure 3.9 to 4.1
//! bits on their own. So there are three rules, each of which can only
//! over-mask, never expose:
//!
//! 1. **hex** (`HEX_*`): every character a hex digit (a digit and a letter
//!    among them), at least 32 of them, at least 3.0 bits (detect-secrets' hex limit). Random hex tops out at 4
//!    bits, so 3.0 is the line that keeps 32 random digits (3.0 to 3.8 bits)
//!    and drops repeated pairs. Git object ids and sha256 digests are masked
//!    too: they are what a hex API key looks like.
//! 2. **short** (`MIN_LEN` to `SHORT_MAX_LEN` characters): a lower-case
//!    letter, a capital and a digit all present, and the entropy at least
//!    `SHORT_RATIO` of `log2(n)`: nearly every character distinct. Ordinary
//!    words and flags (`release`, `origin`, `src/lib.rs`, `HEAD~1`,
//!    `Cargo.toml`) have one or two classes, so they pass. A random
//!    62-symbol token of 6, 8, 12 or 24 characters is masked about 60, 73, 87
//!    and 93 times in a hundred.
//! 3. **long** (`LONG_MIN_LEN` characters and more): a digit and a letter
//!    present and at least `LONG_THRESHOLD` (4.0) bits: gitleaks' 3.5 is
//!    taken up to 4.0 because whole blocks, not extracted values, are
//!    measured. Random 62-symbol tokens measure 4.2 at 24 characters and 4.7
//!    at 40; kebab-case names, UUIDs, model names and file paths without
//!    digits stay under it.
//!
//! What this misses: a low-entropy secret (`hunter2`, `password1`), a short
//! token of only two classes (base36), a token of 16 to 23 characters in the
//! gap between the rules. What it hides that it should not: a camel-case
//! identifier with a digit, and a long path that has digits and many distinct
//! characters. A false positive only hides log text.

/// Fewest characters a block has to be judged.
pub const MIN_LEN: usize = 6;
/// Longest block the short rule judges.
pub const SHORT_MAX_LEN: usize = 24;
/// Short rule: entropy over `log2(length)` at least this.
pub const SHORT_RATIO: f64 = 0.8;
/// Fewest characters the long rule judges.
pub const LONG_MIN_LEN: usize = 16;
/// Long rule: bits per character at least this.
pub const LONG_THRESHOLD: f64 = 4.0;
/// Fewest characters of an all-hex block.
pub const HEX_MIN_LEN: usize = 32;
/// All-hex rule: bits per character at least this.
pub const HEX_THRESHOLD: f64 = 3.0;

/// Shannon entropy of `block` in bits per character (0 for an empty block).
pub fn shannon(block: &str) -> f64 {
    let n = block.chars().count();
    if n == 0 {
        return 0.0;
    }
    let mut chars: Vec<char> = block.chars().collect();
    chars.sort_unstable();
    let total = n as f64;
    let mut bits = 0.0;
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        while j < chars.len() && chars[j] == chars[i] {
            j += 1;
        }
        let p = (j - i) as f64 / total;
        bits -= p * p.log2();
        i = j;
    }
    bits
}

/// Whether `block` looks like a random token.
pub(crate) fn looks_random(block: &str) -> bool {
    let n = block.chars().count();
    if n < MIN_LEN {
        return false;
    }
    let bits = shannon(block);
    let digit = block.chars().any(|c| c.is_ascii_digit());
    let letter = block.chars().any(char::is_alphabetic);
    let hex = block.chars().all(|c| c.is_ascii_hexdigit());
    if n >= HEX_MIN_LEN && hex && digit && letter && bits >= HEX_THRESHOLD {
        return true;
    }
    let upper = block.chars().any(char::is_uppercase);
    let lower = block.chars().any(char::is_lowercase);
    if n <= SHORT_MAX_LEN && upper && lower && digit && bits >= SHORT_RATIO * (n as f64).log2() {
        return true;
    }
    n >= LONG_MIN_LEN && digit && letter && bits >= LONG_THRESHOLD
}

#[cfg(test)]
#[path = "entropy.test.rs"]
mod tests;
