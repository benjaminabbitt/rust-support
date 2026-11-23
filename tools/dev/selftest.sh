#!/usr/bin/env bash
# Self-test of the dev module (dev.py): repin, pins-check (including a tracked
# cargo local-pins file), pins-reachable, land (dry run and with local bare remotes) and
# doctor, against throwaway git repositories built in a temporary directory
# under /home/babbitt (DEV_SELFTEST_TMP overrides; never /tmp, which is tmpfs).
# No network and no real cargo: cargo is a recording stub.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tool="$here/dev.py"
base=${DEV_SELFTEST_TMP:-/home/babbitt/.cache}
mkdir -p "$base"
W=$(mktemp -d "$base/dev-selftest.XXXXXX")
trap 'rm -rf "$W"' EXIT
fail() { echo "selftest FAIL: $*" >&2; exit 1; }

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=protocol.file.allow GIT_CONFIG_VALUE_0=always
export HOME="$W/home"; mkdir -p "$HOME"
export ANGZARR_DEV_REGISTRY="$W/repos.toml" ANGZARR_DEV_ROOT="$W/root" ANGZARR_DEV_CARGO="$W/fakecargo"
export CARGO_LOG="$W/cargo.log"
cat > "$W/fakecargo" <<'EOF'
#!/usr/bin/env bash
echo "$*" >> "$CARGO_LOG"
EOF
chmod +x "$W/fakecargo"
: > "$W/repos.toml"

dev() { python3 -I "$tool" "$@"; }
LAST=""
ok_run() { local rc=0; LAST=$("$@" 2>&1) || rc=$?; [ "$rc" = 0 ] || fail "'$*' exited $rc: $LAST"; }
expect_fail() { # description, grep pattern, command...
  local desc=$1 pat=$2 rc=0; shift 2
  LAST=$("$@" 2>&1) || rc=$?
  [ "$rc" != 0 ] || fail "$desc: expected a failure, got success: $LAST"
  grep -qE -- "$pat" <<<"$LAST" || fail "$desc: output lacks /$pat/: $LAST"
}
has() { grep -qE -- "$1" <<<"$LAST" || fail "${2:-output} lacks /$1/: $LAST"; }
hasnt() { ! grep -qE -- "$1" <<<"$LAST" || fail "${2:-output} has /$1/: $LAST"; }

# ---------------------------------------------------------------------------
echo "== version"
ok_run dev version
[ "$LAST" = "$(tr -d '[:space:]' < "$here/VERSION")" ] || fail "version is not VERSION: $LAST"

# ---------------------------------------------------------------------------
echo "== the git world: core <- lib <- app, spec as a submodule of app"
mkdir -p "$W/ws" "$W/remotes"
newrepo() {
  git init -q --bare -b main "$W/remotes/$1.git"
  git init -q -b main "$W/ws/$1"
  git -C "$W/ws/$1" remote add origin "$W/remotes/$1.git"
}
commit() { echo "$2" >> "$1/f.txt"; git -C "$1" add -A; git -C "$1" commit -qm "$2"; }
push() { git -C "$1" push -q origin HEAD:refs/heads/main; git -C "$1" fetch -q origin; }
head_of() { git -C "$W/ws/$1" rev-parse HEAD; }
url() { echo "file://$W/remotes/$1.git"; }
workspace() { printf '[workspace]\nresolver = "3"\nmembers = ["crates/*"]\n\n[workspace.dependencies]\n' > "$1/Cargo.toml"; }
member() { mkdir -p "$1/crates/$2"; printf '[package]\nname = "%s"\nversion = "0.1.0"\n' "$2" > "$1/crates/$2/Cargo.toml"; }
mklock() { # dir: a lock with a git source per git dependency of the root Cargo.toml
  { echo 'version = 4'; echo
    sed -nE 's|^([A-Za-z0-9_-]+) = \{ git = "([^"]+)", rev = "([0-9a-f]+)".*|\1 \2 \3|p' "$1/Cargo.toml" | while read -r n u r; do
      printf '[[package]]\nname = "%s"\nversion = "0.1.0"\nsource = "git+%s?rev=%s#%s"\n\n' "$n" "$u" "$r" "$r"; done
  } > "$1/Cargo.lock"
}
setpin() { # dir dep-crate url rev
  sed -i -E "s|^($2 = \{ git = \"[^\"]+\", rev = \")[0-9a-f]+|\1$4|" "$1/Cargo.toml"
}

