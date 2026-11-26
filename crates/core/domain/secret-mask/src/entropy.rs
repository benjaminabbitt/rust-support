//! The entropy check: whether one block of text looks like a random token.
//!
//! **Why not one number.** The Shannon entropy of an `n`-character block is
//! at most `log2(n)` bits per character (2.58 for 6 characters, 3.58 for 12),
//! so a fixed threshold such as gitleaks' 3.5 (its `generic-api-key` rule, on
//! values of 10 to 150 characters it has already cut out with a regular
//! expression) could never fire below 12 characters, and detect-secrets'
//! defaults (base64 4.5, hex 3.0, on quoted strings) miss most tokens under
//! 30. A whole whitespace-delimited block is a path, a URL or a flag as often
//! as a token, and paths and URLs of 40 to 70 characters measure 3.9 to 4.1
//! bits on their own, as much as a random 20-character token. Entropy alone
//! cannot tell them apart; what does is how often the character class
//! (lower case, capital, digit) changes: in a random 62-symbol token every
//! pair of neighbours changes class about 62 times in a hundred, in words,
//! kebab-case names, paths and ids far less. So there are three rules, each of
//! which can only over-mask, never expose:
//!
//! 1. **hex** (`HEX_*`): every character a hex digit (a digit and a letter
//!    among them), at least 32 of them, at least 3.0 bits (detect-secrets'
//!    hex limit), EXCEPT a block of exactly 40 or 64 characters
//!    ([`HEX_KEPT_LENGTHS`]), a git object id or a sha256 digest, which stays
//!    readable. Any other all-hex block (a 32-character key, a 48-character
//!    one) is masked, and an all-hex block shorter than 32 is never judged
//!    (a short commit id, a 16-digit id).
//! 2. **short** (`MIN_LEN` to `SHORT_MAX_LEN` characters, 6 to 11): a
//!    lower-case letter, a capital and a digit all present, and the entropy
//!    at least `SHORT_RATIO` (0.8) of `log2(n)`: nearly every character
//!    distinct. Ordinary words and flags (`release`, `origin`, `src/lib.rs`,
//!    `HEAD~1`, `Cargo.toml`) have one or two classes, so they pass.
//! 3. **token** (`TOKEN_MIN_LEN`, 12 characters and more): at least two of
//!    the three classes present, the class changes among the letters and
//!    digits (symbols skipped) at least `TRANSITION_RATIO` (0.4) of the
//!    neighbouring pairs, and the entropy at least `TOKEN_RATIO` (0.8) of
//!    `log2(min(n, TOKEN_LOG_CAP))` (32), so a long token is not asked for
//!    more than 5 bits.
//!
//! **Measured** on random 62-symbol tokens (the tests sample 400 per length
//! with a fixed seed and require these floors): 6 characters 57 in 100 masked,
//! 8: 73, 10: 81, 11: 83, and from 12 up never under 90: 12: 92, 13: 96, 14:
//! 93, 16: 97, 20: 97, 24: 98, 32: 99, 40: 99, 64 and over: 100. URL-safe and
//! standard base64 tokens do as well from 16 (97 at 16, 100 at 64). Random hex:
//! every length from 32 but 40 and 64. On a sample of ordinary command lines
//! (about 120 blocks of 6 or more characters) 2 are masked (camel-case
//! identifiers); the rules before this tuning masked 11.
//!
//! **What this misses:** a low-entropy secret (`hunter2`, `password1`), a
//! token of 2 classes that rarely changes class (base36 hex-like), a token
//! under 12 characters one time in two or three, a token in a script without
//! case, and any hex under 32 characters. **What it hides that it should
//! not:** a camel-case identifier of 12 or more characters. A false positive
//! only hides log text.

/// Fewest characters a block has to be judged.
pub const MIN_LEN: usize = 6;
/// Longest block the short rule judges.
pub const SHORT_MAX_LEN: usize = 11;
/// Short rule: entropy over `log2(length)` at least this.
pub const SHORT_RATIO: f64 = 0.8;
/// Fewest characters of a block the token rule judges.
pub const TOKEN_MIN_LEN: usize = 12;
/// Token rule: entropy over `log2(min(length, TOKEN_LOG_CAP))` at least this.
pub const TOKEN_RATIO: f64 = 0.8;
/// Token rule: lengths over this count as this in the logarithm.
pub const TOKEN_LOG_CAP: usize = 32;
/// Token rule: class changes over neighbouring letter pairs at least this.
pub const TRANSITION_RATIO: f64 = 0.4;
/// Fewest characters of an all-hex block.
pub const HEX_MIN_LEN: usize = 32;
/// All-hex rule: bits per character at least this.
pub const HEX_THRESHOLD: f64 = 3.0;
/// The all-hex lengths of a git object id and a sha256 digest: kept readable.
pub const HEX_KEPT_LENGTHS: [usize; 2] = [40, 64];

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
    if block.chars().all(|c| c.is_ascii_hexdigit()) {
        return n >= HEX_MIN_LEN
            && !HEX_KEPT_LENGTHS.contains(&n)
            && digit
            && letter
            && bits >= HEX_THRESHOLD;
    }
    let upper = block.chars().any(char::is_uppercase);
    let lower = block.chars().any(char::is_lowercase);
    if n <= SHORT_MAX_LEN {
        return upper && lower && digit && bits >= SHORT_RATIO * (n as f64).log2();
    }
    let classes = usize::from(upper) + usize::from(lower) + usize::from(digit);
    classes >= 2
        && changes_class_often(block)
        && bits >= TOKEN_RATIO * (n.min(TOKEN_LOG_CAP) as f64).log2()
}

/// Whether, among the letters and digits of `block` (symbols skipped), the
/// class (lower, upper, digit) changes between at least `TRANSITION_RATIO`
/// of the neighbouring pairs.
fn changes_class_often(block: &str) -> bool {
    let classes: Vec<u8> = block
        .chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| {
            if c.is_ascii_digit() {
                2
            } else if c.is_uppercase() {
                1
            } else {
                0
            }
        })
        .collect();
    let pairs = classes.len().saturating_sub(1);
    let changes = classes.windows(2).filter(|w| w[0] != w[1]).count();
    pairs > 0 && changes as f64 >= TRANSITION_RATIO * pairs as f64
}

#[cfg(test)]
#[path = "entropy.test.rs"]
mod tests;
