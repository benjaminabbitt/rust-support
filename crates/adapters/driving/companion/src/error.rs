//! This crate's errors, and its catalog.

use std::fmt;

use diagnostic::{code, Arg, Diagnostic};
use exit_code::{Exit, ExitClass};
use presentation::{Locales, Source};

/// This crate's en-US messages, under its package name (`diagnostic::build`).
pub const EN_US: Source = (
    env!("CARGO_PKG_NAME"),
    include_str!(concat!(env!("OUT_DIR"), "/en-US.ftl")),
);

/// This crate's catalogs by locale, under the same name.
pub const LOCALES: Locales = (
    env!("CARGO_PKG_NAME"),
    include!(concat!(env!("OUT_DIR"), "/locales.rs")),
);

/// Why `loadout` printed nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LoadoutError {
    /// A format other than yaml was asked for (`text` or `json`).
    Format { format: String },
    /// The document could not be written.
    Output { detail: String },
}

impl fmt::Display for LoadoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        diagnostic::display(self, f)
    }
}

impl std::error::Error for LoadoutError {}

impl Diagnostic for LoadoutError {
    fn code(&self) -> &str {
        match self {
            LoadoutError::Format { .. } => code!("loadout-format"),
            LoadoutError::Output { .. } => code!("output-failed"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            LoadoutError::Format { format } => vec![("format", format.as_str().into())],
            LoadoutError::Output { detail } => vec![("ext_err_txt", detail.as_str().into())],
        }
    }
}

impl ExitClass for LoadoutError {
    /// A format refused is a usage error (refused); a write that broke failed.
    fn exit(&self) -> Exit {
        todo!()
    }
}

/// One value of every error this crate defines, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(LoadoutError::Format {
            format: "json".to_string(),
        }),
        Box::new(LoadoutError::Output {
            detail: "Broken pipe".to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
