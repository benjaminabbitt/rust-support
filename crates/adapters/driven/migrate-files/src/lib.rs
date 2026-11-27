//! Text files as forms of the `migrate` registry: a form detects the version
//! at the root of a document and loads straight into that version's typed
//! object; the registry carries the object up in memory; writing it back is
//! a separate, explicit step.
//!
//! - A form is a [`Loader`] (required: the version, and the text read as a
//!   type) and, optionally, a [`Writer`]. JSON and TOML are here, over serde;
//!   a YAML form is the consumer's.
//! - A [`Layout`] says where a kind of file keeps its version and which type
//!   each version loads as.
//! - [`load`] reads a file, detects its version, loads and migrates it. It
//!   writes nothing, and works for a form that cannot write.
//! - [`write_back`] needs a [`Writer`]: render the current object (and see it
//!   read back as itself, at the current version), check the file is still
//!   the bytes that were read, copy those bytes to a backup (`<file>.pre-<from>`,
//!   never overwriting one), then replace the file atomically. An object
//!   that needed no migration writes nothing.
//!
//! Every file operation goes through [`file_system::Fs`].

mod error;
mod files;
mod format;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{FilesError, EN_US};
pub use files::{backup_path, load, write_back, Layout, Loaded, Written};
pub use format::{Found, Json, Loader, Toml, Writer};
