//! Why a document does not migrate, and why a registry is not built: one
//! code each.

use std::fmt;

use coded_diagnostic::{code, Arg, Diagnostic, Report};

use crate::Version;

/// Why a document does not migrate.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MigrateError {
    /// The root mapping has no version under `key`, and the registry does
    /// not read an unstamped document as an older version.
    NoVersion { key: String },
    /// The value under `key` is not a non-negative integer; `found` says
    /// what it is (`-1`, `list`, `abc`).
    BadVersion { key: String, found: String },
    /// The root is not a mapping.
    RootNotAMapping,
    /// Newer than the registry's current version.
    TooNew { found: Version, current: Version },
    /// Older than the registry's oldest version.
    TooOld { found: Version, oldest: Version },
    /// A downgrade to a version that is not lower.
    NotADowngrade { from: Version, to: Version },
    /// A downgrade through a version with no back step.
    NoBack { from: Version },
    /// The step from `from` to `to` failed with `cause`.
    StepFailed {
        from: Version,
        to: Version,
        cause: Box<MigrateError>,
    },
    /// A step and its back step did not give the document back.
    RoundTrip { version: Version },
    /// An object is not the type the registry has for its version:
    /// `expected` is the type's name.
    WrongType { expected: String },
    /// A form has no loader for `version`, which the registry reads.
    NoLoader { version: Version },
    /// A version's bytes do not decode as that version's object.
    Undecodable {
        version: Version,
        ext_err_txt: String,
    },
    /// A step's own failure, with its own code and args.
    Caller(Report),
}

impl MigrateError {
    /// A step's own error, keeping its code and args.
    pub fn caller(error: &dyn Diagnostic) -> MigrateError {
        MigrateError::Caller(Report::of(error))
    }
}

/// A step that cannot fail (`From`) has nothing to say.
impl From<std::convert::Infallible> for MigrateError {
    fn from(never: std::convert::Infallible) -> MigrateError {
        match never {}
    }
}

impl From<Report> for MigrateError {
    fn from(report: Report) -> MigrateError {
        MigrateError::Caller(report)
    }
}

impl fmt::Display for MigrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        coded_diagnostic::display(self, f)
    }
}

impl std::error::Error for MigrateError {}

impl Diagnostic for MigrateError {
    fn code(&self) -> &str {
        match self {
            Self::NoVersion { .. } => code!("no-version"),
            Self::BadVersion { .. } => code!("bad-version"),
            Self::RootNotAMapping => code!("root-not-a-mapping"),
            Self::TooNew { .. } => code!("too-new"),
            Self::TooOld { .. } => code!("too-old"),
            Self::NotADowngrade { .. } => code!("not-a-downgrade"),
            Self::NoBack { .. } => code!("no-back"),
            Self::StepFailed { .. } => code!("step-failed"),
            Self::RoundTrip { .. } => code!("round-trip"),
            Self::WrongType { .. } => code!("wrong-type"),
            Self::NoLoader { .. } => code!("no-loader"),
            Self::Undecodable { .. } => code!("undecodable"),
            Self::Caller(report) => report.code(),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Self::NoVersion { key } => vec![("key", key.as_str().into())],
            Self::BadVersion { key, found } => {
                vec![
                    ("key", key.as_str().into()),
                    ("found", found.as_str().into()),
                ]
            }
            Self::RootNotAMapping => Vec::new(),
            Self::TooNew { found, current } => {
                vec![("found", (*found).into()), ("current", (*current).into())]
            }
            Self::TooOld { found, oldest } => {
                vec![("found", (*found).into()), ("oldest", (*oldest).into())]
            }
            Self::NotADowngrade { from, to } => {
                vec![("from", (*from).into()), ("to", (*to).into())]
            }
            Self::NoBack { from } => vec![("from", (*from).into())],
            Self::StepFailed { from, to, cause } => vec![
                ("from", (*from).into()),
                ("to", (*to).into()),
                ("cause", Arg::nested(&**cause)),
            ],
            Self::RoundTrip { version } => vec![("version", (*version).into())],
            Self::WrongType { expected } => vec![("expected", expected.as_str().into())],
            Self::NoLoader { version } => vec![("version", (*version).into())],
            Self::Undecodable {
                version,
                ext_err_txt,
            } => vec![
                ("version", (*version).into()),
                ("ext_err_txt", ext_err_txt.as_str().into()),
            ],
            Self::Caller(report) => report.args(),
        }
    }
}

/// Which map a registry entry is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Forward,
    Back,
}

impl Direction {
    fn word(self) -> &'static str {
        match self {
            Direction::Forward => "forward",
            Direction::Back => "back",
        }
    }
}

