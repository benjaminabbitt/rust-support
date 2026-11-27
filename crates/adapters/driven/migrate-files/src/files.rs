//! Load a file through a registry, and write the migration back.

use std::any::Any;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use file_system::{Fs, Open, Perms};
use migrate::{MigrateError, Migrated, Obj, Registry, Unstamped, Version, DEFAULT_ROOT_KEY};
use os::Mode;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::format::Found;
use crate::{FilesError, Loader, Writer};

type ReadAs<F> = fn(&F, &str) -> Result<Obj, FilesError>;

fn read_as<F: Loader, V: DeserializeOwned + Any>(
    format: &F,
    text: &str,
) -> Result<Obj, FilesError> {
    format.read::<V>(text).map(|v| Box::new(v) as Obj)
}

/// How a kind of file is read: its format, where its version is, and the type
/// each version loads as.
#[derive(Debug)]
pub struct Layout<F> {
    format: F,
    key: String,
    unstamped: Unstamped,
    loaders: BTreeMap<Version, ReadAs<F>>,
}

impl<F: Loader> Layout<F> {
    /// A layout in `format`, its version under [`DEFAULT_ROOT_KEY`], an
    /// unstamped file refused, and no version yet.
    pub fn new(format: F) -> Layout<F> {
        Layout {
            format,
            key: DEFAULT_ROOT_KEY.to_string(),
            unstamped: Unstamped::Refuse,
            loaders: BTreeMap::new(),
        }
    }

    /// The root key the version is under, instead of [`DEFAULT_ROOT_KEY`].
    #[must_use]
    pub fn root_key(mut self, key: &str) -> Layout<F> {
        self.key = key.to_string();
        self
    }

    /// What a file with no version is; refused unless said.
    #[must_use]
    pub fn unstamped(mut self, unstamped: Unstamped) -> Layout<F> {
        self.unstamped = unstamped;
        self
    }

    /// Text of `version` loads as a `V`.
    #[must_use]
    pub fn at<V: DeserializeOwned + Any>(mut self, version: Version) -> Layout<F> {
        self.loaders.insert(version, read_as::<F, V>);
        self
    }

    /// The version `text` says it is, at the root key, never assumed: no
    /// version is [`MigrateError::NoVersion`] unless the layout reads an
    /// unstamped file as an older one. A version is an integer, or its
    /// decimal digits as text.
    ///
    /// # Errors
    /// [`FilesError::Parse`], and the [`MigrateError`]s `NoVersion`,
    /// `BadVersion` and `RootNotAMapping` as [`FilesError::Migrate`] naming
    /// `path`.
    pub fn detect(&self, path: &Path, text: &str) -> Result<Version, FilesError> {
        let bad = |found: String| MigrateError::BadVersion {
            key: self.key.clone(),
            found,
        };
        let found = match self.format.probe(text, &self.key)? {
            Found::NotAMapping => Err(MigrateError::RootNotAMapping),
            Found::Absent => match self.unstamped {
                Unstamped::As(v) => Ok(v),
                Unstamped::Refuse => Err(MigrateError::NoVersion {
                    key: self.key.clone(),
                }),
            },
            Found::Int(n) => Version::try_from(n).map_err(|_| bad(n.to_string())),
            Found::Text(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
                s.parse().map_err(|_| bad(s.clone()))
            }
            Found::Text(s) => Err(bad(s)),
            Found::Other(kind) => Err(bad(kind.to_string())),
        };
        found.map_err(|cause| migrate_error(path, cause))
    }
}

fn migrate_error(path: &Path, cause: MigrateError) -> FilesError {
    FilesError::Migrate {
        path: path_text(path),
        cause,
    }
}

/// A file read and migrated in memory: the object, and the bytes it came
/// from, which a write-back backs up.
#[derive(Debug)]
pub struct Loaded<C> {
    pub migrated: Migrated<C>,
    path: PathBuf,
    original: Vec<u8>,
}

impl<C> Loaded<C> {
    /// The file it was read from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The object at the current version.
    pub fn object(&self) -> &C {
        &self.migrated.object
    }
}

/// What a write-back did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Written {
    /// The file was current: nothing was written.
    Current,
    /// The file was replaced at the current version, its old bytes copied
    /// to `backup` first.
    Replaced { backup: PathBuf },
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

