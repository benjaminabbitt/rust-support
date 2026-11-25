# Extract the generic substrate from angzarr into rust-support

## Goal

rust-support stops depending on angzarr. Today the generic crates it builds
on live in angzarr's repository, though none of them is about angzarr. They
move here, and angzarr depends on rust-support for them. What is
legitimately angzarr's (its store and runtime model) stays in angzarr.

A tool that needs only the substrate (git-tagma, the first) then builds
with no angzarr in its dependency graph.

## Ruled

- **Direction.** The dependency direction flips. Only the generic code moves
  into rust-support. angzarr depends on rust-support for it; rust-support
  never depends on angzarr.
- **Names.** The moved crates drop the `angzarr-` prefix (`coded-diagnostic`,
  `presentation`). Their diagnostic codes change with the package name
  (`angzarr-presentation-…` becomes `presentation-…`), and so do consumers'
  JSON output and Fluent catalogs. This is pre-1.0 everywhere, so no
  compatibility is owed.
- **Licence.** `coded-diagnostic` and `presentation` (AGPL-3.0-only with angzarr's
  runtime exception in angzarr) are relicensed to `Apache-2.0 OR
  BSD-3-Clause` as they move. Nothing AGPL enters rust-support, and
  `deny.toml` stays permissive-only.
- **`sqlite-store-opener` is angzarr's.** It opens angzarr-store-sqlite's
  store and shares angzarr's error types. It leaves rust-support in step 1,
  and angzarr gains it as `angzarr-store-sqlite-opener` beside
  angzarr-store-sqlite in step 3. Its one consumer, ctxloom, repins in
  step 4.
- **`test-dir` moves too.** angzarr's `angzarr-test-dir` (generic,
  Apache/BSD, a dev-dependency) becomes rust-support's `test-dir`. Its
  keep-on-panic variable becomes `TEST_DIR_KEEP`.
- **wasm32 gate.** `coded-diagnostic` keeps its `wasm32-unknown-unknown` build
  check, as a rust-support gate, so the leaf stays dependency-free.
- **Co-development through `[patch]`, not repins.** While the steps are in
  flight, each repo builds against the others' worktrees through a
  `[patch]` in an uncommitted, gitignored cargo config (`cargo local-pins`
  writes it as `.cargo/local-pins.toml`). Pins are bumped once per repo, to full commits,
  after everything is green. CI never sees the patches.

## What moves, what stays

| Crate (in angzarr today) | Disposition | Why |
|---|---|---|
| `os` | moves whole | "everything that differs between operating systems"; no angzarr concept |
| `file-system` | moves whole | the filesystem seam, written for taskloom and ctxloom |
| `file-lock` | moves whole | "ctxloom's advisory file locks", for the ctxloom family |
| `angzarr-presentation` | moves whole | domain-free by its own contract (its tests hold it to that) |
| `angzarr-diagnostic` | **splits** | the core is generic; `Recovery` is angzarr's |
| `angzarr-test-dir` | moves whole, as `test-dir` | a generic test scratch-directory guard |
| rust-support's `sqlite-store-opener` | **moves to angzarr** | a wrapper over angzarr-store-sqlite |

`angzarr-diagnostic` splits like this:

- **Moves:** `Diagnostic`, `Arg`, `Report`, `display`, `quote`, the `code!`
  macro, `catalog` (Fluent parse and check), `build` (catalog embedding),
  and `prefix`.
- **Stays in angzarr:** `Recovery` (Retry / Business / Invalid / …), the
  retry model of angzarr's stores and runtimes. It moves to `angzarr-ports`
  beside `StoreError::recovery`, which is its anchor. Its users (embedded,
  ports, bus, the store and runtime crates, and the ctxloom/taskloom
  kernels through angzarr-embedded) follow it there.

Angzarr crates that use the moved code and stay put, now depending on
rust-support: `store-jsonl`, `angzarr-build`, `angzarr-codegen`, and every
crate that names `Diagnostic` (about 22).

Layout here: the moved crates take rust-support's rings, so `arch-lint` holds
them:
- `coded-diagnostic`: `core/domain`, since it is pure
- `os`, `file-system` and `file-lock`: `adapters/driven`
- `presentation`: `adapters/driving`, or a `@presentation` class, as the
  rules already allow

## Exit codes

### Where the pieces stand

- **rust-support `main`** has the 16-aligned blocks (success 16–31,
  failure 32–47, decline 48–63) and the ruled shape:
  - `Exit::OK` = 0
  - `Exit::DETAILED_OK` = 16, which `OK` is reported as under the flag
  - every detailed success in 16–31
  - `ExitStatuses`, which binds `--detailed-exitcode` once at the edge
    (`cli_support::exit_statuses`)
- **Consumers are on the old 0–9 / 10–19 / 20–29 layout.** ctxloom's help
  still says "Exit 1 for a success that would change something". ctxloom
  calls `Exit::process(detailed)` directly.
- **Each consumer maps angzarr's `Recovery` by hand.** taskloom's
  kernel-runner and ctxloom's sessions retry on `Retry` and then report
  `GaveUp`. ctxloom's hooks map `Invalid` to `Malformed` and everything
  else to `Failed`.

