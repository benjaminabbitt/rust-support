//! The grammar is jiff's friendly format, plus `now`; these tests pin what the
//! layer over it decides (trim, `now`, signs, errors as diagnostics, the two
//! formats) and which forms of a consumer's configuration still parse.

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use diagnostic::{Arg, Diagnostic};

use crate::{
    age_between, cutoff_at, format_age, format_exact, format_signed, parse, parse_signed,
    Comparison, DurationError, DurationText, SignedDuration, MAX_AGE_DAYS,
};

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

fn code(text: &str) -> String {
    parse(text)
        .unwrap_err()
        .code()
        .trim_start_matches("duration-text-")
        .to_string()
}

#[test]
fn a_span_is_a_number_and_a_unit() {
    for (text, expected) in [
        ("5m", secs(300)),
        ("90m", secs(5_400)),
        ("45s", secs(45)),
        ("2d", secs(172_800)),
        ("2days", secs(172_800)),
        ("1w", secs(604_800)),
        ("500ms", Duration::from_millis(500)),
        ("2\u{b5}s", Duration::from_micros(2)),
        ("3ns", Duration::from_nanos(3)),
        ("0s", Duration::ZERO),
        ("0d", Duration::ZERO),
        ("1 hour", secs(3_600)),
    ] {
        assert_eq!(parse(text), Ok(expected), "{text}");
    }
}

#[test]
fn a_chain_runs_largest_to_smallest_with_or_without_spaces() {
    assert_eq!(parse("1h30m"), Ok(secs(5_400)));
    assert_eq!(parse("1h 30m"), Ok(secs(5_400)));
    assert_eq!(parse("1h, 30m"), Ok(secs(5_400)));
    assert_eq!(parse("01:30:00"), Ok(secs(5_400)));
    assert_eq!(parse("2w1d"), Ok(secs(1_296_000)));
    for text in ["30m1h", "1h1h", "1d2w"] {
        assert_eq!(code(text), "malformed", "{text}");
    }
}

#[test]
fn a_fraction_is_read_on_the_last_unit_where_jiff_reads_one() {
    assert_eq!(parse("1.5h"), Ok(secs(5_400)));
    assert_eq!(parse("0.5h"), Ok(secs(1_800)));
    assert_eq!(parse("1,5h"), Ok(secs(5_400)));
    assert_eq!(parse("1.5m"), Ok(secs(90)));
    assert_eq!(parse("4.2s"), Ok(Duration::from_millis(4_200)));
    assert_eq!(parse("1.5ms"), Ok(Duration::from_micros(1_500)));
}

#[test]
fn a_fraction_of_a_day_a_week_or_a_nanosecond_is_refused() {
    for text in ["1.5d", "0.5d", "1.5w", "1.5ns"] {
        assert_eq!(code(text), "malformed", "{text}");
    }
}

#[test]
fn a_leading_dot_is_refused_as_jiff_refuses_it() {
    // Pinned so a change in jiff's grammar is seen: `.5h` was a span of the
    // grammar this replaced.
    assert_eq!(code(".5h"), "malformed");
    assert_eq!(code("1.h"), "malformed");
}

#[test]
fn now_is_zero_and_whitespace_around_the_text_is_ignored() {
    assert_eq!(parse("now"), Ok(Duration::ZERO));
    assert_eq!(parse(" now "), Ok(Duration::ZERO));
    assert_eq!(parse(" 1h "), Ok(secs(3_600)));
    assert_eq!(code("NOW"), "malformed");
    assert_eq!(code("Now"), "malformed");
}

#[test]
fn a_bare_number_is_refused_zero_too() {
    for text in ["30", "0", "1h30", "1.5", "30 "] {
        assert_eq!(code(text), "malformed", "{text}");
    }
}

#[test]
fn a_unit_is_lowercase_and_a_word_is_no_span() {
    for text in [
        "1H", "1.5H", "1M", "NaN", "inf", "soon", "h", "d", "-", "+", "1hh",
    ] {
        assert_eq!(code(text), "malformed", "{text}");
    }
}

#[test]
fn nothing_is_empty() {
    assert_eq!(parse(""), Err(DurationError::Empty));
    assert_eq!(parse("  \t"), Err(DurationError::Empty));
}