/// Why a registry is not built.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RegistryError {
    /// Two steps registered from one version: a second path.
    Duplicate { direction: Direction, from: Version },
    /// A forward step from a version outside `oldest..current`.
    OutOfRange {
        direction: Direction,
        from: Version,
        oldest: Version,
        current: Version,
    },
    /// No forward step from `from`, a version below the current one.
    Gap { from: Version },
    /// A back step from `from` whose forward step (from `from - 1`) is not
    /// in the registry.
    BackWithoutForward { from: Version },
    /// `oldest` is above `current`.
    Inverted { oldest: Version, current: Version },
    /// The forward step from `from` does not produce the object the next
    /// forward step (or, for the last, the registry's current type) takes.
    TypeMismatch { from: Version },
    /// The registry's current type is not what its last forward step makes.
    NotCurrent { from: Version },
    /// A back step from `from` does not go between the types of `from` and
    /// `from - 1`.
    BackTypeMismatch { from: Version },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        coded_diagnostic::display(self, f)
    }
}

impl std::error::Error for RegistryError {}

impl Diagnostic for RegistryError {
    fn code(&self) -> &str {
        match self {
            Self::Duplicate { .. } => code!("registry-duplicate"),
            Self::OutOfRange { .. } => code!("registry-out-of-range"),
            Self::Gap { .. } => code!("registry-gap"),
            Self::BackWithoutForward { .. } => code!("registry-back-without-forward"),
            Self::Inverted { .. } => code!("registry-inverted"),
            Self::TypeMismatch { .. } => code!("registry-type-mismatch"),
            Self::NotCurrent { .. } => code!("registry-not-current"),
            Self::BackTypeMismatch { .. } => code!("registry-back-type-mismatch"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        match self {
            Self::Duplicate { direction, from } => {
                vec![
                    ("direction", direction.word().into()),
                    ("from", (*from).into()),
                ]
            }
            Self::OutOfRange {
                direction,
                from,
                oldest,
                current,
            } => vec![
                ("direction", direction.word().into()),
                ("from", (*from).into()),
                ("oldest", (*oldest).into()),
                ("current", (*current).into()),
            ],
            Self::Gap { from }
            | Self::BackWithoutForward { from }
            | Self::TypeMismatch { from }
            | Self::NotCurrent { from }
            | Self::BackTypeMismatch { from } => {
                vec![("from", (*from).into())]
            }
            Self::Inverted { oldest, current } => {
                vec![("oldest", (*oldest).into()), ("current", (*current).into())]
            }
        }
    }
}

/// This crate's en-US messages, its name first.
pub const EN_US: (&str, &str) = (
    env!("CARGO_PKG_NAME"),
    include_str!(concat!(env!("OUT_DIR"), "/en-US.ftl")),
);

/// One value of every error, for the catalog test.
#[cfg(any(test, feature = "test-support"))]
pub fn diagnostic_samples() -> Vec<Box<dyn Diagnostic>> {
    let inner = MigrateError::RootNotAMapping;
    vec![
        Box::new(MigrateError::NoVersion {
            key: "schema_version".into(),
        }),
        Box::new(MigrateError::BadVersion {
            key: "schema_version".into(),
            found: "-1".into(),
        }),
        Box::new(MigrateError::RootNotAMapping),
        Box::new(MigrateError::TooNew {
            found: 3,
            current: 2,
        }),
        Box::new(MigrateError::TooOld {
            found: 0,
            oldest: 1,
        }),
        Box::new(MigrateError::NotADowngrade { from: 1, to: 2 }),
        Box::new(MigrateError::NoBack { from: 2 }),
        Box::new(MigrateError::StepFailed {
            from: 1,
            to: 2,
            cause: Box::new(inner),
        }),
        Box::new(MigrateError::RoundTrip { version: 1 }),
        Box::new(MigrateError::WrongType {
            expected: "Config".into(),
        }),
        Box::new(MigrateError::NoLoader { version: 1 }),
        Box::new(MigrateError::Undecodable {
            version: 0,
            ext_err_txt: "unexpected end of input".into(),
        }),
        Box::new(RegistryError::Duplicate {
            direction: Direction::Forward,
            from: 1,
        }),
        Box::new(RegistryError::OutOfRange {
            direction: Direction::Forward,
            from: 5,
            oldest: 0,
            current: 2,
        }),
        Box::new(RegistryError::Gap { from: 1 }),
        Box::new(RegistryError::BackWithoutForward { from: 3 }),
        Box::new(RegistryError::Inverted {
            oldest: 3,
            current: 1,
        }),
        Box::new(RegistryError::TypeMismatch { from: 1 }),
        Box::new(RegistryError::NotCurrent { from: 1 }),
        Box::new(RegistryError::BackTypeMismatch { from: 2 }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
