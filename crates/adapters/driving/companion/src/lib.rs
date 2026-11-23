//! The companion contract: what a host program reads from a tool it runs to
//! learn what the tool contributes. Two commands make it:
//!
//! - `version --format json`: the tool's `name`, `version` and, when it has
//!   one, its `lock_protocol`, in that order ([`Version`]).
//! - `loadout --format yaml`: the tool's loadout, a YAML document the tool
//!   owns and this crate prints verbatim ([`loadout`]). yaml is the only
//!   format it is printed in; asking for another is refused.
//!
//! The document's contents are the tool's: this crate neither reads nor
//! checks them.

use std::io::Write;
use std::str::FromStr;

use presentation::{Format, Render, UnknownFormat};
use serde::Serialize;

mod error;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{LoadoutError, EN_US, LOCALES};

/// A `--format` value as the companion commands read it: one of
/// presentation's formats, or `yaml`, which only `loadout` speaks.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Asked {
    Format(Format),
    Yaml,
}

impl FromStr for Asked {
    type Err = UnknownFormat;

    fn from_str(value: &str) -> Result<Asked, UnknownFormat> {
        let _ = value;
        todo!()
    }
}

/// What `version` reports: its fields in this order, `lock_protocol` left
/// out when 0.
#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Version<'a> {
    pub name: &'a str,
    pub version: &'a str,
    #[serde(skip_serializing_if = "is_zero")]
    pub lock_protocol: u32,
}

fn is_zero(n: &u32) -> bool {
    let _ = n;
    todo!()
}

impl Render for Version<'_> {
    fn json(&self) -> impl Serialize + '_ {
        self
    }

    /// The bare version, on its own line.
    fn text(&self, out: &mut dyn Write) -> std::io::Result<()> {
        let _ = out;
        todo!()
    }
}

/// Print `document`, the tool's loadout, to `out` as it is (comments and
/// all), unless a format other than yaml was asked for: then nothing is
/// printed and the request is refused. No `--format` at all is yaml.
pub fn loadout(
    asked: Option<Asked>,
    document: &str,
    out: &mut dyn Write,
) -> Result<(), LoadoutError> {
    let _ = (asked, document, out);
    todo!()
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
