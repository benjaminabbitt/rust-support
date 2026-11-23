//! The exit pattern: the blocks and their bounds, the collapse of the
//! distinguishing successes without `--detailed-exitcode`, the severity
//! order across the blocks, a child's status said in the classes (by default
//! and by the blocks), and the usage error's class.

use std::panic::catch_unwind;

use super::*;

/// The codes of the success block: 0 (ok), and 16 to 31 (16 is detailed ok).
const SUCCESSES: [u8; 17] = [
    0, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
];

#[test]
fn test_the_defined_codes_are_zero_sixteen_seventeen_thirty_two_forty_eight_and_forty_nine() {
    assert_eq!(Exit::OK, Exit::success(0));
    assert_eq!(Exit::DETAILED_OK, Exit::success(16));
    let detailed = [
        Exit::OK,
        Exit::DETAILED_OK,
        Exit::WOULD_CHANGE,
        Exit::FAILED,
        Exit::REFUSED,
        Exit::FATAL_FINDINGS,
    ]
    .map(|exit| exit.code(true));
    assert_eq!(detailed, [16, 16, 17, 32, 48, 49]);
}

// The generator: the flag is bound once, at the edge, and every exit is
// turned into its status through it; nothing else carries the flag.
#[test]
fn test_the_statuses_without_the_flag_collapse_every_success_to_zero() {
    let statuses = ExitStatuses::new(false);
    assert!(!statuses.detailed());
    for code in SUCCESSES {
        assert_eq!(statuses.code(Exit::success(code)), 0, "success {code}");
    }
    assert_eq!(statuses.code(Exit::FAILED), 32);
    assert_eq!(statuses.code(Exit::REFUSED), 48);
    assert_eq!(statuses.code(Exit::signal(15)), 143);
}

#[test]
fn test_the_statuses_with_the_flag_put_every_success_in_sixteen_to_thirty_one() {
    let statuses = ExitStatuses::new(true);
    assert!(statuses.detailed());
    assert_eq!(statuses.code(Exit::OK), 16);
    assert_eq!(statuses.code(Exit::DETAILED_OK), 16);
    assert_eq!(statuses.code(Exit::WOULD_CHANGE), 17);
    for code in SUCCESSES {
        assert_eq!(statuses.code(Exit::success(code)) >> 4, 1, "success {code}");
    }
    assert_eq!(statuses.code(Exit::FAILED), 32);
}

#[test]
fn test_the_statuses_agree_with_the_exit_for_every_exit_and_flag() {
    let exits = SUCCESSES
        .map(Exit::success)
        .into_iter()
        .chain((32..=47).map(Exit::failed))
        .chain((48..=63).map(Exit::declined))
        .chain((1..=127).map(Exit::signal));
    for exit in exits {
        for detailed in [false, true] {
            let statuses = ExitStatuses::new(detailed);
            assert_eq!(
                statuses.code(exit),
                exit.code(detailed),
                "{exit:?} {detailed}"
            );
            assert_eq!(
                statuses.process(exit),
                exit.process(detailed),
                "{exit:?} {detailed}"
            );
        }
    }
}

#[test]
fn test_ok_is_zero_without_the_flag_and_detailed_ok_with_it() {
    assert_eq!(Exit::OK.code(false), 0);
    assert_eq!(Exit::OK.code(true), Exit::DETAILED_OK.code(true));
    assert_eq!(Exit::DETAILED_OK.code(false), 0);
    assert_eq!(Exit::DETAILED_OK.code(true), 16);
}

// The one-comparison success test a caller of `--detailed-exitcode` writes:
// `status >> 4 == 1`. It holds for every success, ok included, and for
// nothing else the program can return.
#[test]
fn test_with_the_flag_every_success_and_only_a_success_has_high_nibble_one() {
    for code in SUCCESSES {
        assert_eq!(Exit::success(code).code(true) >> 4, 1, "success {code}");
    }
    let others = (32..=47)
        .map(Exit::failed)
        .chain((48..=63).map(Exit::declined))
        .chain((1..=127).map(Exit::signal));
    for exit in others {
        assert_ne!(exit.code(true) >> 4, 1, "{exit:?}");
    }
}

