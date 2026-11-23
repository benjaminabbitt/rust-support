//! A diagnostic's JSON.

use crate::Catalog;
use diagnostic::{Arg, Report};

const DEMO: crate::Source = (
    "demo",
    "demo-gave-up = gave up after { $attempts } attempts at { $offset }: { $error }\n\
     demo-busy = the store { $path } is busy\n\
     demo-bare = nothing to say\n",
);

fn json_of(d: &Report) -> String {
    let catalog = Catalog::en_us(&[DEMO]).unwrap();
    serde_json::to_string(&catalog.envelope(d)).unwrap()
}

#[test]
fn test_an_envelope_is_code_args_message_with_args_in_their_own_order() {
    let busy = Report::new("demo-busy").arg("path", "/db");
    let gave_up = Report::new("demo-gave-up")
        .arg("offset", -2i64)
        .arg("attempts", 10u32)
        .arg("error", Arg::nested(&busy));
    assert_eq!(
        json_of(&gave_up),
        concat!(
            r#"{"code":"demo-gave-up","#,
            r#""args":{"offset":-2,"attempts":10,"#,
            r#""error":{"code":"demo-busy","args":{"path":"/db"},"message":"the store /db is busy"}},"#,
            r#""message":"gave up after 10 attempts at -2: the store /db is busy"}"#,
        )
    );
}

#[test]
fn test_no_args_is_an_empty_object() {
    assert_eq!(
        json_of(&Report::new("demo-bare")),
        r#"{"code":"demo-bare","args":{},"message":"nothing to say"}"#
    );
}

// No English crosses. A code no catalog has is its message as its key and args.
#[test]
fn test_an_unknown_code_keeps_its_code_and_args_and_its_message_is_them_never_english() {
    assert_eq!(
        json_of(
            &Report::new("demo-x")
                .arg("n", 3u64)
                .arg("ext_err_txt", "disk I/O error")
        ),
        concat!(
            r#"{"code":"demo-x","args":{"n":3,"ext_err_txt":"disk I/O error"},"#,
            r#""message":"demo-x n=3 ext_err_txt=\"disk I/O error\""}"#,
        )
    );
}
