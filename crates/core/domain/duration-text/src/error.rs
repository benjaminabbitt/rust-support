//! Why a span or a date is refused: one code each.

use std::fmt;

use coded_diagnostic::{code, Arg, Diagnostic};

/// Why a text is not a span (or, for [`cutoff_at`](crate::cutoff_at), a
/// date). Each variant has its own code and carries the text it refused;
/// jiff's own error is never exposed, its words ride as `ext_err_txt`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DurationError {
    /// Nothing but whitespace (or nothing).
    Empty,
    /// Not a span of jiff's friendly format: no number, no unit (`30`), an
    /// unknown unit, a fraction on a unit that takes none (`1.5d`), a value
    /// beyond a span's range. `ext_err_txt` says which.
    Malformed { text: String, ext_err_txt: String },
    /// A span of less than nothing, where one of none or more is read:
    /// `-3d`, `1h ago`.
    Negative { text: String },
    /// A year or a month, which has no fixed length: `1y`, `2mo`.
    CalendarUnit { text: String },
    /// Shaped like a date (`1-2-3`) but not a calendar date written
    /// `YYYY-MM-DD`: `2026-13-40`, `2026-02-29`, `2026-1-1`.
    InvalidDate { text: String },
    /// A cutoff before the earliest time `SystemTime` holds, or a date
    /// beyond what it holds.
    OutOfRange { text: String },
}

impl DurationError {
    /// jiff's refusal of `text`, as this crate's.
    pub(crate) fn malformed(text: &str, cause: &jiff::Error) -> Self {
        Self::Malformed {
            text: text.to_string(),
            ext_err_txt: cause.to_string(),
        }
    }
}

impl fmt::Display for DurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        coded_diagnostic::display(self, f)
    }
}

impl Diagnostic for DurationError {
    fn code(&self) -> &str {
        match self {
            Self::Empty => code!("empty"),
            Self::Malformed { .. } => code!("malformed"),
            Self::Negative { .. } => code!("negative"),
            Self::CalendarUnit { .. } => code!("calendar-unit"),
            Self::InvalidDate { .. } => code!("invalid-date"),
            Self::OutOfRange { .. } => code!("out-of-range"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Self::Empty => Vec::new(),
            Self::Negative { text }
            | Self::CalendarUnit { text }
            | Self::InvalidDate { text }
            | Self::OutOfRange { text } => vec![("text", text.as_str().into())],
            Self::Malformed { text, ext_err_txt } => vec![
                ("text", text.as_str().into()),
                ("ext_err_txt", ext_err_txt.as_str().into()),
            ],
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!(concat!(env!("OUT_DIR"), "/en-US.ftl")),
);

/// The remedy of an error: its code with `-fix`, and no args (a remedy is
/// fixed words), for the catalog test to hold each `-fix` id to a sample.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug)]
struct FixOf(String);

#[cfg(any(test, feature = "test-support"))]
impl fmt::Display for FixOf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        coded_diagnostic::display(self, f)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Diagnostic for FixOf {
    fn code(&self) -> &str {
        &self.0
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        Vec::new()
    }
}

/// One value of every error and every remedy, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let fixes = ["empty", "malformed", "negative", "calendar-unit"].map(|suffix| {
        Box::new(FixOf(format!("{}-{suffix}-fix", env!("CARGO_PKG_NAME")))) as Box<dyn Diagnostic>
    });
    let mut samples: Vec<Box<dyn Diagnostic>> = fixes.into();
    samples.extend(errors());
    samples
}

#[cfg(any(test, feature = "test-support"))]
fn errors() -> Vec<Box<dyn Diagnostic>> {
    vec![
        Box::new(DurationError::Empty),
        Box::new(DurationError::Malformed {
            text: "30".into(),
            ext_err_txt: "expected a unit designator after the number".into(),
        }),
        Box::new(DurationError::Negative { text: "-3d".into() }),
        Box::new(DurationError::CalendarUnit { text: "1y".into() }),
        Box::new(DurationError::InvalidDate {
            text: "2026-13-40".into(),
        }),
        Box::new(DurationError::OutOfRange {
            text: "99999999999d".into(),
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
