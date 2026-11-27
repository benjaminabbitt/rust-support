//! The suite's migration registry for anything on disk. Pure: it reads no
//! file, no clock, no environment; the forms that load and store (files,
//! protobuf, databases) are adapters over it.
//!
//! # The rule
//!
//! Every on-disk artifact carries a monotonically increasing integer
//! [`Version`], **detected** by its form (at the root of a document, under a
//! root key; in a stamp beside a database row) and never assumed: a
//! document with no readable version is a typed error, unless the form says,
//! for a kind written before versions, that unstamped is an older version
//! ([`Unstamped::As`]). The document is migrated **in memory first**: the
//! form loads it as its own version's object, the registry carries it up to
//! the current one, and it is used. Storing it back is a separate, explicit
//! step, with a backup taken first, and only a form that has a writer can.
//!
//! # The registry
//!
//! A [`Registry`] is two maps keyed on the detected version, `forward[v]`
//! stepping `v` to `v + 1` and `back[v]` stepping `v` to `v - 1`
//! ([`BTreeMap`](std::collections::BTreeMap)s, so iteration is
//! deterministic). Each version is a type of its own and a step is the
//! `TryFrom` between two of them ([`Builder::forward`], [`Builder::back`]),
//! stored as a plain `fn` pointer: pure, no captured state. Chained single
//! steps only: 1 to 2 to 3, never 1 to 3, one forward step per version.
//! Forward is required for every version from the oldest to the current;
//! back is optional, and where present must round-trip
//! ([`Registry::round_trip`]). [`Builder::build`] refuses a registry that
//! breaks any of that, the types that do not line up included.
//!
//! The registry knows no format. The `prost` feature adds the protobuf form.

mod error;
#[cfg(feature = "prost")]
pub mod prost;
mod registry;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{Direction, MigrateError, RegistryError, EN_US};
pub use registry::{Builder, Migrated, Obj, Registry, RoundTrip, Step};

/// A document's version: an integer that only ever increases.
pub type Version = u32;

/// The root key a document's version is read from unless its form names its
/// own.
pub const DEFAULT_ROOT_KEY: &str = "schema_version";

/// What a document with no version is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Unstamped {
    /// Refused: [`MigrateError::NoVersion`]. The default; a version is
    /// never assumed.
    #[default]
    Refuse,
    /// Written before versions were: read as this version, so a registry
    /// that has steps for that generation migrates it.
    As(Version),
}
