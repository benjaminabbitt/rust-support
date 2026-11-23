# Exit codes: a convention for command-line programs

This document is a convention any command-line program can follow. It is
written to be copied into another project unchanged; nothing in it names a
particular program.

## The blocks

An exit code says which of four things happened, by the block it falls in:

| codes   | class    | meaning                                                        |
|---------|----------|----------------------------------------------------------------|
| 0       | success  | the command did what was asked (every success, without the flag) |
| 1-15    | (none)   | not used by this convention: left to generic errors            |
| 16-31   | success  | the command did what was asked (every success, under `--detailed-exitcode`; 16 is ok) |
| 32-47   | failure  | the command tried, and something broke                         |
| 48-63   | decline  | the command did not try: the request or the state forbids it   |
| 64-78   | (none)   | not used: BSD `sysexits.h`                                     |
| 101     | (none)   | not used: a Rust panic                                         |
| 126-127 | (none)   | not used: the shell (cannot execute, not found)                |
| 128+n   | signal   | the command was stopped by signal `n`                          |
| 255     | (none)   | not used: out of range (often `exit(-1)`)                      |

Codes not listed (79-100, 102-125) belong to no block and are not used.

The blocks are 16 wide and 16-aligned, so the class is the high nibble of the
status: under `--detailed-exitcode`, `status >> 4` is 1 for a success, 2 for
a failure, 3 for a decline, and 8 or more for a signal (255, out of range,
reads as 15 and is no signal a program sends). A caller that only cares
about the class tests that, or the range (`32 <= status && status <= 47` is
a failure). The high nibble 0 (statuses 1-15) is no class: a generic error,
a crash or an argument parser's own status lands there, and is never
mistaken for a success.

The success test is one comparison:

```sh
cmd --detailed-exitcode; status=$?
[ $(( status >> 4 )) -eq 1 ] && echo success   # subcode: $(( status & 15 ))
```

Without the flag the test is the usual `status == 0`.

