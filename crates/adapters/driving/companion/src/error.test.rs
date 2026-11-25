//! The errors' English, their exits, and the catalog.

use exit_code::{Exit, ExitClass};

presentation::english_for!(&[crate::EN_US.1]);

use super::{diagnostic_samples, LoadoutError};

#[test]
fn test_each_error_says_what_failed_in_its_catalog_english() {
    let english: Vec<String> = diagnostic_samples().iter().map(|d| d.english()).collect();

    assert_eq!(
        english,
        [
            "the loadout is printed only as yaml, not json",
            "the answer could not be written: Broken pipe",
        ]
    );
}

#[test]
fn test_codes_are_prefixed_with_the_crate_name() {
    let codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();

    assert_eq!(
        codes,
        ["companion-loadout-format", "companion-output-failed"]
    );
}

#[test]
fn test_a_refused_format_is_refused_and_a_broken_write_failed() {
    let format = LoadoutError::Format {
        format: "json".into(),
    };
    let output = LoadoutError::Output { detail: "x".into() };

    assert_eq!(format.exit(), Exit::REFUSED);
    assert_eq!(output.exit(), Exit::FAILED);
}

#[test]
fn test_every_error_has_a_catalog_message_with_exactly_its_args() {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    let resource = fluent_syntax::parser::parse(super::EN_US.1).expect("the catalog parses");
    let mut catalog: Vec<String> = Vec::new();
    for entry in &resource.body {
        if let Entry::Message(m) = entry {
            let mut vars: Vec<&str> = m
                .value
                .iter()
                .flat_map(|p| p.elements.iter())
                .filter_map(|e| match e {
                    PatternElement::Placeable {
                        expression: Expression::Inline(InlineExpression::VariableReference { id }),
                    } => Some(id.name),
                    _ => None,
                })
                .collect();
            vars.sort_unstable();
            catalog.push(format!("{}({})", m.id.name, vars.join(",")));
        }
    }
    catalog.sort_unstable();
    let mut sampled: Vec<String> = diagnostic_samples()
        .iter()
        .map(|s| {
            let mut args: Vec<&str> = s.args().into_iter().map(|(n, _)| n).collect();
            args.sort_unstable();
            format!("{}({})", s.code(), args.join(","))
        })
        .collect();
    sampled.sort_unstable();

    assert_eq!(
        catalog, sampled,
        "catalog messages and samples, with their args"
    );
}

#[test]
fn test_display_is_the_code_and_args_never_english() {
    for sample in diagnostic_samples() {
        assert_eq!(
            sample.to_string(),
            coded_diagnostic::Report::of(&*sample).to_string(),
            "{}",
            sample.code()
        );
        assert!(sample.to_string().starts_with(sample.code()), "{sample}");
    }
}
