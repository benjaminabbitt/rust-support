//! A text format as a form: a required [`Loader`] (find the version at the
//! root, read the text as one version's type) and an optional [`Writer`]
//! (write the current object as text).
//!
//! A form with no writer migrates in memory and cannot be written back: the
//! writeback functions take a form that is a [`Writer`], so the mistake does
//! not compile.
//!
//! JSON and TOML are here, over serde. YAML is a consumer's: this repository
//! takes no YAML parser of its own, so the family's one (`angzarr-yaml`)
//! implements [`Loader`] over its own reader, and [`Writer`] when it has a
//! serializer.
//!
//! Mappings are written with serde's field order; comments and layout of a
//! file are not kept.

use std::any::Any;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::FilesError;

/// What a document says its version is, at the root key.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Found {
    /// The root is no mapping.
    NotAMapping,
    /// The key is not there.
    Absent,
    /// An integer.
    Int(i64),
    /// Text, which is a version if it is decimal digits (a YAML scalar is
    /// text).
    Text(String),
    /// Anything else: what it is, as a word (`null`, `list`, `float`).
    Other(&'static str),
}

/// The required half of a form: detect the version, read it as a type.
pub trait Loader {
    /// The format's name, as errors say it (`json`).
    fn name(&self) -> &'static str;

    /// What the text says its version is under `key` at the root.
    ///
    /// # Errors
    /// [`FilesError::Parse`] for text that is not the format.
    fn probe(&self, text: &str, key: &str) -> Result<Found, FilesError>;

    /// The text as a `T`: the type of the version the probe found.
    ///
    /// # Errors
    /// [`FilesError::Parse`] for text that is not a `T`.
    fn read<T: DeserializeOwned + Any>(&self, text: &str) -> Result<T, FilesError>;
}

/// The optional half: write an object as text.
pub trait Writer: Loader {
    /// The object as the format's text.
    ///
    /// # Errors
    /// [`FilesError::Parse`], carrying the serializer's words, for an object
    /// the format cannot hold.
    fn write<T: Serialize>(&self, object: &T) -> Result<String, FilesError>;
}

fn parse_error(format: &str, e: impl std::fmt::Display) -> FilesError {
    FilesError::Parse {
        format: format.into(),
        ext_err_txt: e.to_string(),
    }
}

/// JSON, two-space indented, a final newline.
#[derive(Clone, Copy, Debug, Default)]
pub struct Json;

impl Loader for Json {
    fn name(&self) -> &'static str {
        "json"
    }

    fn probe(&self, text: &str, key: &str) -> Result<Found, FilesError> {
        let doc: serde_json::Value =
            serde_json::from_str(text).map_err(|e| parse_error("json", e))?;
        let serde_json::Value::Object(root) = doc else {
            return Ok(Found::NotAMapping);
        };
        Ok(match root.get(key) {
            None => Found::Absent,
            Some(serde_json::Value::Number(n)) => match n.as_i64() {
                Some(i) => Found::Int(i),
                None if n.is_u64() => Found::Other("number-too-large"),
                None => Found::Other("float"),
            },
            Some(serde_json::Value::String(s)) => Found::Text(s.clone()),
            Some(serde_json::Value::Null) => Found::Other("null"),
            Some(serde_json::Value::Bool(_)) => Found::Other("bool"),
            Some(serde_json::Value::Array(_)) => Found::Other("list"),
            Some(serde_json::Value::Object(_)) => Found::Other("mapping"),
        })
    }

    fn read<T: DeserializeOwned + Any>(&self, text: &str) -> Result<T, FilesError> {
        serde_json::from_str(text).map_err(|e| parse_error("json", e))
    }
}

impl Writer for Json {
    fn write<T: Serialize>(&self, object: &T) -> Result<String, FilesError> {
        let mut text = serde_json::to_string_pretty(object).map_err(|e| parse_error("json", e))?;
        text.push('\n');
        Ok(text)
    }
}

/// TOML; the root is a table.
#[derive(Clone, Copy, Debug, Default)]
pub struct Toml;

impl Loader for Toml {
    fn name(&self) -> &'static str {
        "toml"
    }

    fn probe(&self, text: &str, key: &str) -> Result<Found, FilesError> {
        let root: toml::Table = toml::from_str(text).map_err(|e| parse_error("toml", e))?;
        Ok(match root.get(key) {
            None => Found::Absent,
            Some(toml::Value::Integer(i)) => Found::Int(*i),
            Some(toml::Value::String(s)) => Found::Text(s.clone()),
            Some(toml::Value::Float(_)) => Found::Other("float"),
            Some(toml::Value::Boolean(_)) => Found::Other("bool"),
            Some(toml::Value::Datetime(_)) => Found::Other("datetime"),
            Some(toml::Value::Array(_)) => Found::Other("list"),
            Some(toml::Value::Table(_)) => Found::Other("mapping"),
        })
    }

    fn read<T: DeserializeOwned + Any>(&self, text: &str) -> Result<T, FilesError> {
        toml::from_str(text).map_err(|e| parse_error("toml", e))
    }
}

impl Writer for Toml {
    fn write<T: Serialize>(&self, object: &T) -> Result<String, FilesError> {
        toml::to_string(object).map_err(|e| parse_error("toml", e))
    }
}

#[cfg(test)]
#[path = "format.test.rs"]
mod tests;