The low integers are not in any block because they are the codes every
program already means something by: 1 is the catch-all for a general error
and 2 is the usage error that `argparse`, `clap` and the GNU tools return.
A success subcode must never be one of them: a caller that reads any
non-zero status as an error would be right and the program wrong. See
[Reserved ranges](#reserved-ranges-and-what-a-program-must-not-use).

Under `--detailed-exitcode` every success is its code in 16-31: 16 is a
plain success (ok), and 17-31 are *distinguishing* successes: the command
succeeded, and it has something more to say about how. Without that flag
every success exits 0. A plain success is 16, not 0, under the flag so that
one test (`status >> 4 == 1`) covers every success: were it 0, the success
classes would straddle 1-15, which no single mask or range can leave out.

### Why only the success block collapses

By long Unix convention a non-zero exit status means an error. `set -e`,
`&&`, `||`, make and every CI system treat any non-zero status as a failure.
A success must therefore exit 0 unless the caller has said it understands
otherwise, which is what `--detailed-exitcode` is: the caller opting in to
reading 16-31, and only 16-31, as successes.

The failure (32-47) and decline (48-63) blocks are always returned as they
are, with or without the flag. They are already non-zero, so under that same
convention they already read correctly as errors; collapsing them would only
lose information.

## The defined codes

| code | name             | class   | when                                                                                   |
|------|------------------|---------|----------------------------------------------------------------------------------------|
| 16   | ok               | success | the command succeeded; also a dry run or preview that succeeded (0 without the flag)   |
| 17   | would change     | success | a dry run or preview succeeded and applying it would change something (0 without the flag) |
| 32   | failed           | failure | the command tried and something broke: I/O, a store, a child program, the network     |
| 48   | refused          | decline | the request or the state forbids it: a bad argument, a usage error, a taken name, unreadable configuration |
| 49   | fatal findings   | decline | the command examined its input and found it cannot produce a working result            |
| 128+n| signal n         | signal  | the command was asked to stop by signal `n` and did                                    |

What belongs where:

- **Success (16-31; 0 without the flag).** Anything that completed as asked. A dry run, a preview
  or a check that ran to its end is a success even when what it reports is
  that something would change; the distinction is a success subcode, not an
  error.
- **Failure (32-47).** The command made an honest attempt and the attempt
  broke for a reason outside the request: a file could not be written, a
  database was locked, a child process crashed, a remote could not be
  reached. Trying again later, unchanged, may succeed.
- **Decline (48-63).** The command decided not to act, before or instead of
  acting: the request is malformed (a usage error from the argument parser
  is a decline, 48, not the parser's own exit status), the state forbids it
  (the thing is in use, the name is taken), or the input was examined and
  found unworkable (fatal findings, 49). Trying again unchanged will be
  declined again; something in the request or the state has to change.
- **Signal (128+n).** The shell's own convention for a process ended by a
  signal, kept so that a program that catches a signal, cleans up and exits
  reads to its caller exactly as if the signal had ended it.

An argument parser's request for help or for the version is a success (ok),
not a usage error.

A child program's own exit status is not one of these codes by default, and
is never passed through as it is: a child's 17 would read as a success
subcode. It is said in the classes: 0 is ok, a child ended by signal `n` (or
a shell's `128+n`) is signal `n`, and any other status is a failure (32).

A child known to follow this convention can be read by its blocks instead:
0 (its collapsed success) is ok, 16-31 is that success, 32-47 that failure, 48-63 that
decline, a signal is a signal, and any status outside the blocks (1, 2, 64-78,
101, 126, 127, ...) is still a failure (32). This is opt-in because a child
that does not follow the convention would have its own 16 to 63 misread.

## When two outcomes apply

A command sometimes has more than one thing to report: a launch both refused
and found fatal problems in its input, or a batch in which one item failed
and another was declined. It exits with the **most severe**, and severity is
the order of the codes themselves:

```text
signal (128+n)  >  decline (48-63)  >  failure (32-47)  >  success (16-31)
```

and within a block the higher code wins (49 fatal findings over 48 refused;
17 would change over 16 ok; signal 15 over signal 2). One rule, "the higher
detailed code wins", is the whole order.

Why a decline outranks a failure: a decline is a definitive verdict on the
request, while a failure may be transient. A caller that sees a failure is
invited to retry unchanged; if part of the same request was also declined,
that retry is declined again. Reporting the decline tells the caller the one
thing that must happen whatever else does: the request or the state has to
change. A failure is still said in the command's messages; only the code
carries the decline.

Why a signal outranks everything: the command was stopped from outside, so
nothing it would otherwise have reported is known to be complete.

Why any error outranks a success: an outcome that is partly an error is an
error to a caller reading the status under the Unix convention.

All the messages are still printed; only the exit code is chosen.

## Precedent

A dry run that succeeds exits 0 in the tools people already script against:
`rsync --dry-run`, `apt-get -s` and `terraform plan` all exit 0 when the
dry run itself worked, whatever it found. `terraform plan
-detailed-exitcode` is the model for the opt-in: it exits 0 for no changes,
2 for changes present and 1 for an error. This convention keeps the opt-in
and moves every success, ok included, into a block of its own (16-31),
clear of the low codes (1 and 2) that generic errors already use, so a
success can never be confused with an error code and one test finds it.

The blocks are sixteen wide so that each class has room for subcodes. A
caller that only cares about the class tests the block (`status >> 4 == 1` is
a success, `== 2` a failure); a caller that cares about the detail reads the code. A
new subcode never changes the class an existing caller sees.

## Reserved ranges and what a program must not use

The blocks sit where they do because the rest of the range is spoken for.
These facts are checked against the bash manual ("Exit Status"), `sysexits.h`
and the behaviour of bash 5.

### Shell and signal ranges (POSIX, Bash, Zsh)

Avoid 126 and above for application errors: shells and orchestrators treat
them as special.

| code    | meaning                                                                 |
|---------|-------------------------------------------------------------------------|
| 0       | success (this convention's, without `--detailed-exitcode`)              |
| 1       | catch-all for general or unspecified errors                             |
| 2       | misuse of a shell builtin; also the standard CLI usage or argument error (`argparse`, `clap`, GNU tools) |
| 3-125   | free for an application's own codes (64-78 optionally follow `sysexits.h`; 101 is a Rust panic) |
| 126     | the command was found but cannot execute (permission problem, or not an executable) |
| 127     | command not found                                                       |
| 128+n   | terminated by fatal signal `n` (130 = SIGINT, 137 = SIGKILL, 143 = SIGTERM) |
| 255     | status out of range: a status is taken modulo 256, so `exit(-1)` is 255 |

Two entries of a commonly copied version of this table are wrong. There is
no reserved 128 for "invalid argument to exit": bash's `exit foo` prints
"numeric argument required" and exits 2. And "out of range" is not a distinct
status: the value wraps, so `exit 256` is 0 and `exit -1` is 255.

This convention uses none of 1-15 (it leaves 1 and 2, and 3-15 as headroom,
to generic errors), 64-78, 101 or 126-127, nor 255, and 128+n only for a
signal. Its codes 16-63 collide with nothing a shell, a runtime or a common
library means by a status.

### BSD `sysexits.h` (64-78)

Created for sendmail and later adopted across the BSDs and many command-line
tools, it is the nearest thing to a formal taxonomy of granular command-line
failures. A program may use these where one fits, but this convention does
not: its failures are 32-47 and its declines 48-63.

| code | name           | meaning                                                  |
|------|----------------|----------------------------------------------------------|
| 64   | EX_USAGE       | command line usage error (bad flags, missing arguments)  |
| 65   | EX_DATAERR     | user-supplied input data was malformed                   |
| 66   | EX_NOINPUT     | an input file did not exist or was unreadable            |
| 67   | EX_NOUSER      | addressee unknown                                        |
| 68   | EX_NOHOST      | host name unknown                                        |
| 69   | EX_UNAVAILABLE | a required service or dependency is unavailable          |
| 70   | EX_SOFTWARE    | internal software error (non-OS)                         |
| 71   | EX_OSERR       | system error (for example, cannot fork or create a pipe) |
| 72   | EX_OSFILE      | a critical OS file is missing                            |
| 73   | EX_CANTCREAT   | cannot create a user-specified output file               |
| 74   | EX_IOERR       | input or output error during read or write               |
| 75   | EX_TEMPFAIL    | temporary failure; safe to retry                         |
| 76   | EX_PROTOCOL    | remote error in protocol                                 |
| 77   | EX_NOPERM      | permission denied                                        |
| 78   | EX_CONFIG      | configuration error                                      |

### Rust

A Rust program that panics exits 101 (with the default `panic = "unwind"`
and the standard runtime). It is not used here so that a panic is never read
as one of this convention's codes.

## Adding a subcode

1. Decide its class first: did the command succeed, try and break, or
   decline? The class is the block; it is what every existing caller reads.
2. Take the next free code in that block: success 18-31, failure 33-47,
   decline 50-63. Never reuse a code that was published with another
   meaning.
3. A new success subcode is returned only under `--detailed-exitcode`, like
   17. It must be a success the caller might want to branch on (as "would
   change" is); a warning is not a subcode.
4. Place it in the severity order: within its block, a higher code is more
   severe. If the new subcode should not outrank an existing one, it is the
   wrong block or the wrong number.
5. Document it in this table and in the program's own list of codes.

## Migrating from the 0 / 1-9 / 10-19 / 20-29 layout

The first layout put the blocks at 0-9 (successes), 10-19 (failures) and
20-29 (declines). A success subcode of 1 collided with the generic error code
1, and 2 with the usage error, and failures and declines fell among the
low application codes. The blocks moved to 16-aligned ranges above 15. Every
published non-zero code changes; there is no compatibility mapping, and a
caller that read the old numbers must be updated together with the program.
Only the codes the old layout defined are mapped: its other subcodes were
never published, so they have no new number.

| old   | name             | new                                    |
|-------|------------------|----------------------------------------|
| 0     | ok               | 16 (0 without the flag, as before)     |
| 1     | would change     | 17                                     |
| 10    | failed           | 32                                     |
| 20    | refused          | 48                                     |
| 21    | fatal findings   | 49                                     |
| 128+n | signal n         | 128+n (unchanged)                      |

A caller that tested a class by range moves from `10 <= status && status <=
19` to `32 <= status && status <= 47` (or `status >> 4 == 2`), and a script
that tested `status -eq 1` for "would change" tests 17. Under the flag a
plain success is 16, not 0: a script that tested `status -eq 0` with the
flag tests `status -eq 16`, or `status >> 4 == 1` for any success. A usage error is now
48, no longer 20.

## The type that encodes it

In the `exit-code` crate (pure: no I/O and no parser, so a program's core
may name its errors' classes with it, and re-exported by `cli-support`, the
command-line edge) the convention is the type `Exit` (and its `Class`):
constructors for each block that refuse a code outside it
(`Exit::success`, `Exit::failed`, `Exit::declined`, `Exit::signal`), the
defined codes as constants (`Exit::OK`, `Exit::WOULD_CHANGE`,
`Exit::FAILED`, `Exit::REFUSED`, `Exit::FATAL_FINDINGS`), `Exit::code`, the
single function that turns an exit and the `--detailed-exitcode` flag into
the process's status and holds the collapse rule, `Exit::worst` (and the
type's ordering), which is the severity order, and `Exit::from_child_status`,
which says a child program's status in the classes (a child that follows this convention is read by its blocks with `Exit::from_convention_child_status`), and `Exit::usage`, which maps
an argument parser's verdict to 48 for a usage error (or to ok for help and
version). An error type says its own class by implementing `ExitClass`. In
`cli-support`, a command's `Failure` carries its `Exit` (`Failure::of` takes
it from an `ExitClass` error), `usage_exit` reads a clap error, and
`detailed_exitcode_arg` and `detailed_exitcode` declare and read the global
flag.