newrepo core; workspace "$W/ws/core"; member "$W/ws/core" core-x; commit "$W/ws/core" c1; push "$W/ws/core"
newrepo lib;  workspace "$W/ws/lib";  member "$W/ws/lib" lib-a; member "$W/ws/lib" lib-b
echo "core-x = { git = \"$(url core)\", rev = \"$(head_of core)\" }" >> "$W/ws/lib/Cargo.toml"
mklock "$W/ws/lib"; commit "$W/ws/lib" l1; push "$W/ws/lib"
newrepo spec; echo spec > "$W/ws/spec/doc.md"; commit "$W/ws/spec" s1; push "$W/ws/spec"
newrepo app;  workspace "$W/ws/app"; member "$W/ws/app" app-bin
{ echo "lib-a = { git = \"$(url lib)\", rev = \"$(head_of lib)\" }"
  echo "lib-b = { git = \"$(url lib)\", rev = \"$(head_of lib)\" }"
  echo "core-x = { git = \"$(url core)\", rev = \"$(head_of core)\" }"; } >> "$W/ws/app/Cargo.toml"
mklock "$W/ws/app"
mkdir -p "$W/ws/app/src"; echo "LIB_REV = \"$(head_of lib)\"" > "$W/ws/app/src/rev.txt"
cat > "$W/ws/app/pins.toml" <<EOF
schema = 1

[[site]]
dep = "lib"
file = "src/rev.txt"
pattern = 'LIB_REV = "{rev}"'
EOF
printf '/.cargo/local-pins.toml\n' > "$W/ws/app/.gitignore"
git -C "$W/ws/app" submodule -q add "$(url spec)" spec
commit "$W/ws/app" a1; push "$W/ws/app"
cat > "$W/repos.toml" <<EOF
[repos.core]
path = "$W/ws/core"
[repos.lib]
path = "$W/ws/lib"
[repos.spec]
path = "$W/ws/spec"
EOF
app="$W/ws/app"

