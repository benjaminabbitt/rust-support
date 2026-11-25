//! A span of time written as text, and a cutoff written as a span or a date,
//! over the `jiff` crate's friendly duration format, so every project of the
//! family reads and writes spans one way. Pure: it reads no clock, no
//! environment, no file, no time zone database; a caller hands it the instant
//! "now" through a [`Now`] (a closure over its own clock, or a fixed
//! instant), in std's own types ([`Duration`], [`SystemTime`]) or jiff's
//! [`SignedDuration`] where a span may be negative.
//!
//! # The grammar
//!
//! jiff's friendly format, and the word `now`:
//!
//! - A span is a number and a unit, or several, largest to smallest, each
//!   unit once, with or without spaces and commas between: `5m`, `1h30m`,
//!   `1h 30m`, `1h, 30m`, `2 days`, `500ms`, and `01:30:00`.
//! - Units, lowercase: `w`/`wk`/`week(s)`, `d`/`day(s)`, `h`/`hr`/`hour(s)`,
//!   `m`/`min`/`minute(s)`, `s`/`sec`/`second(s)`, `ms`, `us`/`µs` (the micro
//!   sign U+00B5; not the Greek mu U+03BC) and `ns`. A day is 24 hours and a
//!   week 7 days. A year or a month has no fixed length: it is
//!   [`DurationError::CalendarUnit`].
//! - A fraction (`.` or `,`) is read on the last unit when that is a week's
//!   or a day's smaller: `1.5h`, `1.5m`, `4.2s`, `1.5ms`; never on days,
//!   weeks or nanoseconds (`1.5d`), and only after digits (`.5h` is refused).
//! - A number always has a unit: `30` and `0` are refused.
//! - A sign (`-3d`, `+3d`) or a trailing `ago` makes a span negative:
//!   [`parse_signed`] reads one and [`parse`] refuses it
//!   ([`DurationError::Negative`]).
//! - Whitespace around the whole text is ignored.
//! - The word `now`, lowercase, is a span of no time (zero), as `0s` is: "at
//!   once" where a caller reads a span of waiting, "this instant" as a cutoff.
//!
//! The longest span is `i64::MAX` seconds. A span written with days or
//! weeks is held to jiff's span range of about 20,000 years.
//!
//! # Texts of a span
//!
//! [`format_exact`] (and [`format_signed`]) is jiff's compact text of the
//! span, `1h 30m` or `1s 500ms`, which [`parse`] reads back to the same
//! span. [`format_age`] rounds down to the largest whole unit among `d`,
//! `h`, `m` and `s` for a display: `12s`, `3m`, `5h`, `2d`; [`parse`] reads
//! it back to the rounded value.
//!
//! # A cutoff
//!
//! [`DurationText::cutoff`] reads "older than this span, or before this date":
//! a span is that long before now, and a date `YYYY-MM-DD` is that day's
//! midnight, **UTC** (a date-only literal is never local time). Any other
//! date form (a single-digit month, a time of day, an offset) is refused.
//!
//! # Now
//!
//! [`DurationText::new`] takes a [`Now`]: a `Fn() -> SystemTime` closure, or a
//! `SystemTime` for a fixed instant. Each method reads it at most once.
//! [`DurationText::frozen`] reads it once and returns an instance whose "now"
//! is that instant, for a query that judges many items against one instant.

mod date;
mod error;

use std::time::{Duration, SystemTime};

use jiff::fmt::friendly::{Designator, Direction, SpanParser, SpanPrinter};
pub use jiff::SignedDuration;
use jiff::{Span, SpanRelativeTo};

pub use error::{DurationError, EN_US};

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;

/// The word that is a span of no time.
pub const NOW: &str = "now";

/// Where "now" comes from: a closure over the caller's clock, or a fixed
/// instant. This crate never reads the system clock itself.
pub trait Now {
    fn now(&self) -> SystemTime;
}

impl<F: Fn() -> SystemTime> Now for F {
    fn now(&self) -> SystemTime {
        self()
    }
}

impl Now for SystemTime {
    fn now(&self) -> SystemTime {
        *self
    }
}

/// How an age compares with a span.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Comparison {
    /// `>=`
    AtLeast,
    /// `>`
    MoreThan,
    /// `<`
    LessThan,
    /// `<=`
    AtMost,
}

impl Comparison {
    /// Whether `age` is `self` than `span`, exactly.
    pub fn holds(self, age: Duration, span: Duration) -> bool {
        match self {
            Self::AtLeast => age >= span,
            Self::MoreThan => age > span,
            Self::LessThan => age < span,
            Self::AtMost => age <= span,
        }
    }
}

/// The grammar bound to a source of "now".
#[derive(Clone, Debug)]
pub struct DurationText<N> {
    now: N,
}

impl<N: Now> DurationText<N> {
    pub fn new(now: N) -> Self {
        Self { now }
    }

    /// The source's "now", read once.
    pub fn now(&self) -> SystemTime {
        self.now.now()
    }