#[test]
fn test_broken_is_the_failure_forty_seven_with_or_without_the_flag() {
    assert_eq!(Exit::BROKEN, Exit::failed(47));
    assert_eq!(Exit::BROKEN.class(), Class::Failed);
    assert_eq!(Exit::BROKEN.code(false), 47);
    assert_eq!(Exit::BROKEN.code(true), 47);
}

// Broken (an operator must act) outranks every other failure, transient
// ones included, and is outranked by every decline.
#[test]
fn test_broken_outranks_every_other_failure_and_no_decline() {
    for code in 32..=46 {
        let other = Exit::failed(code);
        assert_eq!(other.worst(Exit::BROKEN), Exit::BROKEN, "{code}");
        assert_eq!(Exit::BROKEN.worst(other), Exit::BROKEN, "{code}");
    }
    assert_eq!(Exit::BROKEN.worst(Exit::REFUSED), Exit::REFUSED);
}

#[test]
fn test_each_defined_code_is_of_its_class() {
    assert_eq!(Exit::OK.class(), Class::Success);
    assert_eq!(Exit::DETAILED_OK.class(), Class::Success);
    assert_eq!(Exit::WOULD_CHANGE.class(), Class::Success);
    assert_eq!(Exit::FAILED.class(), Class::Failed);
    assert_eq!(Exit::REFUSED.class(), Class::Declined);
    assert_eq!(Exit::FATAL_FINDINGS.class(), Class::Declined);
}

#[test]
fn test_a_distinguishing_success_collapses_to_zero_without_the_flag() {
    for code in SUCCESSES {
        let exit = Exit::success(code);
        assert_eq!(exit.code(false), 0, "success {code} without the flag");
        assert_eq!(
            exit.code(true),
            code.max(16),
            "success {code} with the flag"
        );
        assert_eq!(exit.class(), Class::Success);
    }
}

#[test]
fn test_failures_declines_and_signals_never_collapse() {
    for code in 32..=47 {
        let exit = Exit::failed(code);
        assert_eq!((exit.code(false), exit.code(true)), (code, code));
        assert_eq!(exit.class(), Class::Failed);
    }
    for code in 48..=63 {
        let exit = Exit::declined(code);
        assert_eq!((exit.code(false), exit.code(true)), (code, code));
        assert_eq!(exit.class(), Class::Declined);
    }
    for n in 1..=127 {
        let exit = Exit::signal(n);
        assert_eq!((exit.code(false), exit.code(true)), (128 + n, 128 + n));
        assert_eq!(exit.class(), Class::Signalled);
        assert_eq!(exit.signal_number(), Some(n));
    }
}

#[test]
fn test_only_a_signal_has_a_signal_number() {
    for exit in [Exit::OK, Exit::WOULD_CHANGE, Exit::FAILED, Exit::REFUSED] {
        assert_eq!(exit.signal_number(), None, "{exit:?}");
    }
    assert_eq!(Exit::signal(15).signal_number(), Some(15));
}

#[test]
fn test_a_success_is_zero_or_a_code_from_sixteen_to_thirty_one() {
    assert!(catch_unwind(|| Exit::success(0)).is_ok());
    assert!(catch_unwind(|| Exit::success(16)).is_ok());
    assert!(catch_unwind(|| Exit::success(31)).is_ok());
    for code in [1, 2, 15] {
        assert!(
            catch_unwind(|| Exit::success(code)).is_err(),
            "{code} is left to generic errors"
        );
    }
    assert!(
        catch_unwind(|| Exit::success(32)).is_err(),
        "32 is a failure"
    );
}

#[test]
fn test_a_failure_is_a_code_from_thirty_two_to_forty_seven() {
    assert!(
        catch_unwind(|| Exit::failed(31)).is_err(),
        "31 is a success"
    );
    assert!(catch_unwind(|| Exit::failed(32)).is_ok());
    assert!(catch_unwind(|| Exit::failed(47)).is_ok());
    assert!(
        catch_unwind(|| Exit::failed(48)).is_err(),
        "48 is a decline"
    );
    assert!(catch_unwind(|| Exit::failed(1)).is_err(), "1 is generic");
}

