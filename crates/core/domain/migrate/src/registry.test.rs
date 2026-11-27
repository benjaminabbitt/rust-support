use super::{Registry, RoundTrip};
use crate::error::{Direction, RegistryError};
use crate::MigrateError;

/// Four versions of one object. Each step marks the trail with the version
/// it left (forward, `n`) or the version it left going back (`-n`), so what
/// ran, and in what order, is in the object.
macro_rules! versions {
    ($($v:ident),*) => {
        $(#[derive(Clone, PartialEq, Debug, Default)]
        struct $v { trail: Vec<i64> })*
    };
}
versions!(V0, V1, V2, V3);

macro_rules! step {
    ($from:ident => $to:ident, $n:expr) => {
        impl From<$from> for $to {
            fn from(old: $from) -> $to {
                let mut trail = old.trail;
                trail.push($n);
                $to { trail }
            }
        }
    };
}
step!(V0 => V1, 0);
step!(V1 => V2, 1);
step!(V2 => V3, 2);
step!(V3 => V2, -3);
step!(V2 => V1, -2);
step!(V1 => V0, -1);
step!(V2 => V0, -20);

/// A step that refuses, with the caller's own error.
#[derive(Clone, PartialEq, Debug)]
struct Odd;
impl TryFrom<V1> for Odd {
    type Error = MigrateError;
    fn try_from(_: V1) -> Result<Odd, MigrateError> {
        Err(MigrateError::RootNotAMapping)
    }
}

impl From<Odd> for V1 {
    fn from(_: Odd) -> V1 {
        V1::default()
    }
}

/// Versions 0 to 3: three forward steps and the three back steps.
fn registry() -> Registry<V3> {
    Registry::builder(0, 3)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .forward::<V2, V3>(2)
        .back::<V1, V0>(1)
        .back::<V2, V1>(2)
        .back::<V3, V2>(3)
        .build()
        .unwrap()
}

fn trail(t: &[i64]) -> V0 {
    V0 { trail: t.to_vec() }
}

// ------------------------------------------------------------ construction

#[test]
fn a_whole_chain_builds() {
    let r = registry();
    assert_eq!((r.oldest(), r.current()), (0, 3));
    assert!(r.has_back(1) && r.has_back(2) && r.has_back(3) && !r.has_back(0));
    assert!(r.type_name(0).unwrap().ends_with("V0"));
    assert!(r.type_name(3).unwrap().ends_with("V3"));
    assert_eq!(r.type_name(4), None);
}

#[test]
fn a_registry_at_one_version_has_no_steps() {
    let r = Registry::<V3>::builder(2, 2).build().unwrap();
    let m = r.migrate_from(V3::default(), 2).unwrap();
    assert_eq!(m.applied, []);
}

#[test]
fn a_gap_in_forward_is_refused() {
    let built = Registry::<V3>::builder(0, 3)
        .forward::<V0, V1>(0)
        .forward::<V2, V3>(2)
        .build();
    assert_eq!(built.unwrap_err(), RegistryError::Gap { from: 1 });
}

#[test]
fn a_current_version_with_no_path_to_it_is_refused() {
    let built = Registry::<V3>::builder(0, 3)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .build();
    assert_eq!(built.unwrap_err(), RegistryError::Gap { from: 2 });
    assert_eq!(
        Registry::<V1>::builder(0, 1).build().unwrap_err(),
        RegistryError::Gap { from: 0 }
    );
}