#[test]
fn a_year_or_a_month_has_no_fixed_length() {
    for text in ["1y", "2mo", "1y 2d"] {
        assert_eq!(code(text), "calendar-unit", "{text}");
    }
    assert_eq!(parse("0y"), Ok(Duration::ZERO));
}

#[test]
fn a_sign_makes_a_signed_span_and_parse_refuses_it() {
    assert_eq!(parse_signed("-3d"), Ok(SignedDuration::from_secs(-259_200)));
    assert_eq!(parse_signed("+3d"), Ok(SignedDuration::from_secs(259_200)));
    assert_eq!(
        parse_signed("-1h30m"),
        Ok(SignedDuration::from_secs(-5_400))
    );
    assert_eq!(
        parse_signed("1h ago"),
        Ok(SignedDuration::from_secs(-3_600))
    );
    assert_eq!(parse_signed("-0d"), Ok(SignedDuration::ZERO));
    assert_eq!(parse("+3d"), Ok(secs(259_200)));
    for text in ["-3d", "-1.5h", "1h ago"] {
        assert_eq!(code(text), "negative", "{text}");
    }
}

#[test]
fn a_signed_span_is_whole_signed_nanoseconds() {
    let nanos = |text: &str| parse_signed(text).unwrap().as_nanos();
    assert_eq!(nanos("1us"), 1_000);
    assert_eq!(nanos("-2\u{b5}s3ns"), -2_003);
    assert_eq!(nanos("500ms"), 500_000_000);
    assert_eq!(nanos("-1h"), -3_600_000_000_000);
    assert_eq!(nanos("9223372036854775807ns"), i128::from(i64::MAX));
    assert!(parse_signed("9223372036854775808ns").is_err());
}

#[test]
fn the_greek_mu_is_not_the_micro_sign() {
    assert_eq!(code("2\u{3bc}s"), "malformed");
}

#[test]
fn a_span_that_is_too_long_is_malformed_with_jiffs_words() {
    assert_eq!(code("99999999999999d"), "malformed");
    assert_eq!(code("18446744073709551615s"), "malformed");
    assert_eq!(
        parse("9223372036854775807s"),
        Ok(secs(9_223_372_036_854_775_807))
    );
    assert_eq!(parse("175307617h"), Ok(secs(631_107_421_200)));
}

#[test]
fn a_refusal_names_the_text_and_carries_the_third_partys_words() {
    let args = |text: &str| {
        parse(text)
            .unwrap_err()
            .args()
            .into_iter()
            .map(|(k, v)| match v {
                Arg::Str(s) => (k.to_string(), s.to_string()),
                other => panic!("{other:?}"),
            })
            .collect::<Vec<_>>()
    };
    let keys = |text: &str| args(text).into_iter().map(|(k, _)| k).collect::<Vec<_>>();
    assert_eq!(keys("30"), ["text", "ext_err_txt"]);
    assert_eq!(keys("-3d"), ["text"]);
    assert_eq!(keys("1y"), ["text"]);
    assert_eq!(keys(""), Vec::<String>::new());
    assert_eq!(args("30")[0], ("text".to_string(), "30".to_string()));
    assert!(
        args("30")[1].1.contains("unit designator"),
        "{:?}",
        args("30")
    );
}

#[test]
fn an_error_is_a_diagnostic_whose_display_is_its_code_and_args() {
    let e: DurationError = parse("-3d").unwrap_err();
    assert_eq!(e.to_string(), "duration-text-negative text=-3d");
}

#[test]
fn an_exact_span_is_jiffs_compact_text() {
    assert_eq!(format_exact(secs(5_400)), "1h 30m");
    assert_eq!(format_exact(Duration::ZERO), "0s");
    assert_eq!(format_exact(Duration::from_millis(1_500)), "1s 500ms");
    assert_eq!(format_signed(SignedDuration::from_secs(-5_400)), "-1h 30m");
}

/// A seeded xorshift, so the property tests are deterministic and need no
/// crate of their own.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A duration of at most `i64::MAX` seconds, its magnitude chosen first
    /// so every range is hit.
    fn duration(&mut self) -> Duration {
        let bits = self.next() % 63;
        let seconds = (self.next() >> 1) >> (62 - bits);
        let nanos = u32::try_from(self.next() % 1_000_000_000).unwrap();
        let nanos = if self.next().is_multiple_of(3) {
            0
        } else {
            nanos
        };
        Duration::new(seconds, nanos)
    }
}

