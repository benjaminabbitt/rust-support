//! The protobuf form: a message's bytes loaded into the prost type of the
//! version they were written as.
//!
//! A protobuf message does not say what version it is, so the caller tells
//! it: a column that stamps each stored row, a header, or a test of the
//! bytes' shape. The version types are prost messages, the frozen copy of
//! each package's message (an old version is a copy that never changes),
//! and a step between two of them is an ordinary `TryFrom`.

use std::any::Any;
use std::collections::BTreeMap;

use crate::registry::Obj;
use crate::{MigrateError, Migrated, Registry, Version};

type Decoder = fn(&[u8]) -> Result<Obj, String>;

fn decode_as<V: prost::Message + Default + Any>(bytes: &[u8]) -> Result<Obj, String> {
    V::decode(bytes)
        .map(|v| Box::new(v) as Obj)
        .map_err(|e| e.to_string())
}

/// The prost type each version's bytes decode as.
#[derive(Debug, Default)]
pub struct Decoders {
    by_version: BTreeMap<Version, Decoder>,
}

impl Decoders {
    /// No version yet.
    pub fn new() -> Decoders {
        Decoders::default()
    }

    /// Bytes of `version` decode as a `V`.
    #[must_use]
    pub fn at<V: prost::Message + Default + Any>(mut self, version: Version) -> Decoders {
        self.by_version.insert(version, decode_as::<V>);
        self
    }
}

/// Decode `bytes`, written as version `from`, as that version's type and
/// carry it up to the registry's current type, in memory.
///
/// # Errors
/// [`Registry::readable`]'s, [`MigrateError::NoLoader`] for a version with no
/// decoder, [`MigrateError::Undecodable`] for bytes that are not that
/// version's message, and [`Registry::migrate`]'s.
pub fn load<C: Any>(
    registry: &Registry<C>,
    decoders: &Decoders,
    bytes: &[u8],
    from: Version,
) -> Result<Migrated<C>, MigrateError> {
    registry.readable(from)?;
    let decode = decoders
        .by_version
        .get(&from)
        .ok_or(MigrateError::NoLoader { version: from })?;
    let object = decode(bytes).map_err(|ext_err_txt| MigrateError::Undecodable {
        version: from,
        ext_err_txt,
    })?;
    registry.migrate(object, from)
}

/// A current object as the bytes a store keeps.
pub fn encode<C: prost::Message>(object: &C) -> Vec<u8> {
    object.encode_to_vec()
}

#[cfg(test)]
#[path = "prost.test.rs"]
mod tests;