echo "== pins-check"
ok_run dev --project "$app" pins-check
has 'ok'
chk() { # description, pattern, mutation command (run in app), then undone by git checkout
  local desc=$1 pat=$2; shift 2
  ( cd "$app" && "$@" )
  expect_fail "pins-check $desc" "$pat" dev --project "$app" pins-check
  git -C "$app" reset -q --hard; git -C "$app" clean -qfd -e spec
}
chk "branch pin" 'branch' sed -i -E 's|^(lib-a = \{ git = "[^"]+")|\1, branch = "main"|' Cargo.toml
chk "tag pin" 'tag' sed -i -E 's|^(lib-a = \{ git = "[^"]+")|\1, tag = "v1"|' Cargo.toml
chk "short hash" '40' sed -i -E 's|(lib-b = \{ git = "[^"]+", rev = ")([0-9a-f]{7})[0-9a-f]+|\1\2|' Cargo.toml
chk "no rev" 'rev' sed -i -E 's|^(core-x = \{ git = "[^"]+"), rev = "[0-9a-f]+"|\1|' Cargo.toml
chk "outside path dep" 'outside' sed -i '$a outside-dep = { path = "../elsewhere" }' Cargo.toml
chk "patch table" 'patch' sh -c 'printf "\n[patch.crates-io]\nfoo = { path = \"crates/app-bin\" }\n" >> Cargo.toml'
chk "tracked cargo config" 'config' sh -c 'mkdir -p .cargo; printf "[patch.\"x\"]\n" > .cargo/config.toml; git add -f .cargo/config.toml'
chk "tracked local-pins file" 'local-pins\.toml.*tracked' sh -c 'mkdir -p .cargo; printf "# Generated by local-pins. Machine-local paths: do not commit.\n" > .cargo/local-pins.toml; git add -f .cargo/local-pins.toml'
chk "stale site" 'src/rev.txt' sh -c 'echo "LIB_REV = \"0000000000000000000000000000000000000000\"" > src/rev.txt'
chk "lock without the git source" 'Cargo.lock' sed -i '/^source = "git+file.*lib/d' Cargo.lock
# a workspace dependency nothing uses is not in the lock: not a finding
( cd "$app" && echo "unused-x = { git = \"$(url spec)\", rev = \"$(head_of spec)\" }" >> Cargo.toml )
ok_run dev --project "$app" pins-check
git -C "$app" checkout -q -- Cargo.toml
# an untracked (gitignored) local-pins file, included from a tracked config: passes
mkdir -p "$app/.cargo"
printf '# Generated by local-pins. Machine-local paths: do not commit.\n[patch."%s"]\nlib-a = { path = "%s" }\n' "$(url lib)" "$W/ws/lib/crates/lib-a" > "$app/.cargo/local-pins.toml"
printf 'include = [{ path = "local-pins.toml", optional = true }]\n' > "$app/.cargo/config.toml"
git -C "$app" add -f .cargo/config.toml
ok_run dev --project "$app" pins-check
git -C "$app" reset -q -- .cargo/config.toml; rm -f "$app/.cargo/config.toml"
printf 'schema = 2\n' > "$app/pins.toml.new"
cp "$app/pins.toml" "$W/pins.good"; cp "$app/pins.toml.new" "$app/pins.toml"
expect_fail "schema" 'schema' dev --project "$app" pins-check
printf 'schema = 1\nmin_version = "99.0.0"\n' > "$app/pins.toml"
expect_fail "min_version" 'version' dev --project "$app" pins-check
cp "$W/pins.good" "$app/pins.toml"; rm "$app/pins.toml.new"

echo "== doctor"
ok_run dev --project "$app" doctor
has "$W/repos.toml" "registry"; has 'lib'; has 'core'
has "-> $W/ws/lib\$" "lib's checkout"; has 'local-pins .*present' "local-pins status"
rm "$app/.cargo/local-pins.toml"
ok_run dev --project "$app" doctor; has 'local-pins +off' "local-pins status"
# the merged repo: several URLs, one checkout
mkdir -p "$W/ws/merged"; workspace "$W/ws/merged"; member "$W/ws/merged" core-x; member "$W/ws/merged" lib-a; member "$W/ws/merged" lib-b
git init -q -b main "$W/ws/merged"; git -C "$W/ws/merged" add -A; git -C "$W/ws/merged" commit -qm merged
printf '[repos.core]\npath = "%s"\n[repos.lib]\npath = "%s"\n' "$W/ws/merged" "$W/ws/merged" > "$W/repos.merged.toml"
ok_run dev --registry "$W/repos.merged.toml" --project "$app" doctor
[ "$(grep -c -- "-> $W/ws/merged\$" <<<"$LAST")" = 2 ] || fail "both URLs should resolve to the merged checkout: $LAST"
mv "$W/ws/core" "$W/ws/core.away"
expect_fail "doctor with a missing checkout" 'missing|not found|no checkout' dev --project "$app" doctor
mv "$W/ws/core.away" "$W/ws/core"

