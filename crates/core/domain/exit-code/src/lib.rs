//! How a command ends: its class and its code, by the exit-code convention
//! (`docs/exit-codes.md` at this repository's root, rendered as
//! [`convention`]). Pure: a core may say its errors' classes with it, and a
//! driving edge turns the class into the process's status.
//!
//! The codes come in blocks, each 16 wide and 16-aligned (the class is
//! `status >> 4`): 0 ok and 16-31 successes, 32-47 failures, 48-63 declines,
//! 128+n a signal; 1-15 are left to generic errors (1, and 2 for a usage
//! error), 64-78 to sysexits.h, 101 to a Rust panic, 126-127 to the shell and
//! 255 to an out-of-range status. Only the success block collapses: by long
//! Unix convention a non-zero status is an error (`set -e`, `&&`, make and CI
//! all read it so), so a success exits 0 unless the caller opted in with
//! `--detailed-exitcode`, and only then are the distinguishing successes
//! 16-31 returned. A failure or a decline is already non-zero and reads correctly
//! as an error, so it is always returned as it is.

use std::process::ExitCode;

/// A signal's code is this plus its number (the shell's convention).
const SIGNAL_BASE: u8 = 128;

/// The class of an [`Exit`]: which block its code is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    /// 0 and 16-31: the command did what was asked.
    Success,
    /// 32-47: the command tried, and something broke.
    Failed,
    /// 48-63: the command did not try: the request or the state forbids it.
    Declined,
    /// 128+n: the command was stopped by signal n.
    Signalled,
}

/// How a command ends. Built only by the constructors and constants, so its
/// code is always in its block. Its order is the severity order (the order
/// of the detailed codes: a signal over a decline over a failure over a
/// success, and within a block the higher code): see [`Exit::worst`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Exit(u8);

impl Exit {
    /// 0: the command succeeded.
    pub const OK: Exit = Exit(0);
    /// 16: a dry run or preview succeeded, and applying it would change
    /// something (returned only under `--detailed-exitcode`).
    pub const WOULD_CHANGE: Exit = Exit(16);
    /// 32: the command tried and something broke.
    pub const FAILED: Exit = Exit(32);
    /// 48: the request or the state forbids it (a usage error too).
    pub const REFUSED: Exit = Exit(48);
    /// 49: the input was examined and cannot produce a working result.
    pub const FATAL_FINDINGS: Exit = Exit(49);

    /// The success `code`, 0 or 16 to 31. Panics on any other.
    pub const fn success(code: u8) -> Exit {
        assert!(
            code == 0 || (16 <= code && code <= 31),
            "a success is the code 0 or a code from 16 to 31"
        );
        Exit(code)
    }

    /// The failure `code`, 32 to 47. Panics on any other.
    pub const fn failed(code: u8) -> Exit {
        assert!(
            32 <= code && code <= 47,
            "a failure is a code from 32 to 47"
        );
        Exit(code)
    }

    /// The decline `code`, 48 to 63. Panics on any other.
    pub const fn declined(code: u8) -> Exit {
        assert!(
            48 <= code && code <= 63,
            "a decline is a code from 48 to 63"
        );
        Exit(code)
    }

    /// Stopped by signal `number`, 1 to 127: the code 128 + `number`.
    /// Panics on any other.
    pub const fn signal(number: u8) -> Exit {
        assert!(
            1 <= number && number <= 127,
            "a signal is numbered from 1 to 127"
        );
        Exit(SIGNAL_BASE + number)
    }

    /// The exit of an argument parser's verdict: a usage error (`is_error`)
    /// is refused (48, never the parser's own status); a request for help or
    /// the version is ok.
    pub const fn usage(is_error: bool) -> Exit {
        if is_error {
            Exit::REFUSED
        } else {
            Exit::OK
        }
    }

    /// The block this exit is in.
    pub const fn class(self) -> Class {
        match self.0 {
            0 | 16..=31 => Class::Success,
            32..=47 => Class::Failed,
            48..=63 => Class::Declined,
            _ => Class::Signalled,
        }
    }

    /// The signal that stopped the command, if a signal did.
    pub const fn signal_number(self) -> Option<u8> {
        match self.class() {
            Class::Signalled => Some(self.0 - SIGNAL_BASE),
            _ => None,
        }
    }

    /// The process's exit status. This is the one place the collapse rule
    /// lives: without `detailed` (the `--detailed-exitcode` flag) a success
    /// is 0 whatever its code; every other exit is its code.
    pub const fn code(self, detailed: bool) -> u8 {
        match self.class() {
            Class::Success if !detailed => 0,
            _ => self.0,
        }
    }

    /// [`code`](Exit::code) as the process's [`ExitCode`].
    pub fn process(self, detailed: bool) -> ExitCode {
        ExitCode::from(self.code(detailed))
    }

    /// The exit of a child program that ended with `code`, or was ended by
    /// `signal`: a child's own codes are not this convention's, so none is
    /// passed through (its 16 would read as a success subcode). Ok for 0; the
    /// signal for a signal in 1..=127, or a shell's 128 plus one; failed (32)
    /// for any other status, or none. For a child that follows this
    /// convention itself, see [`from_convention_child_status`](Exit::from_convention_child_status).
    pub const fn from_child_status(code: Option<i32>, signal: Option<i32>) -> Exit {
        match (code, signal) {
            (_, Some(n @ 1..=127)) => Exit::signal(n as u8),
            (Some(0), None) => Exit::OK,
            (Some(status @ 129..=255), None) => Exit::signal(status as u8 - SIGNAL_BASE),
            _ => Exit::FAILED,
        }
    }

    /// The exit of a child program that follows this convention, read by its
    /// blocks: 0 is ok, 16-31 the success subcode, 32-47 the failure, 48-63
    /// the decline; a signal (or a shell's 128 plus one) is the signal, as in
    /// [`from_child_status`](Exit::from_child_status); any other status, or
    /// none, is failed (32). Use it only for a child known to follow the
    /// convention; otherwise a child's own 16 to 63 would be read as ours.
    pub const fn from_convention_child_status(code: Option<i32>, signal: Option<i32>) -> Exit {
        match (code, signal) {
            (None, _) | (_, Some(_)) | (Some(129..=255), _) | (Some(0), _) => {
                Exit::from_child_status(code, signal)
            }
            (Some(status @ (16..=31)), None) => Exit::success(status as u8),
            (Some(status @ (32..=47)), None) => Exit::failed(status as u8),
            (Some(status @ (48..=63)), None) => Exit::declined(status as u8),
            _ => Exit::FAILED,
        }
    }

    /// The more severe of `self` and `other`, the one a command that has
    /// both to report exits with. A signal outranks everything; a decline
    /// outranks a failure (it is a verdict on the request, which a retry
    /// unchanged would meet again, where a failure may be transient); any
    /// error outranks a success; within a block the higher code wins.
    pub fn worst(self, other: Exit) -> Exit {
        self.max(other)
    }
}

/// An error that says its own class: the exit a command it ends exits with.
pub trait ExitClass {
    /// The exit this error ends a command with.
    fn exit(&self) -> Exit;
}

/// The exit-code convention [`Exit`] encodes: `docs/exit-codes.md` at this
/// repository's root, a document any command-line program can copy.
#[doc = include_str!("../../../../../docs/exit-codes.md")]
pub mod convention {}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
