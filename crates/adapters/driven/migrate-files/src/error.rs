//! Why a file does not load or write back: one code each.

use std::fmt;

use coded_diagnostic::{code, Arg, Diagnostic};
use migrate::{MigrateError, Version};

/// Why a file does not load, or does not write back.
#[derive(Clone, PartialEq, Debug)]
pub enum FilesError {
    /// The text is not `format`; the parser's words ride as `ext_err_txt`.
    Parse { format: String, ext_err_txt: String },
    /// The document, written as `format`, does not read back as itself.
    NotRoundTrip { format: String },
    /// The object, written, would be read as another version than the
    /// current: its type does not carry its version as the layout reads it.
    StampMismatch {
        format: String,
        found: Version,
        current: Version,
    },
    /// The file could not be read.
    Read { path: String, ext_err_txt: String },
    /// The file could not be replaced.
    Write { path: String, ext_err_txt: String },
    /// A backup is already there: one is never overwritten.
    BackupExists { path: String },
    /// The backup could not be made; the file was not touched.
    Backup { path: String, ext_err_txt: String },
    /// The file is not the bytes that were migrated: someone wrote it
    /// since; nothing was written.
    ChangedSinceRead { path: String },
    /// The document does not migrate.
    Migrate { path: String, cause: MigrateError },
}

impl fmt::Display for FilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        coded_diagnostic::display(self, f)
    }
}

impl std::error::Error for FilesError {}

impl Diagnostic for FilesError {
    fn code(&self) -> &str {
        match self {
            Self::Parse { .. } => code!("parse"),
            Self::NotRoundTrip { .. } => code!("not-round-trip"),
            Self::StampMismatch { .. } => code!("stamp-mismatch"),
            Self::Read { .. } => code!("read"),
            Self::Write { .. } => code!("write"),
            Self::BackupExists { .. } => code!("backup-exists"),
            Self::Backup { .. } => code!("backup"),
            Self::ChangedSinceRead { .. } => code!("changed-since-read"),
            Self::Migrate { .. } => code!("migrate"),
        }
    }

    fn args(&self) -> Vec<(&str, Arg)> {
        let s = |v: &String| Arg::from(v.as_str());
        match self {
            Self::Parse {
                format,
                ext_err_txt,
            } => {
                vec![("format", s(format)), ("ext_err_txt", s(ext_err_txt))]
            }
            Self::NotRoundTrip { format } => vec![("format", s(format))],
            Self::StampMismatch {
                format,
                found,
                current,
            } => vec![
                ("format", s(format)),
                ("found", (*found).into()),
                ("current", (*current).into()),
            ],
            Self::Read { path, ext_err_txt }
            | Self::Write { path, ext_err_txt }
            | Self::Backup { path, ext_err_txt } => {
                vec![("path", s(path)), ("ext_err_txt", s(ext_err_txt))]
            }
            Self::BackupExists { path } | Self::ChangedSinceRead { path } => {
                vec![("path", s(path))]
            }
            Self::Migrate { path, cause } => {
                vec![("path", s(path)), ("cause", Arg::nested(cause))]
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
    let s = |v: &str| v.to_string();
    vec![
        Box::new(FilesError::Parse {
            format: s("json"),
            ext_err_txt: s("expected value at line 1"),
        }),
        Box::new(FilesError::NotRoundTrip { format: s("toml") }),
        Box::new(FilesError::StampMismatch {
            format: s("toml"),
            found: 1,
            current: 2,
        }),
        Box::new(FilesError::Read {
            path: s("/a/b"),
            ext_err_txt: s("No such file or directory (os error 2)"),
        }),
        Box::new(FilesError::Write {
            path: s("/a/b"),
            ext_err_txt: s("Permission denied (os error 13)"),
        }),
        Box::new(FilesError::BackupExists {
            path: s("/a/b.pre-1"),
        }),
        Box::new(FilesError::Backup {
            path: s("/a/b.pre-1"),
            ext_err_txt: s("Permission denied (os error 13)"),
        }),
        Box::new(FilesError::ChangedSinceRead { path: s("/a/b") }),
        Box::new(FilesError::Migrate {
            path: s("/a/b"),
            cause: MigrateError::TooNew {
                found: 3,
                current: 2,
            },
        }),
    ]
}

#[cfg(test)]
#[path = "error.test.rs"]
mod tests;
