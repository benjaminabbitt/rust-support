//! Masks secrets in a short text before it is recorded. Pure: no I/O, no
//! clock, no environment, no dependency.
//!
//! The text is split on whitespace and on `=` (a block that does not look random
//! whole is judged again in the pieces [`PUNCTUATION`] cuts it into); each block is judged by its
//! Shannon entropy (see the constants, and the rules in the
//! `entropy` module's documentation) and a block that looks random is
//! replaced by `[masked:N]`, N its length in characters. Whitespace, the `=` and every unmasked block are
//! kept byte for byte. The mask never carries a byte of what it hid, and
//! masked text masks to itself. There are no provider-specific or keyword
//! rules: a secret of low entropy (`password=hunter2`) is not seen.

mod entropy;

pub use entropy::{
    shannon, HEX_KEPT_LENGTHS, HEX_MIN_LEN, HEX_THRESHOLD, MIN_LEN, SHORT_MAX_LEN, SHORT_RATIO,
    TOKEN_LOG_CAP, TOKEN_MIN_LEN, TOKEN_RATIO, TRANSITION_RATIO,
};

/// The punctuation that joins the parts of a URL or a path. A block that does
/// not look random whole is judged again in the pieces these cut it into, so
/// a token in `https://user:<token>@host/` is masked alone; a block that does
/// look random whole is masked whole (a base64 secret holds `/` and `+`).
pub const PUNCTUATION: &str = ":@/?&;,\"'";

/// What replaces a masked block of `len` characters: `[masked:<len>]`. The
/// length is of the hidden block, in characters.
pub fn marker(len: usize) -> String {
    format!("[masked:{len}]")
}

/// Whether `block` is exactly a [`marker`]: never judged again.
fn is_marker(block: &str) -> bool {
    block
        .strip_prefix("[masked:")
        .and_then(|rest| rest.strip_suffix(']'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The result of masking one text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Masked {
    /// The text, each masked block replaced by a [`marker`], every other byte as given.
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
/// rules in the crate documentation) becomes a [`marker`], and every other byte,
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

/// Pushes `block` or its mask; the number of blocks masked.
fn push_block(out: &mut String, block: &str) -> usize {
    if is_marker(block) {
        out.push_str(block);
        0
    } else if entropy::looks_random(block) {
        out.push_str(&marker(block.chars().count()));
        1
    } else {
        push_pieces(out, block)
    }
}

/// Pushes `block` with each piece between [`PUNCTUATION`] that looks random masked.
fn push_pieces(out: &mut String, block: &str) -> usize {
    let mut count = 0;
    let mut start = 0;
    for (i, c) in block.char_indices() {
        if PUNCTUATION.contains(c) {
            count += push_piece(out, &block[start..i]);
            out.push(c);
            start = i + c.len_utf8();
        }
    }
    count + push_piece(out, &block[start..])
}

fn push_piece(out: &mut String, piece: &str) -> usize {
    if !is_marker(piece) && entropy::looks_random(piece) {
        out.push_str(&marker(piece.chars().count()));
        1
    } else {
        out.push_str(piece);
        0
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