#[test]
fn an_exact_span_reads_back_to_the_same_duration() {
    let mut rng = Seeded(0x9E37_79B9_7F4A_7C15);
    let mut cases = vec![
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::new(0, 999_999_999),
        secs(1),
        secs(59),
        secs(31_557_600),
        Duration::new(i64::MAX.unsigned_abs(), 999_999_999),
    ];
    cases.extend((0..20_000).map(|_| rng.duration()));
    for d in cases {
        let text = format_exact(d);
        assert_eq!(parse(&text), Ok(d), "{text}");
    }
}

#[test]
fn an_exact_signed_span_reads_back_to_the_same_span() {
    let mut rng = Seeded(0x2545_F491_4F6C_DD1D);
    for _ in 0..20_000 {
        let magnitude = SignedDuration::try_from(rng.duration()).unwrap();
        let signed = if rng.next().is_multiple_of(2) {
            magnitude
        } else {
            -magnitude
        };
        let text = format_signed(signed);
        assert_eq!(parse_signed(&text), Ok(signed), "{text}");
    }
}

#[test]
fn an_age_is_rounded_down_to_its_largest_whole_unit() {
    for (age, text) in [
        (Duration::ZERO, "0s"),
        (Duration::from_millis(999), "0s"),
        (secs(12), "12s"),
        (secs(59), "59s"),
        (secs(60), "1m"),
        (secs(3 * 60 + 59), "3m"),
        (secs(5 * 3_600), "5h"),
        (secs(23 * 3_600 + 3_599), "23h"),
        (secs(86_400), "1d"),
        (secs(47 * 3_600), "1d"),
        (secs(2 * 86_400), "2d"),
        (secs(400 * 86_400 + 5), "400d"),
    ] {
        assert_eq!(format_age(age), text, "{age:?}");
    }
}

#[test]
fn an_age_beyond_a_jiff_span_of_days_is_capped() {
    let capped = format!("{MAX_AGE_DAYS}d");
    assert_eq!(format_age(Duration::from_secs(u64::MAX)), capped);
    assert_eq!(
        parse(&capped),
        Ok(secs(u64::try_from(MAX_AGE_DAYS).unwrap() * 86_400))
    );
}

