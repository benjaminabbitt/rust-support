use serde::{Deserialize, Serialize};

use super::{Found, Json, Loader, Toml, Writer};
use crate::FilesError;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Doc {
    schema_version: u32,
    on: bool,
    ratio: f64,
    tags: Vec<String>,
    inner: Inner,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Inner {
    name: String,
}

fn doc() -> Doc {
    Doc {
        schema_version: 2,
        on: true,
        ratio: 0.5,
        tags: vec!["a".into(), "b".into()],
        inner: Inner { name: "x".into() },
    }
}

fn parse_error<T: std::fmt::Debug>(r: Result<T, FilesError>) -> bool {
    matches!(r, Err(FilesError::Parse { .. }))
}

#[test]
fn json_round_trips_a_fixture_through_its_type() {
    let text = Json.write(&doc()).unwrap();
    assert!(text.ends_with("}\n"));
    assert!(text.contains("  \"schema_version\": 2"));
    assert_eq!(Json.read::<Doc>(&text).unwrap(), doc());
    assert_eq!(Json.name(), "json");
}

#[test]
fn toml_round_trips_a_fixture_through_its_type() {
    let text = Toml.write(&doc()).unwrap();
    assert!(text.contains("schema_version = 2"));
    assert!(text.contains("[inner]"));
    assert_eq!(Toml.read::<Doc>(&text).unwrap(), doc());
    assert_eq!(Toml.name(), "toml");
}

#[test]
fn text_that_is_not_the_type_is_a_parse_error() {
    assert!(parse_error(Json.read::<Doc>("{")));
    assert!(parse_error(Json.read::<Doc>("{\"schema_version\": 1}")));
    assert!(parse_error(Toml.read::<Doc>("a = ")));
    assert!(parse_error(Toml.read::<Doc>("schema_version = 1")));
    assert!(parse_error(Json.probe("{", "schema_version")));
    assert!(parse_error(Toml.probe("a = ", "schema_version")));
}

#[test]
fn a_type_the_format_cannot_hold_is_a_parse_error_with_the_serializers_words() {
    // TOML has no null, and no root that is not a table.
    assert!(parse_error(Toml.write(&Option::<i64>::None)));
    assert!(parse_error(Toml.write(&1_i64)));
}

#[test]
fn json_probes_the_root_key_of_each_kind() {
    let probe = |text: &str| Json.probe(text, "v").unwrap();
    assert_eq!(probe("{\"v\": 3}"), Found::Int(3));
    assert_eq!(probe("{\"v\": -1}"), Found::Int(-1));
    assert_eq!(probe("{\"v\": \"3\"}"), Found::Text("3".into()));
    assert_eq!(probe("{\"w\": 3}"), Found::Absent);
    assert_eq!(probe("{\"v\": 1.5}"), Found::Other("float"));
    assert_eq!(
        probe("{\"v\": 18446744073709551615}"),
        Found::Other("number-too-large")
    );
    assert_eq!(probe("{\"v\": null}"), Found::Other("null"));
    assert_eq!(probe("{\"v\": true}"), Found::Other("bool"));
    assert_eq!(probe("{\"v\": []}"), Found::Other("list"));
    assert_eq!(probe("{\"v\": {}}"), Found::Other("mapping"));
    assert_eq!(probe("[1]"), Found::NotAMapping);
}

#[test]
fn toml_probes_the_root_key_of_each_kind() {
    let probe = |text: &str| Toml.probe(text, "v").unwrap();
    assert_eq!(probe("v = 3"), Found::Int(3));
    assert_eq!(probe("v = \"3\""), Found::Text("3".into()));
    assert_eq!(probe(""), Found::Absent);
    assert_eq!(probe("v = 1.5"), Found::Other("float"));
    assert_eq!(probe("v = true"), Found::Other("bool"));
    assert_eq!(probe("v = 1979-05-27T07:32:00Z"), Found::Other("datetime"));
    assert_eq!(probe("v = []"), Found::Other("list"));
    assert_eq!(probe("[v]\na = 1"), Found::Other("mapping"));
}

#[test]
fn the_probe_reads_only_the_root_key_whatever_else_the_text_holds() {
    let text = "{\"schema_version\": 4, \"anything\": {\"else\": [1, 2]}}";
    assert_eq!(Json.probe(text, "schema_version").unwrap(), Found::Int(4));
}