/// Where the bytes of `path` at version `from` are kept by a write-back:
/// `<file>.pre-<from>`, beside it.
pub fn backup_path(path: &Path, from: Version) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".pre-{from}"));
    path.with_file_name(name)
}

/// Read `path`, detect its version, load it as that version's type and
/// migrate it in memory to the registry's current type. Nothing is written,
/// and a form that cannot write loads here as any other.
///
/// # Errors
/// [`FilesError::Read`], [`Layout::detect`]'s, [`FilesError::Migrate`] for a
/// version the registry does not read (too new, too old) or a step that
/// fails, `NoLoader` the same way for a version the layout has no type for,
/// and the format's [`FilesError::Parse`] for text that is not that version's
/// type.
pub fn load<F: Loader, C: Any>(
    fs: &dyn Fs,
    path: &Path,
    layout: &Layout<F>,
    registry: &Registry<C>,
) -> Result<Loaded<C>, FilesError> {
    let original = fs.read(path).map_err(|e| FilesError::Read {
        path: path_text(path),
        ext_err_txt: e.to_string(),
    })?;
    let text = std::str::from_utf8(&original).map_err(|e| FilesError::Read {
        path: path_text(path),
        ext_err_txt: e.to_string(),
    })?;
    let version = layout.detect(path, text)?;
    registry
        .readable(version)
        .map_err(|cause| migrate_error(path, cause))?;
    let read = layout
        .loaders
        .get(&version)
        .ok_or_else(|| migrate_error(path, MigrateError::NoLoader { version }))?;
    let object = read(&layout.format, text)?;
    let migrated = registry
        .migrate(object, version)
        .map_err(|cause| migrate_error(path, cause))?;
    Ok(Loaded {
        migrated,
        path: path.to_path_buf(),
        original,
    })
}

/// Replace the file `loaded` came from with its migrated object, after a
/// backup of the bytes that were read. Nothing happens unless the object was
/// migrated; the file is not touched unless the backup was made; a file
/// written by someone else since the read is refused; and an object that
/// would not read back as itself, at the current version, is not written.
/// Only a form with a [`Writer`] can be written back.
///
/// # Errors
/// The writer's, [`FilesError::StampMismatch`], [`FilesError::NotRoundTrip`],
/// [`FilesError::ChangedSinceRead`], [`FilesError::BackupExists`],
/// [`FilesError::Backup`] and [`FilesError::Write`]; on any of them before
/// the replace, the file is as it was.
pub fn write_back<F, C>(
    fs: &dyn Fs,
    layout: &Layout<F>,
    registry: &Registry<C>,
    loaded: &Loaded<C>,
) -> Result<Written, FilesError>
where
    F: Writer,
    C: Any + Serialize + DeserializeOwned + PartialEq,
{
    if !loaded.migrated.changed() {
        return Ok(Written::Current);
    }
    let path = loaded.path.as_path();
    let format = layout.format.name().to_string();
    let text = layout.format.write(loaded.object())?;
    let found = layout.detect(path, &text)?;
    if found != registry.current() {
        return Err(FilesError::StampMismatch {
            format,
            found,
            current: registry.current(),
        });
    }
    if layout.format.read::<C>(&text)? != *loaded.object() {
        return Err(FilesError::NotRoundTrip { format });
    }
    let now = fs.read(path).map_err(|e| FilesError::Read {
        path: path_text(path),
        ext_err_txt: e.to_string(),
    })?;
    if now != loaded.original {
        return Err(FilesError::ChangedSinceRead {
            path: path_text(path),
        });
    }
    let backup = backup_path(path, loaded.migrated.from);
    let failed = |e: std::io::Error| FilesError::Backup {
        path: path_text(&backup),
        ext_err_txt: e.to_string(),
    };
    match fs.open(&backup, Open::CreateNew(Mode::PRIVATE_FILE)) {
        Ok(mut file) => {
            file.write_all(&loaded.original).map_err(failed)?;
            file.sync_all().map_err(failed)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(FilesError::BackupExists {
                path: path_text(&backup),
            })
        }
        Err(e) => return Err(failed(e)),
    }
    fs.write_atomic(path, text.as_bytes(), Perms::KeepOr(Mode::PRIVATE_FILE))
        .map_err(|e| FilesError::Write {
            path: path_text(path),
            ext_err_txt: e.to_string(),
        })?;
    Ok(Written::Replaced { backup })
}

#[cfg(test)]
#[path = "files.test.rs"]
mod tests;