#[test]
fn an_age_reads_back_to_its_rounded_value() {
    let mut rng = Seeded(0xD1B5_4A32_D192_ED03);
    let limit = u64::try_from(MAX_AGE_DAYS).unwrap() * 86_400;
    for _ in 0..20_000 {
        let age = Duration::new(rng.duration().as_secs() % limit, 123);
        let text = format_age(age);
        let read = parse(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
        assert!(read <= age, "{text} is more than {age:?}");
        assert_eq!(format_age(read), text, "{age:?}");
        // The loss is under one unit of the age's own size: under half.
        assert!(
            age.as_secs() == 0 || u128::from(read.as_secs()) * 2 > u128::from(age.as_secs()),
            "{text} loses more than half of {age:?}"
        );
    }
}

fn at(seconds: u64, nanos: u32) -> SystemTime {
    UNIX_EPOCH + Duration::new(seconds, nanos)
}

const NOW: fn() -> SystemTime = || at(1_800_000_000, 7);

#[test]
fn a_span_cutoff_is_that_long_before_now_keeping_its_fraction() {
    for (text, seconds) in [
        ("30d", 2_592_000),
        ("12w", 7_257_600),
        ("720h", 2_592_000),
        ("1h30m", 5_400),
        ("1h 30m", 5_400),
        ("0d", 0),
        ("now", 0),
    ] {
        assert_eq!(
            cutoff_at(text, NOW()),
            Ok(at(1_800_000_000 - seconds, 7)),
            "{text}"
        );
    }
}

#[test]
fn a_date_cutoff_is_its_utc_midnight_whatever_now_is() {
    assert_eq!(cutoff_at("2026-01-01", NOW()), Ok(at(1_767_225_600, 0)));
    assert_eq!(
        cutoff_at(" 2026-01-01 ", UNIX_EPOCH),
        Ok(at(1_767_225_600, 0))
    );
    assert_eq!(
        cutoff_at("1969-12-31", NOW()),
        Ok(UNIX_EPOCH - Duration::from_secs(86_400))
    );
}

#[test]
fn a_cutoff_refuses_what_is_neither() {
    let code = |text: &str| cutoff_at(text, NOW()).unwrap_err().code().to_string();
    for (text, expected) in [
        ("2026-13-40", "invalid-date"),
        ("2026-02-29", "invalid-date"),
        ("2026-1-1", "invalid-date"),
        ("2026-01-01T00:00:00Z", "malformed"),
        ("30", "malformed"),
        ("-3d", "negative"),
        ("1y", "calendar-unit"),
        ("", "empty"),
        ("2026-01-01 x", "malformed"),
        ("soon", "malformed"),
    ] {
        assert_eq!(code(text), format!("duration-text-{expected}"), "{text}");
    }
}

#[test]
fn a_comparison_is_exact() {
    use Comparison::{AtLeast, AtMost, LessThan, MoreThan};
    let span = secs(172_800);
    let under = Duration::new(172_799, 999_999_999);
    let on = secs(172_800);
    let on_and_a_half = Duration::new(172_800, 500_000_000);
    let over = secs(172_801);
    for (op, expected) in [
        (AtLeast, [false, true, true, true]),
        (MoreThan, [false, false, true, true]),
        (LessThan, [true, false, false, false]),
        (AtMost, [true, true, false, false]),
    ] {
        let got = [under, on, on_and_a_half, over].map(|age| op.holds(age, span));
        assert_eq!(got, expected, "{op:?}");
    }
}

#[test]
fn a_cutoff_before_the_earliest_instant_is_out_of_range() {
    let earliest = UNIX_EPOCH - Duration::from_secs(1 << 62);
    let far = format!("{}s", i64::MAX);
    assert_eq!(
        cutoff_at(&far, earliest).unwrap_err().code(),
        "duration-text-out-of-range"
    );
    assert!(cutoff_at("1s", NOW()).is_ok());
}

#[test]
fn an_age_is_zero_for_a_since_after_now() {
    let now = at(100, 500);
    assert_eq!(age_between(at(40, 0), now), Duration::new(60, 500));
    assert_eq!(age_between(now, now), Duration::ZERO);
    assert_eq!(age_between(at(100, 501), now), Duration::ZERO);
    assert_eq!(age_between(at(200, 0), now), Duration::ZERO);
}

/// A factory that counts its reads.
fn counting(reads: &Cell<u32>) -> impl Fn() -> SystemTime + '_ {
    move || {
        reads.set(reads.get() + 1);
        NOW()
    }
}

#[test]
fn an_instance_reads_now_once_per_use() {
    let reads = Cell::new(0);
    let spans = DurationText::new(counting(&reads));
    assert_eq!(reads.get(), 0);
    assert_eq!(spans.parse("1h"), Ok(Duration::from_secs(3_600)));
    assert_eq!(reads.get(), 0, "parse reads no clock");
    assert_eq!(spans.cutoff("1d"), Ok(at(1_800_000_000 - 86_400, 7)));
    assert_eq!(reads.get(), 1);
    assert_eq!(spans.age(at(1_799_999_000, 0)), Duration::new(1_000, 7));
    assert_eq!(reads.get(), 2);
    assert!(spans.age_is(
        at(1_799_999_000, 0),
        Comparison::AtLeast,
        Duration::from_secs(1_000)
    ));
    assert_eq!(reads.get(), 3);
    assert_eq!(spans.now(), NOW());
    assert_eq!(reads.get(), 4);
}

#[test]
fn a_frozen_instance_reads_the_factory_once_and_keeps_that_instant() {
    let reads = Cell::new(0);
    let spans = DurationText::new(counting(&reads));
    let frozen = spans.frozen();
    assert_eq!(reads.get(), 1);
    for _ in 0..3 {
        assert_eq!(frozen.cutoff("1d"), Ok(at(1_800_000_000 - 86_400, 7)));
        assert_eq!(frozen.age(at(1_799_999_000, 0)), Duration::new(1_000, 7));
    }
    assert_eq!(frozen.frozen().now(), NOW());
    assert_eq!(reads.get(), 1);
}

#[test]
fn a_fixed_instant_is_a_factory() {
    let spans = DurationText::new(at(1_000, 0));
    assert_eq!(spans.cutoff("10s"), Ok(at(990, 0)));
    assert_eq!(spans.age(at(400, 0)), Duration::from_secs(600));
}