#[test]
fn test_a_decline_is_a_code_from_forty_eight_to_sixty_three() {
    assert!(
        catch_unwind(|| Exit::declined(47)).is_err(),
        "47 is a failure"
    );
    assert!(catch_unwind(|| Exit::declined(48)).is_ok());
    assert!(catch_unwind(|| Exit::declined(63)).is_ok());
    assert!(
        catch_unwind(|| Exit::declined(64)).is_err(),
        "64 is sysexits.h's"
    );
    assert!(catch_unwind(|| Exit::declined(2)).is_err(), "2 is usage");
}

#[test]
fn test_the_blocks_are_sixteen_wide_and_the_class_is_the_high_nibble() {
    for (code, nibble) in [(0, 1), (16, 1), (31, 1), (32, 2), (47, 2), (48, 3), (63, 3)] {
        let exit = match nibble {
            1 => Exit::success(code),
            2 => Exit::failed(code),
            _ => Exit::declined(code),
        };
        assert_eq!(exit.code(true) >> 4, nibble, "code {code}");
    }
    assert_eq!(Exit::signal(1).code(true) >> 4, 8, "a signal is 128 and up");
}

#[test]
fn test_a_signal_is_numbered_from_one_to_one_hundred_twenty_seven() {
    assert!(
        catch_unwind(|| Exit::signal(0)).is_err(),
        "there is no signal 0"
    );
    assert!(catch_unwind(|| Exit::signal(1)).is_ok());
    assert!(catch_unwind(|| Exit::signal(127)).is_ok());
    assert!(
        catch_unwind(|| Exit::signal(128)).is_err(),
        "128 + 128 is no exit code"
    );
}

#[test]
fn test_a_decline_outranks_a_failure() {
    assert_eq!(Exit::FAILED.worst(Exit::REFUSED), Exit::REFUSED);
    assert_eq!(Exit::REFUSED.worst(Exit::FAILED), Exit::REFUSED);
    assert_eq!(Exit::failed(47).worst(Exit::declined(48)), Exit::REFUSED);
}

#[test]
fn test_fatal_findings_outrank_a_refusal() {
    assert_eq!(
        Exit::REFUSED.worst(Exit::FATAL_FINDINGS),
        Exit::FATAL_FINDINGS
    );
    assert_eq!(
        Exit::FATAL_FINDINGS.worst(Exit::REFUSED),
        Exit::FATAL_FINDINGS
    );
}

#[test]
fn test_a_signal_outranks_everything_and_any_error_outranks_a_success() {
    let signal = Exit::signal(1);
    for other in [
        Exit::OK,
        Exit::success(31),
        Exit::FAILED,
        Exit::declined(63),
    ] {
        assert_eq!(signal.worst(other), signal, "{other:?}");
        assert_eq!(other.worst(signal), signal, "{other:?}");
    }
    assert_eq!(Exit::success(31).worst(Exit::FAILED), Exit::FAILED);
    assert_eq!(Exit::WOULD_CHANGE.worst(Exit::OK), Exit::WOULD_CHANGE);
    assert_eq!(Exit::OK.worst(Exit::WOULD_CHANGE), Exit::WOULD_CHANGE);
    assert_eq!(Exit::signal(2).worst(Exit::signal(15)), Exit::signal(15));
}

#[test]
fn test_every_code_of_a_higher_block_outranks_every_code_of_a_lower_one() {
    let blocks: [Vec<Exit>; 4] = [
        SUCCESSES.map(Exit::success).to_vec(),
        (32..=47).map(Exit::failed).collect(),
        (48..=63).map(Exit::declined).collect(),
        (1..=127).map(Exit::signal).collect(),
    ];
    for pair in blocks.windows(2) {
        for (low, high) in pair[0]
            .iter()
            .flat_map(|l| pair[1].iter().map(move |h| (l, h)))
        {
            assert_eq!(low.worst(*high), *high, "{low:?} vs {high:?}");
            assert!(low < high);
        }
    }
}

#[test]
fn test_the_severity_order_is_the_order_of_the_detailed_codes() {
    let mut exits = [
        Exit::signal(2),
        Exit::FATAL_FINDINGS,
        Exit::OK,
        Exit::FAILED,
        Exit::REFUSED,
        Exit::WOULD_CHANGE,
        Exit::DETAILED_OK,
    ];
    exits.sort();
    assert_eq!(
        exits,
        [
            Exit::OK,
            Exit::DETAILED_OK,
            Exit::WOULD_CHANGE,
            Exit::FAILED,
            Exit::REFUSED,
            Exit::FATAL_FINDINGS,
            Exit::signal(2),
        ]
    );
    assert_eq!(
        exits.map(|exit| exit.code(true)),
        [16, 16, 17, 32, 48, 49, 130]
    );
}