echo "== repin"
commit "$W/ws/core" c2; commit "$W/ws/lib" l2        # unpushed commits in both
: > "$CARGO_LOG"
ok_run dev --project "$app" repin
has 'lib'; has 'not pushed|unpushed'
grep -q "rev = \"$(head_of lib)\"" "$app/Cargo.toml" || fail "lib pin not moved to lib's HEAD"
grep -q "rev = \"$(head_of core)\"" "$app/Cargo.toml" || fail "core pin not moved to core's HEAD"
grep -qF "LIB_REV = \"$(head_of lib)\"" "$app/src/rev.txt" || fail "pins.toml site not rewritten"
grep -q 'update' "$CARGO_LOG" || fail "cargo update not run"
for p in lib-a lib-b core-x; do grep -q -- "-p $p" "$CARGO_LOG" || fail "cargo update lacks -p $p: $(cat "$CARGO_LOG")"; done
grep -q 'url\.' <<<"$(cat "$CARGO_LOG.env" 2>/dev/null || true)" || true
git -C "$app" diff --stat -- Cargo.toml | grep -q Cargo.toml || fail "no diff"
git -C "$app" checkout -q -- .
# only the changed pins get a cargo update
git -C "$W/ws/lib" reset -q --hard "$(git -C "$W/ws/lib" rev-parse HEAD~1)"
push "$W/ws/lib"                                    # lib back to its pushed tip
setpin "$app" lib-a x "$(head_of lib)"; setpin "$app" lib-b x "$(head_of lib)"
sed -i -E "s|^(core-x = \{ git = \"[^\"]+\", rev = \")[0-9a-f]+|\1$(git -C "$W/ws/core" rev-parse HEAD~1)|" "$app/Cargo.toml"
echo "LIB_REV = \"$(head_of lib)\"" > "$app/src/rev.txt"
: > "$CARGO_LOG"
ok_run dev --project "$app" repin
grep -q -- '-p core-x' "$CARGO_LOG" || fail "changed core-x not updated"
! grep -q -- '-p lib-a' "$CARGO_LOG" || fail "an unchanged pin was cargo-updated: $(cat "$CARGO_LOG")"
: > "$CARGO_LOG"
ok_run dev --project "$app" repin
has 'up to date'; [ ! -s "$CARGO_LOG" ] || fail "repin of an up to date repo ran cargo"
git -C "$app" checkout -q -- .
# dry run changes nothing
: > "$CARGO_LOG"; sed -i -E "s|^(core-x = \{ git = \"[^\"]+\", rev = \")[0-9a-f]+|\10000000000000000000000000000000000000000|" "$app/Cargo.toml"
cp "$app/Cargo.toml" "$W/cargo.dry"
ok_run dev --project "$app" repin --dry-run
cmp -s "$app/Cargo.toml" "$W/cargo.dry" || fail "dry run wrote"
git -C "$app" checkout -q -- .
# refusals: dirty, detached, diverged
echo x >> "$W/ws/core/f.txt"
expect_fail "dirty dependency" 'dirty' dev --project "$app" repin
git -C "$W/ws/core" checkout -q -- f.txt
git -C "$W/ws/core" checkout -q --detach
expect_fail "detached dependency" 'detached' dev --project "$app" repin
git -C "$W/ws/core" checkout -q main
push "$W/ws/core"
git -C "$W/ws/core" branch -q side "$(git -C "$W/ws/core" rev-parse HEAD~1)"; git -C "$W/ws/core" checkout -q side; commit "$W/ws/core" divergent
expect_fail "diverged dependency" 'diverge' dev --project "$app" repin
git -C "$W/ws/core" checkout -q main; git -C "$W/ws/core" branch -q -D side
git -C "$app" checkout -q -- .

echo "== pins-reachable"
sed -i -E "s|^(lib-a = \{ git = \"[^\"]+\", rev = \")[0-9a-f]+|\1$(head_of lib)|" "$app/Cargo.toml"
commit "$W/ws/lib" l3-unpushed
dev --project "$app" repin >/dev/null 2>&1 || true
git -C "$app" add -A; git -C "$app" commit -qm "pin unpushed lib"
expect_fail "pins to unpushed" 'push lib first \(or run just land\)' dev --project "$app" pins-reachable
git -C "$app" reset -q --hard HEAD~1                 # back to the pinned state
git -C "$W/ws/lib" reset -q --hard HEAD~1            # lib's HEAD = its pushed tip
dev --project "$app" repin >/dev/null 2>&1 || true
git -C "$app" checkout -q -- .
ok_run dev --project "$app" pins-reachable           # everything pinned is on a remote
# A hook's GIT_DIR (the repository being pushed) must not redirect git runs on the pinned checkouts.
GIT_DIR="$app/.git" ok_run dev --project "$app" pins-reachable
has 'ok|reachable'

