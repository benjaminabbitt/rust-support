//! The companion contract: `version` and `loadout`.

use presentation::{render, Format};

use super::{loadout, Asked, LoadoutError, Version};

const DOCUMENT: &str = "# demo's loadout\nrun:\n  version: 1.0.0\n";

fn printed(asked: Option<Asked>) -> (Result<(), LoadoutError>, String) {
    let mut out = Vec::new();
    let result = loadout(asked, DOCUMENT, &mut out);
    (result, String::from_utf8(out).expect("utf-8"))
}

#[test]
fn test_asked_reads_yaml_and_presentations_formats() {
    assert_eq!("yaml".parse::<Asked>(), Ok(Asked::Yaml));
    assert_eq!("json".parse::<Asked>(), Ok(Asked::Format(Format::Json)));
    assert_eq!("text".parse::<Asked>(), Ok(Asked::Format(Format::Text)));
}

#[test]
fn test_asked_refuses_any_other_value_naming_it() {
    let refused = "toml".parse::<Asked>().expect_err("toml is no format");

    assert_eq!(refused.value, "toml");
}

#[test]
fn test_loadout_prints_the_document_verbatim_as_yaml() {
    assert_eq!(printed(Some(Asked::Yaml)), (Ok(()), DOCUMENT.to_string()));
}

#[test]
fn test_loadout_with_no_format_asked_is_yaml() {
    assert_eq!(printed(None), (Ok(()), DOCUMENT.to_string()));
}

#[test]
fn test_loadout_refuses_text_and_json_printing_nothing() {
    for (format, name) in [(Format::Json, "json"), (Format::Text, "text")] {
        assert_eq!(
            printed(Some(Asked::Format(format))),
            (
                Err(LoadoutError::Format {
                    format: name.to_string()
                }),
                String::new()
            ),
            "{name}"
        );
    }
}

#[test]
fn test_loadout_a_write_that_breaks_is_an_output_failure() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let result = loadout(None, DOCUMENT, &mut Broken);

    assert!(
        matches!(&result, Err(LoadoutError::Output { detail }) if detail.contains("gone")),
        "{result:?}"
    );
}

fn rendered(format: Format, version: &Version) -> String {
    let mut out = Vec::new();
    render(&mut out, format, version).expect("written");
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn test_version_json_is_name_version_lock_protocol_in_order() {
    let v = Version {
        name: "demo",
        version: "1.2.3",
        lock_protocol: 2,
    };

    let json: serde_json::Value = serde_json::from_str(&rendered(Format::Json, &v)).expect("json");

    assert_eq!(
        json,
        serde_json::json!({"name": "demo", "version": "1.2.3", "lock_protocol": 2})
    );
    let keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["name", "version", "lock_protocol"]);
}

#[test]
fn test_version_json_leaves_out_a_zero_lock_protocol() {
    let v = Version {
        name: "demo",
        version: "1.2.3",
        lock_protocol: 0,
    };

    let json: serde_json::Value = serde_json::from_str(&rendered(Format::Json, &v)).expect("json");

    assert_eq!(
        json,
        serde_json::json!({"name": "demo", "version": "1.2.3"})
    );
}

#[test]
fn test_version_text_is_the_bare_version_line() {
    let v = Version {
        name: "demo",
        version: "1.2.3",
        lock_protocol: 2,
    };

    assert_eq!(rendered(Format::Text, &v), "1.2.3\n");
}