#[test]
fn test_a_usage_error_is_refused_and_help_or_version_is_ok() {
    assert_eq!(Exit::usage(true), Exit::REFUSED);
    assert_eq!(Exit::usage(false), Exit::OK);
}

#[test]
fn test_the_process_exit_code_is_the_code_under_the_flag() {
    use std::process::ExitCode;
    assert_eq!(Exit::OK.process(false), ExitCode::from(0));
    assert_eq!(Exit::OK.process(true), ExitCode::from(16));
    assert_eq!(Exit::WOULD_CHANGE.process(false), ExitCode::from(0));
    assert_eq!(Exit::WOULD_CHANGE.process(true), ExitCode::from(17));
    assert_eq!(Exit::REFUSED.process(false), ExitCode::from(48));
}

/// An error of a crate, which says its own class.
struct Taken;

impl ExitClass for Taken {
    fn exit(&self) -> Exit {
        Exit::REFUSED
    }
}

#[test]
fn test_an_error_says_its_own_class() {
    assert_eq!(Taken.exit(), Exit::REFUSED);
}

// A child program's own codes are not the convention's: its status is said in
// the classes, never passed through (its 1 would read as a success).
#[test]
fn test_a_child_status_is_said_in_the_classes() {
    let of = |code, signal| Exit::from_child_status(code, signal).code(true);
    assert_eq!(of(Some(0), None), 16, "a child that exited 0 is ok");
    for code in [1, 2, 9, 10, 16, 20, 21, 32, 48, 64, 127, 128, 256, -1] {
        assert_eq!(of(Some(code), None), 32, "a child's {code} failed");
    }
    assert_eq!(of(Some(129), None), 129, "a shell's 128+1 is signal 1");
    assert_eq!(of(Some(143), None), 143, "a shell's 128+15 is signal 15");
    assert_eq!(of(Some(255), None), 255, "a shell's 128+127 is signal 127");
    assert_eq!(of(None, Some(15)), 143, "a child the signal ended");
    assert_eq!(of(None, Some(1)), 129);
    assert_eq!(of(None, Some(127)), 255);
    assert_eq!(of(Some(0), Some(9)), 137, "the signal is what ended it");
    for signal in [0, 128, -9] {
        assert_eq!(of(None, Some(signal)), 32, "no such signal: {signal}");
    }
    assert_eq!(of(None, None), 32, "a status that says nothing failed");
}

// A child that follows this convention is read by its blocks, on request.
#[test]
fn test_a_child_that_follows_the_convention_is_read_by_its_blocks() {
    let of = |code, signal| Exit::from_convention_child_status(code, signal);
    assert_eq!(of(Some(0), None), Exit::OK, "its collapsed ok");
    assert_eq!(of(Some(16), None), Exit::DETAILED_OK, "its detailed ok");
    for code in 16..=31 {
        assert_eq!(of(Some(code), None), Exit::success(code as u8), "{code}");
    }
    for code in 32..=47 {
        assert_eq!(of(Some(code), None), Exit::failed(code as u8), "{code}");
    }
    for code in 48..=63 {
        assert_eq!(of(Some(code), None), Exit::declined(code as u8), "{code}");
    }
}

#[test]
fn test_a_convention_child_outside_its_blocks_failed_and_a_signal_is_a_signal() {
    let of = |code, signal| Exit::from_convention_child_status(code, signal);
    for code in [1, 2, 15, 64, 78, 101, 126, 127, 128, 256, -1, -16, -32, -48] {
        assert_eq!(of(Some(code), None), Exit::FAILED, "{code}");
    }
    assert_eq!(of(Some(143), None), Exit::signal(15), "a shell's 128+15");
    assert_eq!(of(Some(255), None), Exit::signal(127));
    assert_eq!(of(None, Some(15)), Exit::signal(15));
    assert_eq!(
        of(Some(48), Some(9)),
        Exit::signal(9),
        "the signal ended it"
    );
    assert_eq!(of(None, Some(0)), Exit::FAILED);
    assert_eq!(of(None, None), Exit::FAILED);
}