#[test]
fn a_forward_step_outside_the_versions_is_refused() {
    let at_current = Registry::<V1>::builder(0, 1)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .build();
    assert_eq!(
        at_current.unwrap_err(),
        RegistryError::OutOfRange {
            direction: Direction::Forward,
            from: 1,
            oldest: 0,
            current: 1
        }
    );
    let below = Registry::<V2>::builder(1, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .build();
    assert!(matches!(
        below.unwrap_err(),
        RegistryError::OutOfRange { from: 0, .. }
    ));
}

#[test]
fn two_steps_from_one_version_are_refused() {
    let forward = Registry::<V2>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .build();
    assert_eq!(
        forward.unwrap_err(),
        RegistryError::Duplicate {
            direction: Direction::Forward,
            from: 0
        }
    );
    let back = Registry::<V2>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .back::<V1, V0>(1)
        .back::<V1, V0>(1)
        .build();
    assert_eq!(
        back.unwrap_err(),
        RegistryError::Duplicate {
            direction: Direction::Back,
            from: 1
        }
    );
}

#[test]
fn a_back_step_whose_forward_step_is_missing_is_refused() {
    // back[3] undoes forward[2], which is not there to be undone.
    let past_current = Registry::<V2>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .back::<V3, V2>(3)
        .build();
    assert_eq!(
        past_current.unwrap_err(),
        RegistryError::BackWithoutForward { from: 3 }
    );
    // back[0] would undo forward[-1].
    let at_oldest = Registry::<V1>::builder(0, 1)
        .forward::<V0, V1>(0)
        .back::<V1, V0>(0)
        .build();
    assert_eq!(
        at_oldest.unwrap_err(),
        RegistryError::BackWithoutForward { from: 0 }
    );
}

#[test]
fn an_oldest_above_the_current_is_refused() {
    assert_eq!(
        Registry::<V1>::builder(3, 1).build().unwrap_err(),
        RegistryError::Inverted {
            oldest: 3,
            current: 1
        }
    );
}

#[test]
fn steps_whose_types_do_not_line_up_are_refused() {
    // Version 1's step takes a V2, but version 0's step makes a V1.
    let apart = Registry::<V3>::builder(0, 3)
        .forward::<V0, V1>(0)
        .forward::<V2, V3>(1)
        .forward::<V2, V3>(2)
        .build();
    assert_eq!(apart.unwrap_err(), RegistryError::TypeMismatch { from: 0 });
}

#[test]
fn a_chain_that_does_not_end_in_the_registrys_type_is_refused() {
    let wrong = Registry::<V3>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .build();
    assert_eq!(wrong.unwrap_err(), RegistryError::NotCurrent { from: 1 });
}

#[test]
fn a_back_step_between_the_wrong_types_is_refused() {
    let wrong = Registry::<V2>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .back::<V2, V0>(2)
        .build();
    assert_eq!(
        wrong.unwrap_err(),
        RegistryError::BackTypeMismatch { from: 2 }
    );
}

// -------------------------------------------------------------- migration

#[test]
fn an_old_object_migrates_through_every_step_in_order() {
    let m = registry().migrate_from(trail(&[]), 0).unwrap();
    assert_eq!(m.applied, [0, 1, 2]);
    assert_eq!((m.from, m.to), (0, 3));
    assert!(m.changed());
    assert_eq!(
        m.object,
        V3 {
            trail: vec![0, 1, 2]
        }
    );
}

#[test]
fn an_object_starts_at_its_own_version() {
    let m = registry().migrate_from(V2::default(), 2).unwrap();
    assert_eq!(m.applied, [2]);
    assert_eq!(m.object, V3 { trail: vec![2] });
}

#[test]
fn a_current_object_is_untouched() {
    let current = V3 { trail: vec![9] };
    let m = registry().migrate_from(current.clone(), 3).unwrap();
    assert!(!m.changed());
    assert_eq!(m.object, current);
}

#[test]
fn a_newer_version_is_refused() {
    let e = registry().migrate_from(V3::default(), 4).unwrap_err();
    assert_eq!(
        e,
        MigrateError::TooNew {
            found: 4,
            current: 3
        }
    );
}

#[test]
fn a_version_below_the_oldest_is_refused() {
    let r = Registry::<V2>::builder(1, 2)
        .forward::<V1, V2>(1)
        .build()
        .unwrap();
    let e = r.migrate_from(V0::default(), 0).unwrap_err();
    assert_eq!(
        e,
        MigrateError::TooOld {
            found: 0,
            oldest: 1
        }
    );
    assert_eq!(r.readable(1), Ok(()));
    assert_eq!(r.readable(2), Ok(()));
    assert!(r.readable(3).is_err());
}

#[test]
fn an_object_of_the_wrong_type_for_its_version_is_told() {
    let e = registry().migrate_from(V2::default(), 0).unwrap_err();
    let MigrateError::StepFailed {
        from: 0,
        to: 1,
        cause,
    } = e
    else {
        panic!("{e:?}")
    };
    assert!(matches!(*cause, MigrateError::WrongType { ref expected } if expected.ends_with("V0")));
    // An object that is not even the registry's own type at the end.
    let r = Registry::<V3>::builder(2, 2).build().unwrap();
    assert!(matches!(
        r.migrate_from(V0::default(), 2),
        Err(MigrateError::WrongType { .. })
    ));
}

#[test]
fn a_failing_step_names_its_versions_and_its_cause() {
    let r = Registry::<Odd>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, Odd>(1)
        .build()
        .unwrap();
    let e = r.migrate_from(V0::default(), 0).unwrap_err();
    assert_eq!(
        e,
        MigrateError::StepFailed {
            from: 1,
            to: 2,
            cause: Box::new(MigrateError::RootNotAMapping)
        }
    );
}

#[test]
fn migrate_takes_an_erased_object_as_a_form_loads_it() {
    let m = registry().migrate(Box::new(trail(&[7])), 0).unwrap();
    assert_eq!(m.object.trail, [7, 0, 1, 2]);
}

// -------------------------------------------------------------------- back

#[test]
fn an_object_goes_back_through_the_newest_step_first() {
    let m = registry().downgrade::<V3, V1>(V3::default(), 3, 1).unwrap();
    assert_eq!(m.applied, [3, 2]);
    assert_eq!((m.from, m.to), (3, 1));
    assert_eq!(
        m.object,
        V1 {
            trail: vec![-3, -2]
        }
    );
}

#[test]
fn a_downgrade_needs_every_back_step_before_it_runs_any() {
    let build = |back: &[u32]| {
        let mut b = Registry::<V3>::builder(0, 3)
            .forward::<V0, V1>(0)
            .forward::<V1, V2>(1)
            .forward::<V2, V3>(2);
        for v in back {
            b = match v {
                1 => b.back::<V1, V0>(1),
                2 => b.back::<V2, V1>(2),
                _ => b.back::<V3, V2>(3),
            };
        }
        b.build().unwrap()
    };
    assert_eq!(
        build(&[1, 3])
            .downgrade::<V3, V0>(V3::default(), 3, 0)
            .unwrap_err(),
        MigrateError::NoBack { from: 2 }
    );
    assert_eq!(
        build(&[2])
            .downgrade::<V3, V1>(V3::default(), 3, 1)
            .unwrap_err(),
        MigrateError::NoBack { from: 3 }
    );
}

#[test]
fn a_downgrade_to_a_higher_version_or_outside_the_range_is_refused() {
    let r = registry();
    assert_eq!(
        r.downgrade::<V1, V2>(V1::default(), 1, 2).unwrap_err(),
        MigrateError::NotADowngrade { from: 1, to: 2 }
    );
    assert_eq!(
        r.downgrade::<V3, V1>(V3::default(), 5, 1).unwrap_err(),
        MigrateError::TooNew {
            found: 5,
            current: 3
        }
    );
}

#[test]
fn a_downgrade_to_the_same_version_is_nothing() {
    let m = registry().downgrade::<V2, V2>(V2::default(), 2, 2).unwrap();
    assert_eq!((m.object, m.applied), (V2::default(), vec![]));
}

#[test]
fn a_downgrade_whose_result_is_not_the_type_asked_for_is_told() {
    let e = registry()
        .downgrade::<V3, V0>(V3::default(), 3, 1)
        .unwrap_err();
    assert!(matches!(e, MigrateError::WrongType { .. }));
}

/// A pair whose back step can refuse.
#[derive(Clone, PartialEq, Debug)]
struct P0;
#[derive(Debug)]
struct P1;
impl From<P0> for P1 {
    fn from(_: P0) -> P1 {
        P1
    }
}
impl TryFrom<P1> for P0 {
    type Error = MigrateError;
    fn try_from(_: P1) -> Result<P0, MigrateError> {
        Err(MigrateError::RootNotAMapping)
    }
}

#[test]
fn a_failing_back_step_names_its_versions() {
    let r = Registry::<P1>::builder(0, 1)
        .forward::<P0, P1>(0)
        .back::<P1, P0>(1)
        .build()
        .unwrap();
    assert_eq!(
        r.downgrade::<P1, P0>(P1, 1, 0).unwrap_err(),
        MigrateError::StepFailed {
            from: 1,
            to: 0,
            cause: Box::new(MigrateError::RootNotAMapping)
        }
    );
}

/// Two versions of one fixture: a rename, and its undoing.
#[derive(Clone, PartialEq, Debug)]
struct Named {
    a: i64,
}
#[derive(Clone, PartialEq, Debug)]
struct Renamed {
    b: i64,
}
impl From<Named> for Renamed {
    fn from(o: Named) -> Renamed {
        Renamed { b: o.a }
    }
}
impl From<Renamed> for Named {
    fn from(o: Renamed) -> Named {
        Named { a: o.b }
    }
}
/// A back step that loses what the step added.
#[derive(Clone, PartialEq, Debug)]
struct Wider {
    a: i64,
    extra: i64,
}
impl From<Named> for Wider {
    fn from(o: Named) -> Wider {
        Wider { a: o.a, extra: 5 }
    }
}
impl From<Wider> for Named {
    fn from(o: Wider) -> Named {
        Named { a: o.a + o.extra }
    }
}

#[test]
fn back_of_forward_gives_the_fixture_back() {
    let r = Registry::<Renamed>::builder(0, 1)
        .forward::<Named, Renamed>(0)
        .back::<Renamed, Named>(1)
        .build()
        .unwrap();
    assert_eq!(r.round_trip(Named { a: 5 }, 0), Ok(RoundTrip::Held));
    let up = r.migrate_from(Named { a: 5 }, 0).unwrap().object;
    let down: Named = r.downgrade::<Renamed, Named>(up, 1, 0).unwrap().object;
    assert_eq!(down, Named { a: 5 });
}

#[test]
fn a_back_step_that_does_not_round_trip_is_found() {
    let r = Registry::<Wider>::builder(0, 1)
        .forward::<Named, Wider>(0)
        .back::<Wider, Named>(1)
        .build()
        .unwrap();
    assert_eq!(
        r.round_trip(Named { a: 1 }, 0),
        Err(MigrateError::RoundTrip { version: 0 })
    );
}

#[test]
fn a_round_trip_is_asked_of_a_version_with_a_back_step() {
    let r = Registry::<V3>::builder(0, 3)
        .forward::<V0, V1>(0)
        .forward::<V1, V2>(1)
        .forward::<V2, V3>(2)
        .build()
        .unwrap();
    assert_eq!(r.round_trip(V0::default(), 0), Ok(RoundTrip::NoBack));
    let full = registry();
    assert_eq!(
        full.round_trip(V3::default(), 3),
        Err(MigrateError::TooNew {
            found: 3,
            current: 3
        })
    );
    assert_eq!(
        full.round_trip(V3::default(), 7),
        Err(MigrateError::TooNew {
            found: 7,
            current: 3
        })
    );
}

#[test]
fn a_failing_step_in_a_round_trip_names_its_versions() {
    let r = Registry::<Odd>::builder(0, 2)
        .forward::<V0, V1>(0)
        .forward::<V1, Odd>(1)
        .back::<Odd, V1>(2)
        .build()
        .unwrap();
    assert_eq!(
        r.round_trip(V1::default(), 1).unwrap_err(),
        MigrateError::StepFailed {
            from: 1,
            to: 2,
            cause: Box::new(MigrateError::RootNotAMapping)
        }
    );
    let r = Registry::<P1>::builder(0, 1)
        .forward::<P0, P1>(0)
        .back::<P1, P0>(1)
        .build()
        .unwrap();
    assert_eq!(
        r.round_trip(P0, 0).unwrap_err(),
        MigrateError::StepFailed {
            from: 1,
            to: 0,
            cause: Box::new(MigrateError::RootNotAMapping)
        }
    );
}

#[test]
fn the_oldest_version_is_the_one_asked_for() {
    let r = Registry::<V3>::builder(2, 3)
        .forward::<V2, V3>(2)
        .build()
        .unwrap();
    assert_eq!((r.oldest(), r.current()), (2, 3));
}
