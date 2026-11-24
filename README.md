# rust-support

The shared, engine- and tool-agnostic substrate that ctxloom's Rust
applications build on. It is the Rust successor to the archived Go
`ctxloom/shared`, and it is not a port of it: it holds what more than one
application needs, angzarr included: angzarr depends on it, never the other
way.

It knows no application's domain. Each application passes its own names
(program name, directory and file names, environment prefix, root rule) as
arguments.

## Crates

| Crate | Path | Role |
|---|---|---|
| `cli-support` | `crates/adapters/driving/cli-support` | the driving edge's shared shims |
| `config-layers` | `crates/adapters/driven/config-layers` | layered configuration intake (env scan, override split, stamp check) |
| `process-env` | `crates/root/lib/process-env` | the once-read snapshot of the process environment |
| `project-dir` | `crates/adapters/driven/project-dir` | the project-root rule over the filesystem seam |
| `run-within` | `crates/adapters/driven/run-within` | run a program under a deadline: its process tree killed, its output bounded, the wait for it bounded; or a long-lived program on live pipes with no deadline (`run_piped`), its tree ended through a shared handle |
| `fixture-program` | `crates/adapters/driven/fixture-program` | test support: a fake program (a script) a test runs by path, never exec'd as written, so it cannot fail with ETXTBSY |
| `duration-text` | `crates/core/domain/duration-text` | the grammar of a span (`1h30m`, `1.5d`) or a date as text: parse, cutoff, age and exact comparison, over std's `Duration` and `SystemTime` |
| `exit-code` | `crates/core/domain/exit-code` | the exit-code convention (`docs/exit-codes.md`) as a type |
| `coded-diagnostic` | `crates/core/domain/coded-diagnostic` | localizable errors: a stable code and named args on every error, Fluent catalogs keyed by the code, and the build helper that embeds a crate's catalogs under its name |
| `presentation` | `crates/adapters/driving/presentation` | presentation at the driving edge: JSON or UTF-8 text, diagnostics rendered from Fluent catalogs, display-width text |
| `os` | `crates/adapters/driven/os` | everything that differs between operating systems: each flavor's path and environment logic, and the host's syscalls behind one trait |
| `file-system` | `crates/adapters/driven/file-system` | the one seam every filesystem operation goes through: the real filesystem, or the same re-rooted under a directory (a test's own) |
| `file-lock` | `crates/adapters/driven/file-lock` | advisory file locks: `flock` on a lock file through the seams, re-checked against the file at its path, waited for a bounded time |
| `test-dir` | `crates/adapters/driven/test-dir` | test support: a scratch directory for a crate below the file-system seam, removed when its guard drops (kept for a panicking test with `TEST_DIR_KEEP`) |
| `arch-lint` | `crates/root/bin/arch-lint` | the architecture lint: the workspace's dependency edges against `arch-rules.toml` |

`duration-text` holds code too (pure; see below). Today `cli-support` holds code (the call with an application-owned `globals`, the failure and its exit codes, the notices, the failure reporter, the argument readers); the others are stubs, and their code arrives slice by slice.

`templates/` holds files consumers copy (the OS and filesystem clippy rule);
it is not a crate.

## duration-text

`1h30m`, `2w1d`, `1.5d`, `0d`: units `s m h d w` (fixed lengths), longest to
shortest, each once; fractions; no bare number, sign or inner space; the
word `now` is zero. A
cutoff is a span before now or a `YYYY-MM-DD` date at 00:00 UTC. The crate
reads no clock: build a `DurationText` over a closure on your own clock (or a
fixed `SystemTime` in tests), and `frozen()` it to judge many items against
one instant.

```rust
use std::time::{Duration, SystemTime};
use duration_text::{Comparison, DurationText};

// `clock.now()` is the application's own clock seam, converted to std.
let spans = DurationText::new(|| SystemTime::from(clock.now())).frozen();
let cutoff = spans.cutoff("30d")?;           // or "2026-01-01"
let old = spans.age_is(created, Comparison::AtLeast, spans.parse("1.5h")?);
```

## Using it

Pin the repository by full commit:

```toml
cli-support = { git = "https://github.com/benjaminabbitt/rust-support", rev = "<full sha>" }
```

All crates move together: a consumer bumps the whole repository, never one
crate. To build a consumer against a local checkout, see `CLAUDE.md`.

## Working on it

`just build`, `just test`, `just lint`, `just lint-deps`; see `CLAUDE.md`.

## Development tools

`tools/dev` is a `just` module for developing this repository and its consumers together (file system references, pins, push order); see [tools/dev/README.md](tools/dev/README.md). Git hooks are never installed automatically: run `lefthook install` once per clone.

## Licence

Apache-2.0 OR BSD-3-Clause, at your option: see [LICENSE-APACHE](LICENSE-APACHE)
and [LICENSE-BSD](LICENSE-BSD).
