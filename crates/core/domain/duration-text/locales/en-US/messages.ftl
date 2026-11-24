# Messages for the crate's errors. Each id is an error's code without the
# crate's name, which the build prefixes (diagnostic::build); the
# variables are its args.

empty = no span given: write a number and a unit, such as 5m, 1h30m or 2d, or a date such as 2026-01-01
empty-fix = write a span such as 90m, or the word now
malformed = { $text } is not a span: { $ext_err_txt }
malformed-fix = write a number and a unit, such as 90m, 1h30m, 2 days or 1.5h (the units are d, h, m, s, ms, us and ns, and w for weeks)
negative = { $text } is before nothing: a span here is not negative
negative-fix = write it without a sign or an ago
calendar-unit = { $text } has a year or a month, which has no fixed length
calendar-unit-fix = count it in days or weeks, such as 365d or 4w
invalid-date = { $text } is not a date: write it as YYYY-MM-DD, such as 2026-01-01
out-of-range = { $text } is longer than a span or a date can hold