echo "== land"
# lib and core each get an unpushed commit; app pins them (and the lib pin is in a site).
commit "$W/ws/core" c3; commit "$W/ws/lib" l4
dev --project "$app" repin >/dev/null 2>&1 || fail "repin before land"
git -C "$app" add -A; git -C "$app" commit -qm "repin"
remote_tip() { git ls-remote "$W/remotes/$1.git" refs/heads/main | cut -f1; }
before="$(remote_tip core) $(remote_tip lib) $(remote_tip app)"
ok_run dev land "$app"
has '\[1/3\] core'; has '\[2/3\] lib'; has '\[3/3\] app'; has 'dry run'; has 'order: core -> lib -> app'
[ "$before" = "$(remote_tip core) $(remote_tip lib) $(remote_tip app)" ] || fail "dry run pushed"
# the order does not depend on the order given
ok_run dev land "$app" "$W/ws/lib" "$W/ws/core"
has '\[1/3\] core'; has '\[2/3\] lib'; has '\[3/3\] app'
# a pin that is not an ancestor of the dependency tip stops the plan at the first failure
git -C "$W/ws/core" branch -q keep; git -C "$W/ws/core" checkout -q --detach HEAD~1; commit "$W/ws/core" c3-other
git -C "$W/ws/core" checkout -q -B main
expect_fail "lib's pin of core is not core's tip" 'ancestor|not on' dev land "$app"
has 'left|remaining|not reached'
git -C "$W/ws/core" checkout -q keep; git -C "$W/ws/core" branch -q -f main keep; git -C "$W/ws/core" checkout -q main; git -C "$W/ws/core" branch -q -D keep
# a dirty tree is refused; --keep-going (dry run) goes on and names every failure
echo y >> "$W/ws/lib/f.txt"
expect_fail "dirty tree" 'dirty|uncommitted' dev land "$app"
expect_fail "keep going" 'would stop' dev land "$app" --keep-going
has '\[3/3\] app'
git -C "$W/ws/lib" checkout -q -- f.txt
# a remote that moved on (not a fast-forward) is refused
git clone -q "$W/remotes/lib.git" "$W/other"; echo e > "$W/other/e.txt"; git -C "$W/other" add -A; git -C "$W/other" commit -qm elsewhere; git -C "$W/other" push -q origin HEAD:refs/heads/main
expect_fail "remote ahead" 'fast-forward|diverged|behind' dev land "$app"
git -C "$W/ws/lib" fetch -q origin; git -C "$W/ws/lib" rebase -q origin/main
dev --project "$app" repin >/dev/null 2>&1 || fail "repin after rebase"; git -C "$app" add -A; git -C "$app" commit -qm "repin 2"
# a repo-declared extra check that fails stops the push
cat >> "$W/ws/lib/pins.toml" <<EOF
schema = 1
[[check]]
name = "lockstep"
run = ["false"]
EOF
git -C "$W/ws/lib" add -A; git -C "$W/ws/lib" commit -qm "pins.toml"
dev --project "$app" repin >/dev/null 2>&1 || true; git -C "$app" add -A; git -C "$app" commit -qm "repin 3" 2>/dev/null || true
expect_fail "extra check" 'lockstep' dev land "$app" --go
[ "$(remote_tip lib)" != "$(head_of lib)" ] || fail "pushed past a failing check"
sed -i 's|run = \["false"\]|run = ["true"]|' "$W/ws/lib/pins.toml"; git -C "$W/ws/lib" commit -qam "check passes"
dev --project "$app" repin >/dev/null 2>&1 || true; git -C "$app" add -A; git -C "$app" commit -qm "repin 4" 2>/dev/null || true
# --go: pushes in order, verifies the remote tips
ok_run dev land "$app" --go
has 'landed'
for r in core lib app; do [ "$(remote_tip $r)" = "$(head_of $r)" ] || fail "remote tip of $r is not HEAD"; done
ok_run dev land "$app"; has 'nothing to push'

echo "ok   dev module selftest"
