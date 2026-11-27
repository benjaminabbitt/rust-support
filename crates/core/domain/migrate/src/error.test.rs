use coded_diagnostic::Diagnostic;

use super::{diagnostic_samples, MigrateError, EN_US};

#[test]
fn every_code_is_under_the_crates_name_with_its_message() {
    let (name, text) = EN_US;
    assert_eq!(name, "migrate");
    assert_eq!(
        coded_diagnostic::catalog::check(name, text, &diagnostic_samples()),
        []
    );
}

#[test]
fn every_variant_has_its_own_code() {
    let mut codes: Vec<String> = diagnostic_samples()
        .iter()
        .map(|d| d.code().to_string())
        .collect();
    let all = codes.len();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), all);
    assert_eq!(all, 20);
}

#[test]
fn a_callers_error_keeps_its_own_code_and_args() {
    let own = coded_diagnostic::Report::new("my-app-odd").arg("n", 3_u32);
    let wrapped = MigrateError::caller(&own);
    assert_eq!(wrapped.code(), "my-app-odd");
    assert_eq!(wrapped.to_string(), "my-app-odd n=3");
    assert_eq!(MigrateError::from(own), wrapped);
}

#[test]
fn a_failed_step_nests_its_cause() {
    let e = MigrateError::StepFailed {
        from: 1,
        to: 2,
        cause: Box::new(MigrateError::RootNotAMapping),
    };
    assert_eq!(
        e.to_string(),
        "migrate-step-failed from=1 to=2 cause=migrate-root-not-a-mapping"
    );
}

#[test]
fn a_registry_error_says_which_map_it_is_in() {
    use super::{Direction, RegistryError};
    let says = |direction| RegistryError::Duplicate { direction, from: 1 }.to_string();
    assert_eq!(
        says(Direction::Forward),
        "migrate-registry-duplicate direction=forward from=1"
    );
    assert_eq!(
        says(Direction::Back),
        "migrate-registry-duplicate direction=back from=1"
    );
}