### `Recovery` against `Class`

`Recovery` says who acts on a failure. `Class` says which block the process
status is in. They are kept as two types, because `Recovery` is angzarr's
(it stays) and `Class` is the convention's (rust-support). The mapping is
written once, in `angzarr-ports` next to `Recovery`, as
`impl From<Recovery> for exit_code::Class`. That is allowed now that
angzarr depends on rust-support; `exit-code` is pure, so a port crate may
name it.

| `Recovery` | Who acts | `Class` | Why |
|---|---|---|---|
| `Retry` | a machine, again | `Failed` (32–47) | "trying again later, unchanged, may succeed" is the failure block's definition |
| `Business` | the person who asked | `Declined` (48–63) | the state forbids it; the same request is refused the same way |
| `Invalid` | the caller | `Declined` (48–63) | a malformed request is a decline (as a usage error is, 48) |
| `Bug` | an operator | `Failed`, as `Exit::BROKEN` (47) | the command tried and broke; retrying will not help, and the request is not at fault |

Subcodes stay each program's own. The mapping fixes only the block, so
`rc >> 4` reads the same across the family.

### Ruled

- **`Bug` lands on a defined failure subcode, `Exit::BROKEN` = 47** ("an
  operator must act: a bug, damage, a misconfiguration"). It is in the
  failure block because the command tried and something broke. It sits at
  the top of the block, so the severity order ranks it above any transient
  failure.

### Order

These steps fold into the extraction order below.

- **E1, alongside step 1:** add `Exit::BROKEN` = 47 to `exit-code` and to
  `docs/exit-codes.md`. Gate: rust-support's suite. (`ExitStatuses` and the ruled `OK`/`DETAILED_OK`
  shape are already on `main`.)
- **E2, in step 3:** once angzarr depends on rust-support, add
  `From<Recovery> for Class` in `angzarr-ports`, with one test per variant.
- **E3, in step 4:** each consumer, in its single repin:
  - moves to the 16-aligned codes
  - updates its `--detailed-exitcode` help and its documented code table
  - replaces `Exit::process(detailed)` with `ExitStatuses`
  - routes its hand-written `Recovery` matches through the `Class` mapping,
    where one decides an exit

  Gate: that consumer's suite, plus a script check that `rc >> 4 == 1` holds
  for its successes under the flag.
- **E4, after every consumer is on `ExitStatuses`:** make `Exit::code(bool)`
  and `Exit::process(bool)` private to `exit-code`.

## Order (each step green on its own)

rust-support has to stop depending on angzarr **before** angzarr starts
depending on rust-support. Otherwise each repo depends on the other through
a git dependency, and cargo builds two copies of each crate. So the copy-in
and the cut happen in one rust-support branch, first.

1. **rust-support: copy in and cut the edge** (one branch).
   - Add the generic crates under their new names, in their rings, with
     their tests and catalogs.
   - Repoint `cli-support`, `run-within`, `duration-text`, `process-env` and
     `fixture-program` at them.
   - Remove `sqlite-store-opener`, keeping its source for step 3.
   - Drop every `angzarr-*` from `[workspace.dependencies]`.
   - Gate: `just build`, `just lint` and `just test` are green, and
     `cargo tree` has no `angzarr-*`.
2. **angzarr: move `Recovery`** into `angzarr-ports` and repoint its users.
   Nothing else changes. This can run alongside step 1. Gate: angzarr's
   suite.
3. **angzarr: repoint at rust-support** (after 1 and 2).
   - angzarr's crates depend on rust-support (pinned by full commit, step
     1's) for `coded-diagnostic`, `presentation`, `os`, `file-system`,
     `file-lock` and (as a dev-dependency) `test-dir`.
   - Delete angzarr's copies. Every caller is renamed with them
     (`angzarr_diagnostic::` becomes `coded_diagnostic::`, `angzarr_test_dir::`
     becomes `test_dir::`, `ANGZARR_KEEP_TEST_DIRS` becomes `TEST_DIR_KEEP`,
     and so on).
   - Add `angzarr-store-sqlite-opener`, from rust-support's
     `sqlite-store-opener`.
   - Add `From<Recovery> for exit_code::Class` (E2).
   - Gate: angzarr's suite, and `cargo tree` shows one `Diagnostic`.
4. **Consumers, one at a time, each in a single repin** (E3):
   1. ctxloom
   2. taskloom
   3. cherenkov
   4. angzarr-cli

   Each one bumps rust-support and angzarr together, to the commits that
   agree. Gate: that consumer's suite.

`store-jsonl` has already left angzarr's main, so it is not repointed.

**Trap:** until step 3 lands there are two `Diagnostic` traits, rust-support's and angzarr's. A
consumer that sees both cannot pass one where the other is expected. No
consumer moves until step 3 has landed, and each consumer moves in one
repin, never piecemeal.

## Not in scope

- `angzarr-yaml` stays in angzarr until it is decided separately. It depends
  only on diagnostic and serde, and it is generic too.
