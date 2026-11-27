use std::path::Path;

use file_system::{Fs, Perms, Rooted};
use migrate::{MigrateError, Registry, Unstamped};
use serde::{Deserialize, Serialize};

use super::{backup_path, load, write_back, Layout, Written};
use crate::{FilesError, Found, Json, Loader, Toml, Writer};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct ConfigV1 {
    schema_version: u32,
    a: i64,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct ConfigV2 {
    schema_version: u32,
    b: i64,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct ConfigV3 {
    schema_version: u32,
    c: i64,
    tags: Vec<String>,
}

impl From<ConfigV1> for ConfigV2 {
    fn from(old: ConfigV1) -> ConfigV2 {
        ConfigV2 {
            schema_version: 2,
            b: old.a,
        }
    }
}
impl From<ConfigV2> for ConfigV3 {
    fn from(old: ConfigV2) -> ConfigV3 {
        ConfigV3 {
            schema_version: 3,
            c: old.b,
            tags: vec!["migrated".into()],
        }
    }
}
impl From<ConfigV3> for ConfigV2 {
    fn from(new: ConfigV3) -> ConfigV2 {
        ConfigV2 {
            schema_version: 2,
            b: new.c,
        }
    }
}

/// Versions 1 to 3: `a` becomes `b`, then `c`.
fn registry() -> Registry<ConfigV3> {
    Registry::builder(1, 3)
        .forward::<ConfigV1, ConfigV2>(1)
        .forward::<ConfigV2, ConfigV3>(2)
        .back::<ConfigV3, ConfigV2>(3)
        .build()
        .unwrap()
}

fn layout<F: Loader>(format: F) -> Layout<F> {
    Layout::new(format)
        .at::<ConfigV1>(1)
        .at::<ConfigV2>(2)
        .at::<ConfigV3>(3)
}

fn fs() -> Rooted {
    Rooted::temp().unwrap()
}

fn put(fs: &Rooted, path: &str, text: &str) {
    fs.write_atomic(
        Path::new(path),
        text.as_bytes(),
        Perms::Exact(os::Mode::PRIVATE_FILE),
    )
    .unwrap();
}

fn read(fs: &Rooted, path: &str) -> String {
    fs.read_to_string(Path::new(path)).unwrap()
}

const OLD_JSON: &str = "{\n  \"schema_version\": 1,\n  \"a\": 5\n}\n";
const OLD_TOML: &str = "# kept in the backup\nschema_version = 1\na = 5\n";

fn migrated_in_memory_and_nothing_written<F: Loader>(format: F, text: &str, file: &str) {
    let fs = fs();
    put(&fs, file, text);
    let loaded = load(&fs, Path::new(file), &layout(format), &registry()).unwrap();
    assert_eq!(loaded.migrated.applied, [1, 2]);
    assert_eq!(
        loaded.object(),
        &ConfigV3 {
            schema_version: 3,
            c: 5,
            tags: vec!["migrated".into()]
        }
    );
    assert_eq!(loaded.path(), Path::new(file));
    assert_eq!(read(&fs, file), text);
    assert!(!fs.exists(&backup_path(Path::new(file), 1)).unwrap());
}

#[test]
fn json_loads_into_its_versions_object_and_migrates_in_memory() {
    migrated_in_memory_and_nothing_written(Json, OLD_JSON, "/c.json");
}

#[test]
fn toml_loads_into_its_versions_object_and_migrates_in_memory() {
    migrated_in_memory_and_nothing_written(Toml, OLD_TOML, "/c.toml");
}

fn written_back_with_a_backup_first<F: Writer>(format: F, text: &str, file: &str) {
    let fs = fs();
    put(&fs, file, text);
    let path = Path::new(file);
    let layout = layout(format);
    let loaded = load(&fs, path, &layout, &registry()).unwrap();
    let written = write_back(&fs, &layout, &registry(), &loaded).unwrap();
    let backup = backup_path(path, 1);
    assert_eq!(
        written,
        Written::Replaced {
            backup: backup.clone()
        }
    );
    assert_eq!(fs.read_to_string(&backup).unwrap(), text);
    // The file now reads as current, through no step.
    let again = load(&fs, path, &layout, &registry()).unwrap();
    assert!(!again.migrated.changed());
    assert_eq!(again.object(), loaded.object());
}

#[test]
fn json_is_written_back_after_a_backup() {
    written_back_with_a_backup_first(Json, OLD_JSON, "/c.json");
    assert_eq!(
        backup_path(Path::new("/c.json"), 1),
        Path::new("/c.json.pre-1")
    );
}

#[test]
fn toml_is_written_back_after_a_backup() {
    written_back_with_a_backup_first(Toml, OLD_TOML, "/c.toml");
}

#[test]
fn a_current_file_is_not_written_back() {
    let fs = fs();
    let text = "{\"schema_version\": 3, \"c\": 1, \"tags\": []}";
    put(&fs, "/c.json", text);
    let layout = layout(Json);
    let loaded = load(&fs, Path::new("/c.json"), &layout, &registry()).unwrap();
    assert_eq!(
        write_back(&fs, &layout, &registry(), &loaded),
        Ok(Written::Current)
    );
    assert_eq!(read(&fs, "/c.json"), text);
    assert!(!fs.exists(&backup_path(Path::new("/c.json"), 3)).unwrap());
}

#[test]
fn a_backup_already_there_is_never_overwritten_and_the_file_is_untouched() {
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    put(&fs, "/c.json.pre-1", "precious");
    let layout = layout(Json);
    let loaded = load(&fs, Path::new("/c.json"), &layout, &registry()).unwrap();
    assert_eq!(
        write_back(&fs, &layout, &registry(), &loaded),
        Err(FilesError::BackupExists {
            path: "/c.json.pre-1".into()
        })
    );
    assert_eq!(read(&fs, "/c.json"), OLD_JSON);
    assert_eq!(read(&fs, "/c.json.pre-1"), "precious");
}

#[test]
fn a_file_changed_since_it_was_read_is_not_overwritten() {
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    let layout = layout(Json);
    let loaded = load(&fs, Path::new("/c.json"), &layout, &registry()).unwrap();
    put(&fs, "/c.json", "{\"schema_version\": 1, \"a\": 6}");
    assert_eq!(
        write_back(&fs, &layout, &registry(), &loaded),
        Err(FilesError::ChangedSinceRead {
            path: "/c.json".into()
        })
    );
    assert!(!fs.exists(&backup_path(Path::new("/c.json"), 1)).unwrap());
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Stale {
    schema_version: u32,
}
impl From<ConfigV1> for Stale {
    fn from(old: ConfigV1) -> Stale {
        // A step that forgets to say the new version.
        Stale {
            schema_version: old.schema_version,
        }
    }
}

#[test]
fn an_object_that_would_not_read_as_current_is_not_written() {
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    let registry = Registry::<Stale>::builder(1, 2)
        .forward::<ConfigV1, Stale>(1)
        .build()
        .unwrap();
    let layout = Layout::new(Json).at::<ConfigV1>(1).at::<Stale>(2);
    let loaded = load(&fs, Path::new("/c.json"), &layout, &registry).unwrap();
    assert_eq!(
        write_back(&fs, &layout, &registry, &loaded),
        Err(FilesError::StampMismatch {
            format: "json".into(),
            found: 1,
            current: 2
        })
    );
    assert_eq!(read(&fs, "/c.json"), OLD_JSON);
    assert!(!fs.exists(&backup_path(Path::new("/c.json"), 1)).unwrap());
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Lossy {
    schema_version: u32,
    #[serde(skip)]
    only_in_memory: i64,
}
impl From<ConfigV1> for Lossy {
    fn from(_: ConfigV1) -> Lossy {
        Lossy {
            schema_version: 2,
            only_in_memory: 9,
        }
    }
}

#[test]
fn an_object_that_would_not_read_back_as_itself_is_not_written() {
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    let registry = Registry::<Lossy>::builder(1, 2)
        .forward::<ConfigV1, Lossy>(1)
        .build()
        .unwrap();
    let layout = Layout::new(Json).at::<ConfigV1>(1).at::<Lossy>(2);
    let loaded = load(&fs, Path::new("/c.json"), &layout, &registry).unwrap();
    assert_eq!(
        write_back(&fs, &layout, &registry, &loaded),
        Err(FilesError::NotRoundTrip {
            format: "json".into()
        })
    );
    assert_eq!(read(&fs, "/c.json"), OLD_JSON);
}

/// A form with a loader and no writer: JSON, read-only.
struct ReadOnly;
impl Loader for ReadOnly {
    fn name(&self) -> &'static str {
        "read-only"
    }
    fn probe(&self, text: &str, key: &str) -> Result<Found, FilesError> {
        Json.probe(text, key)
    }
    fn read<T: serde::de::DeserializeOwned + std::any::Any>(
        &self,
        text: &str,
    ) -> Result<T, FilesError> {
        Json.read(text)
    }
}

#[test]
fn a_form_with_no_writer_loads_and_migrates() {
    // `write_back(&fs, &layout(ReadOnly), ..)` does not compile: ReadOnly is
    // no `Writer`.
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    let loaded = load(&fs, Path::new("/c.json"), &layout(ReadOnly), &registry()).unwrap();
    assert_eq!(loaded.object().c, 5);
}

fn why_it_does_not_load(text: &[u8]) -> FilesError {
    let fs = fs();
    fs.write_atomic(
        Path::new("/c.json"),
        text,
        Perms::Exact(os::Mode::PRIVATE_FILE),
    )
    .unwrap();
    load(&fs, Path::new("/c.json"), &layout(Json), &registry()).unwrap_err()
}

fn cause(e: FilesError) -> MigrateError {
    match e {
        FilesError::Migrate { cause, .. } => cause,
        other => panic!("{other:?}"),
    }
}

#[test]
fn load_says_why_a_file_does_not_load() {
    let fs = fs();
    assert!(matches!(
        load(&fs, Path::new("/missing"), &layout(Json), &registry()),
        Err(FilesError::Read { .. })
    ));
    assert!(matches!(
        why_it_does_not_load(&[0xff, 0xfe]),
        FilesError::Read { .. }
    ));
    assert!(matches!(
        why_it_does_not_load(b"{"),
        FilesError::Parse { .. }
    ));
    // The version is detected and never assumed.
    assert!(matches!(
        cause(why_it_does_not_load(b"{}")),
        MigrateError::NoVersion { .. }
    ));
    assert_eq!(
        cause(why_it_does_not_load(b"[]")),
        MigrateError::RootNotAMapping
    );
    assert_eq!(
        cause(why_it_does_not_load(b"{\"schema_version\": 4}")),
        MigrateError::TooNew {
            found: 4,
            current: 3
        }
    );
    assert_eq!(
        cause(why_it_does_not_load(b"{\"schema_version\": 0}")),
        MigrateError::TooOld {
            found: 0,
            oldest: 1
        }
    );
    for (text, found) in [
        ("{\"schema_version\": -1}", "-1"),
        ("{\"schema_version\": \"abc\"}", "abc"),
        ("{\"schema_version\": \"\"}", ""),
        ("{\"schema_version\": \"+1\"}", "+1"),
        ("{\"schema_version\": \"1a\"}", "1a"),
        ("{\"schema_version\": \"99999999999\"}", "99999999999"),
        ("{\"schema_version\": 1099511627776}", "1099511627776"),
        ("{\"schema_version\": null}", "null"),
    ] {
        assert_eq!(
            cause(why_it_does_not_load(text.as_bytes())),
            MigrateError::BadVersion {
                key: "schema_version".into(),
                found: found.into()
            },
            "{text}"
        );
    }
}

#[test]
fn a_version_written_as_digits_of_text_is_read() {
    let fs = fs();
    put(&fs, "/c.json", "{\"schema_version\": \"2\", \"b\": 1}");
    // The type of version 2 reads its own stamp as a number: the text form
    // is only the probe's, so the file is refused by the type, not assumed.
    let e = load(&fs, Path::new("/c.json"), &layout(Json), &registry()).unwrap_err();
    assert!(matches!(e, FilesError::Parse { .. }));
}

#[test]
fn a_version_the_layout_has_no_type_for_is_refused() {
    let fs = fs();
    put(&fs, "/c.json", OLD_JSON);
    let layout = Layout::new(Json).at::<ConfigV2>(2).at::<ConfigV3>(3);
    let e = load(&fs, Path::new("/c.json"), &layout, &registry()).unwrap_err();
    assert_eq!(cause(e), MigrateError::NoLoader { version: 1 });
}

#[test]
fn the_layout_names_its_own_root_key() {
    let fs = fs();
    put(
        &fs,
        "/c.json",
        "{\"v\": 3, \"schema_version\": 3, \"c\": 1, \"tags\": []}",
    );
    let l = layout(Json).root_key("v");
    assert_eq!(l.detect(Path::new("/c.json"), &read(&fs, "/c.json")), Ok(3));
}

#[test]
fn an_unstamped_legacy_file_is_the_version_the_layout_says_when_it_says() {
    let fs = fs();
    // Unstamped is read as version 1 by a layout that says so.
    put(&fs, "/bare.json", "{\"a\": 1}");
    let registry = registry();
    let layout = Layout::new(Json)
        .at::<ConfigV1>(1)
        .at::<ConfigV2>(2)
        .at::<ConfigV3>(3)
        .unstamped(Unstamped::As(1));
    // Version 1's type needs its stamp: the legacy file lacks it, so the type
    // refuses the text, and the layout shows no assumption beyond the version.
    assert_eq!(layout.detect(Path::new("/bare.json"), "{\"a\": 1}"), Ok(1));
    assert!(matches!(
        load(&fs, Path::new("/bare.json"), &layout, &registry),
        Err(FilesError::Parse { .. })
    ));
}

#[test]
fn an_unstamped_file_migrates_when_its_type_defaults_the_stamp() {
    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    struct Legacy {
        #[serde(default)]
        schema_version: u32,
        a: i64,
    }
    impl From<Legacy> for ConfigV2 {
        fn from(old: Legacy) -> ConfigV2 {
            ConfigV2 {
                schema_version: 2,
                b: old.a,
            }
        }
    }
    let registry = Registry::<ConfigV3>::builder(1, 3)
        .forward::<Legacy, ConfigV2>(1)
        .forward::<ConfigV2, ConfigV3>(2)
        .build()
        .unwrap();
    let layout = Layout::new(Json)
        .at::<Legacy>(1)
        .at::<ConfigV2>(2)
        .at::<ConfigV3>(3)
        .unstamped(Unstamped::As(1));
    let fs = fs();
    put(&fs, "/bare.json", "{\"a\": 1}");
    let loaded = load(&fs, Path::new("/bare.json"), &layout, &registry).unwrap();
    assert_eq!(loaded.object().c, 1);
    assert!(matches!(
        write_back(&fs, &layout, &registry, &loaded),
        Ok(Written::Replaced { .. })
    ));
    assert_eq!(read(&fs, "/bare.json.pre-1"), "{\"a\": 1}");
}

#[test]
fn a_backup_that_cannot_be_made_is_not_a_backup_that_exists() {
    let fs = fs();
    fs.create_dir_all(Path::new("/ro"), os::Mode::DIR).unwrap();
    put(&fs, "/ro/c.json", OLD_JSON);
    let layout = layout(Json);
    let loaded = load(&fs, Path::new("/ro/c.json"), &layout, &registry()).unwrap();
    fs.set_permissions(Path::new("/ro"), os::Mode::from_bits(0o500))
        .unwrap();
    let written = write_back(&fs, &layout, &registry(), &loaded);
    fs.set_permissions(Path::new("/ro"), os::Mode::DIR).unwrap();
    match written {
        Err(FilesError::Backup { path, .. }) => assert_eq!(path, "/ro/c.json.pre-1"),
        // A process that ignores directory modes (root) makes the backup.
        Ok(Written::Replaced { .. }) => {}
        other => panic!("{other:?}"),
    }
}