    /// `text` as a span (reads no clock).
    #[allow(clippy::unused_self)]
    pub fn parse(&self, text: &str) -> Result<Duration, DurationError> {
        parse(text)
    }

    /// `text` as a cutoff: that span before now, or the date's UTC midnight.
    pub fn cutoff(&self, text: &str) -> Result<SystemTime, DurationError> {
        cutoff_at(text, self.now.now())
    }

    /// How long since `since`, at now; zero for a `since` after now.
    pub fn age(&self, since: SystemTime) -> Duration {
        age_between(since, self.now.now())
    }

    /// Whether the age of `since` is `op` `span`, by [`Comparison::holds`].
    pub fn age_is(&self, since: SystemTime, op: Comparison, span: Duration) -> bool {
        op.holds(self.age(since), span)
    }

    /// An instance whose "now" is the one this reads, once, now.
    pub fn frozen(&self) -> DurationText<SystemTime> {
        DurationText::new(self.now.now())
    }
}

/// How long from `since` to `now`: zero when `since` is after `now`.
pub fn age_between(since: SystemTime, now: SystemTime) -> Duration {
    now.duration_since(since).unwrap_or(Duration::ZERO)
}

/// `text` as a cutoff, read at `now`: [`DurationText::cutoff`] for a caller
/// that has the instant in hand.
pub fn cutoff_at(text: &str, now: SystemTime) -> Result<SystemTime, DurationError> {
    let text = text.trim();
    let out_of_range = || DurationError::OutOfRange {
        text: text.to_string(),
    };
    match date::read(text) {
        date::Dated::At(seconds) => date::at(seconds).ok_or_else(out_of_range),
        date::Dated::Invalid => Err(DurationError::InvalidDate {
            text: text.to_string(),
        }),
        date::Dated::NotADate => now.checked_sub(parse(text)?).ok_or_else(out_of_range),
    }
}

static PARSER: SpanParser = SpanParser::new();
static PRINTER: SpanPrinter = SpanPrinter::new()
    .designator(Designator::Compact)
    .direction(Direction::Sign);

/// The exact text of a span, in compact designators (`1h 30m`, `1s 500ms`,
/// `0s`): [`parse`] reads it back to the same span, up to
/// [`SignedDuration::MAX`] seconds.
pub fn format_exact(span: Duration) -> String {
    PRINTER.unsigned_duration_to_string(&span)
}

/// A signed span's exact text (`-1h 30m`), which [`parse_signed`] reads back.
pub fn format_signed(span: SignedDuration) -> String {
    PRINTER.duration_to_string(&span)
}

/// A span rounded down to its largest whole unit of `d` (24 hours), `h`,
/// `m` or `s` and written in compact designators: `12s`, `3m`, `5h`, `2d`;
/// `0s` under a second. [`parse`] reads it back to the rounded value. Days
/// are not counted past [`MAX_AGE_DAYS`], a span of about 20,000 years.
pub fn format_age(span: Duration) -> String {
    let seconds = span.as_secs();
    let rounded = if seconds >= SECONDS_PER_DAY {
        Span::new().days(
            i64::try_from(seconds / SECONDS_PER_DAY).map_or(MAX_AGE_DAYS, |d| d.min(MAX_AGE_DAYS)),
        )
    } else if seconds >= 3_600 {
        Span::new().hours(seconds as i64 / 3_600)
    } else if seconds >= 60 {
        Span::new().minutes(seconds as i64 / 60)
    } else {
        Span::new().seconds(seconds as i64)
    };
    PRINTER.span_to_string(&rounded)
}

/// The most days [`format_age`] counts: the largest day count of a jiff span.
pub const MAX_AGE_DAYS: i64 = 7_304_484;

const SECONDS_PER_DAY: u64 = 86_400;

/// `text` as a span; [`NOW`] is zero. A negative span is
/// [`DurationError::Negative`]; [`parse_signed`] reads one.
pub fn parse(text: &str) -> Result<Duration, DurationError> {
    let signed = parse_signed(text)?;
    Duration::try_from(signed).map_err(|_| DurationError::Negative {
        text: text.trim().to_string(),
    })
}

/// `text` as a span that may be negative (`-1h30m`, `+2d`, `1h ago`);
/// [`NOW`] is zero. The span is exact: nanoseconds are
/// [`SignedDuration::as_nanos`], a day is 24 hours and a week 7 days, and a
/// year or a month, which have no fixed length, is
/// [`DurationError::CalendarUnit`].
pub fn parse_signed(text: &str) -> Result<SignedDuration, DurationError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    if text == NOW {
        return Ok(SignedDuration::ZERO);
    }
    if let Ok(duration) = PARSER.parse_duration(text) {
        return Ok(duration);
    }
    let span = PARSER
        .parse_span(text)
        .map_err(|cause| DurationError::malformed(text, &cause))?;
    if span.get_years() != 0 || span.get_months() != 0 {
        return Err(DurationError::CalendarUnit {
            text: text.to_string(),
        });
    }
    span.to_duration(SpanRelativeTo::days_are_24_hours())
        .map_err(|cause| DurationError::malformed(text, &cause))
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
