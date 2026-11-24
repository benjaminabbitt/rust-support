//! FNV-1a, 64 and 128 bit. Pure: no dependency, no I/O, no allocation.
//!
//! **What it is.** FNV-1a is the Fowler-Noll-Vo hash, variant 1a. It starts
//! from an offset basis and, for each input byte, XORs the byte into the hash
//! and THEN multiplies by the FNV prime, wrapping modulo the hash width.
//! (Variant 1, FNV-1, multiplies first and XORs second; the two give
//! different values.)
//!
//! ```text
//! h = OFFSET_BASIS
//! for byte in input { h = (h ^ byte).wrapping_mul(PRIME) }
//! ```
//!
//! **Parameters** (G. Fowler, L. C. Noll, K. Vo, D. Eastlake, T. Hansen,
//! "The FNV Non-Cryptographic Hash Algorithm", IETF draft-eastlake-fnv, and
//! <http://www.isthe.com/chongo/tech/comp/fnv/>):
//!
//! | width | offset basis                            | prime                                   |
//! |-------|-----------------------------------------|-----------------------------------------|
//! | 64    | `0xcbf29ce484222325`                    | `0x00000100000001b3` (2^40 + 0x1b3)     |
//! | 128   | `0x6c62272e07bb014262b821756295c58d`    | `0x0000000001000000000000000000013B` (2^88 + 0x13B) |
//!
//! **Test vectors** (hex; the hash of the bytes, no terminator):
//!
//! | input      | [`fnv1a_64`]       | [`fnv1a_128`]                      |
//! |------------|--------------------|------------------------------------|
//! | `""`       | `cbf29ce484222325` | `6c62272e07bb014262b821756295c58d` |
//! | `"a"`      | `af63dc4c8601ec8c` | `d228cb696f1a8caf78912b704e4a8964` |
//! | `"foobar"` | `85944171f73967e8` | `343e1662793c64bf6f0d3597ba446f18` |
//!
//! The empty input hashes to the offset basis by definition. The draft
//! publishes 32- and 64-bit vectors only (its tables 3 and 4: the strings
//! without and with a terminating NUL), so the 64-bit rows are the draft's.
//! The 128-bit rows are NOT from a published table: they were computed by an
//! independent model of the loop above (a few lines of Python integer
//! arithmetic, written apart from this code) that was first run with the
//! 64-bit parameters and reproduced the draft's 64-bit rows, then run with
//! the 128-bit ones.
//!
//! **Why this hash.** It needs no dependency and is a handful of lines, so
//! every consumer, in any language, can reproduce it exactly. It is NOT
//! cryptographic: fine for spreading names and ids, not for adversarial
//! input, and a collision must be caught by whatever uses it.
//!
//! **Compatibility promise.** A value derived from this hash and stored or
//! shown (an id, a name, a seed) changes if the parameters or the byte order
//! of a consumer's use of them change. These functions never change: a
//! different hash is a different function, added beside them.
//!
//! **Using the 128-bit value.** [`fnv1a_128`] returns a `u128`. A consumer
//! that needs 64 bits takes the low half (`h as u64`); one that needs hex
//! writes it as 32 lowercase digits, most significant first
//! (`format!("{h:032x}")`). The two halves are not independent of the width:
//! the low 64 bits of the 128-bit hash are NOT [`fnv1a_64`] of the input.

/// The 64-bit offset basis.
pub const OFFSET_BASIS_64: u64 = 0xcbf2_9ce4_8422_2325;

/// The 64-bit FNV prime, 2^40 + 0x1b3.
pub const PRIME_64: u64 = 0x0000_0100_0000_01b3;

/// The 128-bit offset basis.
pub const OFFSET_BASIS_128: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;

/// The 128-bit FNV prime, 2^88 + 0x13B.
pub const PRIME_128: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013B;

/// FNV-1a of `bytes`, 64 bits.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let _ = bytes;
    todo!("FNV-1a 64")
}

/// FNV-1a of `bytes`, 128 bits.
pub fn fnv1a_128(bytes: &[u8]) -> u128 {
    let _ = bytes;
    todo!("FNV-1a 128")
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
