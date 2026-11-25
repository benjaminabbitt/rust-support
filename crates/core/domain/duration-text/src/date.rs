//! A calendar date, `YYYY-MM-DD`, at 00:00 UTC, in std's own types.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil;

/// 1970-01-01T00:00:00, the Unix epoch, as a civil time.
const EPOCH: civil::DateTime = civil::date(1970, 1, 1).at(0, 0, 0, 0);

/// What a text shaped like a date is: not one, a calendar date, or shaped
/// like one and not valid.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Dated {
    /// Not date-shaped (not three `-`-separated runs of digits).
    NotADate,
    /// Date-shaped, and not a calendar date written `YYYY-MM-DD`.
    Invalid,
    /// Seconds from 1970-01-01T00:00:00Z to the date's midnight.
    At(i64),
}

pub(crate) fn read(text: &str) -> Dated {
    let parts: Vec<&str> = text.split('-').collect();
    let digits = |p: &&str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    if parts.len() != 3 || !parts.iter().all(digits) {
        return Dated::NotADate;
    }
    if (parts[0].len(), parts[1].len(), parts[2].len()) != (4, 2, 2) {
        return Dated::Invalid;
    }
    // Four, two and two digits fit i16 and i8: the calendar is jiff's.
    let number = |p: &str| p.parse::<i16>().unwrap_or(0);
    let (year, month, day) = (number(parts[0]), number(parts[1]), number(parts[2]));
    let Ok(day) = i8::try_from(day) else {
        return Dated::Invalid;
    };
    let Ok(month) = i8::try_from(month) else {
        return Dated::Invalid;
    };
    // Civil arithmetic, not a UTC `Timestamp`, whose range ends a day short
    // of 9999-12-31.
    civil::Date::new(year, month, day).map_or(Dated::Invalid, |date| {
        Dated::At(
            date.to_datetime(civil::Time::midnight())
                .duration_since(EPOCH)
                .as_secs(),
        )
    })
}

/// The instant `seconds` from the epoch (before it, when negative).
pub(crate) fn at(seconds: i64) -> Option<SystemTime> {
    match u64::try_from(seconds) {
        Ok(after) => UNIX_EPOCH.checked_add(Duration::from_secs(after)),
        Err(_) => UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs())),
    }
}

#[cfg(test)]
#[path = "date.test.rs"]
mod tests;
