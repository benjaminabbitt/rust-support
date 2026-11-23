#!/usr/bin/env python3
"""Cross-project dependency automation: full-hash pins on commit, and push
sequencing across repositories. Path patches to sibling checkouts while
developing are `cargo local-pins`' job (a gitignored .cargo/local-pins.toml);
pins-check refuses that file tracked.

Standalone, stdlib only (python3 -I, tomllib); no third-party code. The licence
is the owner's to set. Run through the `dev` just module (the justfile beside
this file), or directly:

    dev.py [--project DIR] [--registry FILE] COMMAND [args]

Commands (the module's recipes are `dev::<command>`):

    repin [--dep NAME] [--dry-run] [--no-fetch] [--no-lock]
                      move every pin of a registered dependency to its checkout's HEAD
    pins-check        pre-commit: nothing path-based (no tracked .cargo/local-pins.toml),
                      full-hash pins, sites agree
    pins-reachable [--no-fetch]
                      pre-push: every pinned hash is on its dependency's remote
    land [REPO...] [--go] [--no-fetch] [--keep-going]
                      push the repositories in dependency order (dry run by default)
    doctor            what was found: registry, checkouts, sources, local-pins, missing
    version

The registry (DEV_REGISTRY / ~/.config/angzarr-dev/repos.toml) is data:

    root = "~/workspace"                  # where the usual places are searched
    [repos.angzarr]                       # key: a short name (the URL's last segment)
    path = "~/workspace/angzarr/core/main"          # or a list of paths, tried in order
    urls = ["github.com/angzarr-io/angzarr"]        # optional URL aliases
    default_branch = "main"                          # optional
    remote = "origin"                                # optional
    [repos]
    "https://github.com/x/y" = "~/src/y"  # a URL key, or `name = "path"`, also work

A repository's pins.toml (schema 1) declares what is repo-specific:

    schema = 1
    min_version = "1.0.0"
    [[site]]                               # a pin outside Cargo.toml
    dep = "angzarr-embedded"               # registry name of the dependency
    file = "crates/x/src/lib.rs"           # a path or glob
    pattern = 'ANGZARR_EMBEDDED_REV: &str = "{rev}"'   # {rev} is the 40-hex pin
    [[check]]                              # an extra check `land` runs before pushing
    name = "lockstep"
    run = ["just", "check-core-lockstep"]
"""
import fnmatch
import glob
import graphlib
import os
import re
import subprocess
import sys
import tempfile
import tomllib

HERE = os.path.dirname(os.path.realpath(__file__))
SCHEMA = 1
LOCAL_PINS = ".cargo/local-pins.toml"  # cargo local-pins' machine-local [patch] file
SKIP = {"target", ".git", "worktrees", "node_modules", "mutants.out"}
DEPS = ("dependencies", "dev-dependencies", "build-dependencies")
HEX40 = re.compile(r"^[0-9a-f]{40}$")


class Fail(Exception):
    pass


def version():
    with open(os.path.join(HERE, "VERSION")) as f:
        return f.read().strip()


def vtuple(s):
    return tuple(int(x) for x in re.findall(r"\d+", s)[:3])


def say(msg=""):
    print(msg, flush=True)


def warn(msg):
    print(f"warning: {msg}", file=sys.stderr, flush=True)


# ---- git and urls ---------------------------------------------------------

# What a git hook's environment names about the repository being pushed or
# committed. It would send every `git -C <other checkout>` here to that
# repository, so it never reaches a git run on another checkout.
REPO_ENV = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR",
            "GIT_PREFIX", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES")


def git_env(extra=None):
    full = {k: v for k, v in os.environ.items() if k not in REPO_ENV}
    if extra:
        full.update(extra)
    return full


def git(directory, *args, env=None, check=False):
    full = git_env(env)
    p = subprocess.run(["git", "-C", directory, *args], capture_output=True,
                       text=True, env=full)
    if check and p.returncode != 0:
        raise Fail(f"git {' '.join(args)} in {directory}: {p.stderr.strip()}")
    return p


def gout(directory, *args):
    p = git(directory, *args)
    return p.stdout.strip() if p.returncode == 0 else ""


def head(directory):
    return gout(directory, "rev-parse", "HEAD")


def norm_url(u):
    """host/path of a git URL in any spelling (https, ssh, scp-like, file, path)."""
    u = u.strip()
    if u.startswith("git+"):
        u = u[4:]
    m = re.match(r"^[A-Za-z][A-Za-z0-9+.-]*://(?:[^@/]*@)?(.*)$", u)
    if m:
        u = m.group(1)
    else:
        m = re.match(r"^(?:[^@/:]+@)?([^/:]+):(.*)$", u)
        if m and not u.startswith("/"):
            u = m.group(1) + "/" + m.group(2)
    u = u.split("?")[0].split("#")[0].rstrip("/")
    if u.endswith(".git"):
        u = u[:-4]
    return u if u.startswith("/") else u.lower()


def seg(u):
    return norm_url(u).rstrip("/").rsplit("/", 1)[-1]


def is_ancestor(directory, a, b):
    return git(directory, "merge-base", "--is-ancestor", a, b).returncode == 0


def toplevel(path):
    top = gout(path, "rev-parse", "--show-toplevel")
    return os.path.realpath(top) if top else os.path.abspath(path)


def tracked(project, *specs):
    p = git(project, "ls-files", "-z", "--", *specs)
    if p.returncode != 0:
        return []
    return [x for x in p.stdout.split("\0") if x]


def read_toml(path):
    try:
        with open(path, "rb") as f:
            return tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError):
        return None


# ---- registry -------------------------------------------------------------

class Entry:
    def __init__(self, name, paths, urls, branch, remote):
        self.name, self.paths, self.urls = name, paths, urls
        self.branch, self.remote = branch, remote


class Ctx:
    def __init__(self, project=None, registry=None, root=None):
        self.project = toplevel(project or os.getcwd())
        self.registry_file = os.path.abspath(os.path.expanduser(
            registry or os.environ.get("ANGZARR_DEV_REGISTRY")
            or "~/.config/angzarr-dev/repos.toml"))
        data = read_toml(self.registry_file) if os.path.isfile(self.registry_file) else {}
        if data is None:
            raise Fail(f"{self.registry_file} is not valid TOML")
        self.root = os.path.abspath(os.path.expanduser(
            root or os.environ.get("ANGZARR_DEV_ROOT") or data.get("root") or "~/workspace"))
        self.entries = []
        for key, val in (data.get("repos") or {}).items():
            spec = {"path": val} if isinstance(val, (str, list)) else dict(val)
            paths = spec.get("path") or []
            paths = [paths] if isinstance(paths, str) else list(paths)
            urls = {norm_url(u) for u in spec.get("urls", [])}
            name = key
            if "/" in key or ":" in key:
                urls.add(norm_url(key))
                name = seg(key)
            self.entries.append(Entry(name, [os.path.abspath(os.path.expanduser(p)) for p in paths],
                                      urls, spec.get("default_branch"), spec.get("remote")))
        self._fetched = set()

    def lookup(self, url):
        n = norm_url(url)
        for e in self.entries:
            if n in e.urls:
                return e
        s = seg(url)
        for e in self.entries:
            if e.name == s and not e.urls:
                return e
        return None

    def by_name(self, name):
        for e in self.entries:
            if e.name == name:
                return e
        return None

    def name_of(self, url):
        e = self.lookup(url)
        return e.name if e else seg(url)

    def candidates(self, s):
        """The usual places, then the worktrees, of a repository named s."""
        rel = [f"{s}", f"{s}/main", f"angzarr/{s}", f"angzarr/{s}/main",
               f"ctxloom/{s}", f"ctxloom/{s}/main",
               f"benjaminabbitt/{s}", f"benjaminabbitt/{s}/main"]
        prefixes = [s + "--"]
        if s == "angzarr":
            rel = ["angzarr/core/main"] + rel
            prefixes += ["core--", "angzarr-core--"]
        usual = [os.path.join(self.root, r) for r in rel]
        wt = os.path.join(self.root, "worktrees")
        trees = []
        if os.path.isdir(wt):
            trees = [os.path.join(wt, n) for n in sorted(os.listdir(wt)) if n.startswith(tuple(prefixes))]
        keep = lambda ds: [d for d in ds if os.path.isfile(os.path.join(d, "Cargo.toml"))]
        return keep(usual), keep(trees)

    def checkout(self, url_or_name, by_name=False):
        """One checkout directory of a repository: the registry's first path
        that exists, else the usual place that is a git repository, else the
        only worktree; None when there is none."""
        e = self.by_name(url_or_name) if by_name else self.lookup(url_or_name)
        if e and e.paths:
            for p in e.paths:
                if os.path.isdir(p):
                    return p
            raise Fail(f"{e.name}: registered at {', '.join(e.paths)}, which does not exist "
                       f"({self.registry_file})")
        s = url_or_name if by_name else seg(url_or_name)
        usual, trees = self.candidates(s)
        usual = [d for d in usual if os.path.exists(os.path.join(d, ".git"))]
        if usual:
            return usual[0]
        if len(trees) == 1:
            return trees[0]
        if len(trees) > 1:
            raise Fail(f"{s}: {len(trees)} worktrees and no main checkout; register one in "
                       f"{self.registry_file}")
        return None

    def remote_info(self, path, e=None):
        """(remote name, remote url, default branch) of a checkout; e is its registry entry."""
        remote = (e.remote if e and e.remote else None) or "origin"
        rurl = gout(path, "remote", "get-url", remote)
        if not rurl:
            raise Fail(f"{path} has no remote '{remote}'")
        branch = e.branch if e and e.branch else None
        if not branch:
            ref = gout(path, "symbolic-ref", "-q", f"refs/remotes/{remote}/HEAD")
            branch = ref.rsplit("/", 1)[-1] if ref else None
        if not branch:
            out = gout(path, "ls-remote", "--symref", remote, "HEAD")
            m = re.match(r"ref: refs/heads/(\S+)\s+HEAD", out)
            branch = m.group(1) if m else None
        if not branch:
            for b in ("main", "master"):
                if git(path, "rev-parse", "-q", "--verify", f"refs/remotes/{remote}/{b}").returncode == 0:
                    branch = b
                    break
        if not branch:
            raise Fail(f"cannot tell the default branch of {path}'s remote '{remote}'")
        return remote, rurl, branch

    def fetch(self, path, remote):
        key = (os.path.realpath(path), remote)
        if key in self._fetched:
            return True
        p = git(path, "fetch", "-q", remote)
        self._fetched.add(key)
        if p.returncode != 0:
            warn(f"git fetch {remote} in {path} failed: {p.stderr.strip()}")
            return False
        return True


# ---- manifests, the lock, pins -------------------------------------------

def manifests(project):
    out = [p for p in tracked(project, "Cargo.toml", "*/Cargo.toml") if os.path.basename(p) == "Cargo.toml"]
    if not out and not os.path.isdir(os.path.join(project, ".git")) and not os.path.isfile(os.path.join(project, ".git")):
        for here, dirs, files in os.walk(project):
            dirs[:] = [d for d in dirs if d not in SKIP]
            if "Cargo.toml" in files:
                out.append(os.path.relpath(os.path.join(here, "Cargo.toml"), project))
    return sorted(out)


def dep_tables(doc):
    def sections(d):
        for s in DEPS:
            if isinstance(d.get(s), dict):
                yield d[s]
    yield from sections(doc)
    ws = doc.get("workspace")
    if isinstance(ws, dict) and isinstance(ws.get("dependencies"), dict):
        yield ws["dependencies"]
    tgt = doc.get("target")
    if isinstance(tgt, dict):
        for v in tgt.values():
            if isinstance(v, dict):
                yield from sections(v)


class Dep:
    def __init__(self, key, spec):
        self.key = key
        self.package = spec.get("package", key)
        self.url = spec["git"]
        self.rev, self.branch, self.tag = spec.get("rev"), spec.get("branch"), spec.get("tag")


def git_deps(doc):
    for table in dep_tables(doc):
        for key, spec in table.items():
            if isinstance(spec, dict) and isinstance(spec.get("git"), str):
                yield Dep(key, spec)


def lock_sources(project):
    """[(package, url as written, rev)] of every git source in Cargo.lock."""
    doc = read_toml(os.path.join(project, "Cargo.lock"))
    out = []
    for p in (doc or {}).get("package", []):
        src = p.get("source", "")
        if src.startswith("git+"):
            rev = src.rsplit("#", 1)[-1] if "#" in src else ""
            out.append((p["name"], src[4:].split("?")[0].split("#")[0], rev))
    return out


def submodules(project):
    """[(path, url, committed hash)] of the submodules at HEAD."""
    out = []
    gm = os.path.join(project, ".gitmodules")
    if not os.path.isfile(gm):
        return out
    cfg = gout(project, "config", "-f", ".gitmodules", "--get-regexp", r"^submodule\..*\.(path|url)$")
    paths, urls = {}, {}
    for line in cfg.splitlines():
        k, _, v = line.partition(" ")
        name, _, field = k[len("submodule."):].rpartition(".")
        (paths if field == "path" else urls)[name] = v
    for name, path in paths.items():
        row = gout(project, "ls-tree", "HEAD", "--", path)
        m = re.match(r"160000 commit ([0-9a-f]{40})\t", row)
        if m and name in urls:
            out.append((path, urls[name], m.group(1)))
    return out


class Pin:
    def __init__(self, kind, url, rev, where, package=None):
        self.kind, self.url, self.rev, self.where, self.package = kind, url, rev, where, package


def pins_of(project):
    out = []
    for m in manifests(project):
        doc = read_toml(os.path.join(project, m))
        for d in git_deps(doc or {}):
            if d.rev:
                out.append(Pin("Cargo.toml", d.url, d.rev, f"{m}: {d.package}", d.package))
    for name, url, rev in lock_sources(project):
        if rev:
            out.append(Pin("Cargo.lock", url, rev, f"Cargo.lock: {name}", name))
    for path, url, rev in submodules(project):
        out.append(Pin("submodule", url, rev, f"submodule {path}"))
    return out


def load_pins(project):
    path = os.path.join(project, "pins.toml")
    if not os.path.isfile(path):
        return {"site": [], "check": []}
    doc = read_toml(path)
    if doc is None:
        raise Fail(f"{path} is not valid TOML")
    if doc.get("schema") != SCHEMA:
        raise Fail(f"{path}: schema {doc.get('schema')!r}, but this module (VERSION {version()}) "
                   f"reads schema {SCHEMA}; update the module or the file")
    need = doc.get("min_version")
    if need and vtuple(need) > vtuple(version()):
        raise Fail(f"{path} needs module version >= {need}; this is {version()}")
    doc.setdefault("site", [])
    doc.setdefault("check", [])
    return doc


def site_regex(pattern):
    if "{rev}" not in pattern:
        raise Fail(f"site pattern {pattern!r} has no {{rev}}")
    parts = pattern.split("{rev}")
    return re.compile("(?P<rev>[0-9a-fA-F]{40})".join(re.escape(p) for p in parts))


def site_files(project, site):
    hits = sorted(glob.glob(os.path.join(project, site["file"]), recursive=True))
    return [h for h in hits if os.path.isfile(h)]


# ---- repin ------------------------------------------------------------------

def dep_state(ctx, path, url, fetch):
    """(head, problems, warnings) of a dependency checkout."""
    problems, warns = [], []
    h = head(path)
    if gout(path, "status", "--porcelain", "-uno"):
        problems.append("dirty (uncommitted changes to tracked files)")
    if git(path, "symbolic-ref", "-q", "HEAD").returncode != 0:
        problems.append("detached HEAD (check out its branch)")
    try:
        remote, _, branch = ctx.remote_info(path, ctx.lookup(url) if url else None)
    except Fail as e:
        warns.append(f"{e}; cannot tell whether {h[:8]} is pushed")
        return h, problems, warns
    if fetch:
        ctx.fetch(path, remote)
    ref = f"refs/remotes/{remote}/{branch}"
    tip = gout(path, "rev-parse", "-q", "--verify", ref)
    if not tip:
        warns.append(f"{remote}/{branch} is not known here; {h[:8]} is not pushed")
    elif tip == h or is_ancestor(path, h, tip):
        pass
    elif is_ancestor(path, tip, h):
        n = gout(path, "rev-list", "--count", f"{tip}..{h}")
        warns.append(f"{h[:8]} is not pushed yet ({n} commit(s) ahead of {remote}/{branch}); "
                     f"`land` pushes in order")
    else:
        problems.append(f"diverged from {remote}/{branch} (HEAD {h[:8]} is neither behind nor ahead of {tip[:8]})")
    return h, problems, warns


def rewrite_revs(text, newrev):
    """Replace the rev of every git dependency table whose url newrev() knows.
    Returns (text, [(package, old, new)])."""
    changes = []

    def sub(region, key):
        mg = re.search(r'\bgit\s*=\s*"([^"]+)"', region)
        mr = re.search(r'\brev\s*=\s*"([0-9a-fA-F]+)"', region)
        if not (mg and mr):
            return region
        new = newrev(mg.group(1))
        if not new or new == mr.group(1):
            return region
        mp = re.search(r'\bpackage\s*=\s*"([^"]+)"', region)
        changes.append((mp.group(1) if mp else key, mr.group(1), new))
        return region[:mr.start(1)] + new + region[mr.end(1):]

    def inline(m):
        before = text[:m.start()].splitlines()[-1] if text[:m.start()] else ""
        mk = re.search(r'([\w.-]+)\s*=\s*$', before)
        return sub(m.group(0), mk.group(1) if mk else "?")
    out = re.sub(r"\{[^{}]*\}", inline, text)
    chunks = re.split(r"(?m)^(?=\[)", out)
    for i, c in enumerate(chunks):
        mh = re.match(r"\[[^\]\n]*dependencies\.([^\]\n]+)\]", c)
        if mh:
            chunks[i] = sub(c, mh.group(1).strip('"'))
    return "".join(chunks), changes


def cmd_repin(ctx, args):
    dry = "--dry-run" in args
    fetch = "--no-fetch" not in args
    nolock = "--no-lock" in args
    only = [args[i + 1] for i, a in enumerate(args) if a == "--dep" and i + 1 < len(args)]
    pins = load_pins(ctx.project)
    self_url = None
    try:
        self_url = norm_url(ctx.remote_info(ctx.project)[1])
    except Fail:
        pass
    # which repositories do the manifests pin?
    texts, urls = {}, {}
    for m in manifests(ctx.project):
        with open(os.path.join(ctx.project, m)) as f:
            texts[m] = f.read()
        for d in git_deps(read_toml(os.path.join(ctx.project, m)) or {}):
            urls.setdefault(norm_url(d.url), d.url)
    state, problems, mapping, local = {}, [], {}, {}
    names = {}
    for n, u in sorted(urls.items()):
        if n == self_url:
            continue
        name = ctx.name_of(u)
        if only and name not in only:
            continue
        path = ctx.checkout(u)
        if not path:
            say(f"note: no checkout of {name}; its pins are left alone")
            continue
        names[n] = name
        if path not in state:
            state[path] = dep_state(ctx, path, u, fetch)
        h, probs, warns = state[path]
        for p in probs:
            problems.append(f"{name} ({path}): {p}")
        mapping[n] = h
        local[u] = path
    for site in pins["site"]:
        if only and site["dep"] not in only:
            continue
        path = ctx.checkout(site["dep"], by_name=True)
        if path and path not in state:
            state[path] = dep_state(ctx, path, None, fetch)
            for p in state[path][1]:
                problems.append(f"{site['dep']} ({path}): {p}")
    if problems:
        raise Fail("repin refused:\n  " + "\n  ".join(problems))
    shown = set()
    for path, (h, _, warns) in state.items():
        for w in warns:
            if (path, w) not in shown:
                warn(f"{os.path.basename(path)}: {w}")
                shown.add((path, w))
    changed_pkgs, report, writes = [], [], {}
    for m, text in texts.items():
        new, ch = rewrite_revs(text, lambda u: mapping.get(norm_url(u)))
        if ch:
            writes[os.path.join(ctx.project, m)] = new
            for pkg, old, nw in ch:
                report.append(f"{m}: {pkg} {old[:8]} -> {nw[:8]}")
                if pkg not in changed_pkgs:
                    changed_pkgs.append(pkg)
    for site in pins["site"]:
        if only and site["dep"] not in only:
            continue
        path = ctx.checkout(site["dep"], by_name=True)
        if not path:
            raise Fail(f"site {site['file']}: no checkout of {site['dep']}")
        h = state[path][0]
        rx = site_regex(site["pattern"])
        files = site_files(ctx.project, site)
        if not files:
            raise Fail(f"site {site['file']}: no such file")
        for fp in files:
            if fp in writes:
                text = writes[fp]
            else:
                with open(fp) as f:
                    text = f.read()
            if not rx.search(text):
                raise Fail(f"site {os.path.relpath(fp, ctx.project)}: pattern {site['pattern']!r} not found")
            new = rx.sub(lambda m: m.group(0).replace(m.group("rev"), h), text)
            if new != text:
                writes[fp] = new
                report.append(f"{os.path.relpath(fp, ctx.project)}: site -> {h[:8]}")
    if not writes:
        say("repin: up to date")
        return
    for line in report:
        say(("would repin " if dry else "repinned ") + line)
    if dry:
        return
    for fp, text in writes.items():
        with open(fp, "w") as f:
            f.write(text)
    if changed_pkgs and not nolock and os.path.exists(os.path.join(ctx.project, "Cargo.lock")):
        cargo = os.environ.get("ANGZARR_DEV_CARGO", "cargo")
        cmd = [cargo, "update"] + [x for p in changed_pkgs for x in ("-p", p)]
        say("repin: " + " ".join(cmd))
        env = dict(os.environ, CARGO_NET_GIT_FETCH_WITH_CLI="true")
        n = int(env.get("GIT_CONFIG_COUNT", "0") or 0)
        for u, p in sorted(local.items()):  # unpushed commits resolve from the checkout
            env[f"GIT_CONFIG_KEY_{n}"] = f"url.{p}.insteadOf"
            env[f"GIT_CONFIG_VALUE_{n}"] = u
            n += 1
        env["GIT_CONFIG_COUNT"] = str(n)
        r = subprocess.run(cmd, cwd=ctx.project, env=env)
        if r.returncode != 0:
            raise Fail("cargo update failed; the manifests are repinned, the lock is not "
                       "(run it by hand, or `git checkout` the manifests)")


# ---- pins-check ---------------------------------------------------------------

def workspace_dirs(project):
    doc = read_toml(os.path.join(project, "Cargo.toml")) or {}
    ws = doc.get("workspace") or {}
    out = {""}
    for pat in ws.get("members", []):
        for hit in glob.glob(os.path.join(project, pat)):
            if os.path.isfile(os.path.join(hit, "Cargo.toml")):
                out.add(os.path.relpath(hit, project))
    for pat in ws.get("exclude", []):
        for hit in glob.glob(os.path.join(project, pat)):
            out.discard(os.path.relpath(hit, project))
    return out


def cmd_pins_check(ctx, args):
    project = ctx.project
    bad = []
    top = os.path.realpath(project)
    for f in tracked(project):
        if re.search(r"(^|/)\.cargo/local-pins\.toml$", f):
            bad.append(f"{f}: cargo local-pins' machine-local patch file is tracked "
                       f"(git rm --cached it; `cargo local-pins` gitignores /{LOCAL_PINS})")
        elif re.search(r"(^|/)\.cargo/config(\.toml)?$", f):
            doc = read_toml(os.path.join(project, f)) or {}
            if "patch" in doc or "replace" in doc or "paths" in doc:
                bad.append(f"{f}: a tracked cargo config with [patch]/[replace]/paths")
    lock = read_toml(os.path.join(project, "Cargo.lock"))
    in_ws = workspace_dirs(project)
    lock_have = {(p["name"], norm_url(p["source"][4:]), p["source"].rsplit("#", 1)[-1])
                 for p in (lock or {}).get("package", []) if p.get("source", "").startswith("git+")}
    lock_names = {p["name"] for p in (lock or {}).get("package", [])}
    revs_by_dep = {}
    for m in manifests(project):
        mdir = os.path.dirname(m)
        doc = read_toml(os.path.join(project, m))
        if doc is None:
            bad.append(f"{m}: not valid TOML")
            continue
        for k in ("patch", "replace"):
            if k in doc:
                bad.append(f"{m}: has [{k}]; nothing path-based is committed (use cargo local-pins)")
        for table in dep_tables(doc):
            for key, spec in table.items():
                if isinstance(spec, dict) and isinstance(spec.get("path"), str):
                    target = os.path.realpath(os.path.join(project, mdir, spec["path"]))
                    if target != top and not target.startswith(top + os.sep):
                        bad.append(f"{m}: {key} is a path dependency outside the repository ({spec['path']})")
        for d in git_deps(doc):
            if d.branch or d.tag:
                bad.append(f"{m}: {d.package} is pinned by {'branch' if d.branch else 'tag'}, not by rev")
            if not d.rev:
                bad.append(f"{m}: {d.package} (git) has no rev")
            elif not HEX40.match(d.rev):
                bad.append(f"{m}: {d.package} rev {d.rev!r} is not a full 40-hex hash")
            else:
                revs_by_dep.setdefault(ctx.name_of(d.url), set()).add(d.rev)
                # (a workspace dependency nothing uses is not in the lock at all)
                if d.package in lock_names and mdir in in_ws and (d.package, norm_url(d.url), d.rev) not in lock_have:
                    bad.append(f"Cargo.lock does not hold {d.package} at {d.rev[:8]} from git "
                               f"({m}): stale, or rewritten by a [patch] (run cargo local-pins clear, or cargo update -p {d.package})")
    for site in load_pins(project)["site"]:
        rx = site_regex(site["pattern"])
        files = site_files(project, site)
        if not files:
            bad.append(f"pins.toml site {site['file']}: no such file")
        for fp in files:
            rel = os.path.relpath(fp, project)
            with open(fp, errors="replace") as fh:
                found = [m.group("rev") for m in rx.finditer(fh.read())]
            if not found:
                bad.append(f"{rel}: pins.toml pattern {site['pattern']!r} not found")
            want = revs_by_dep.get(site["dep"])
            for h in found:
                if want and h not in want:
                    bad.append(f"{rel}: pins {site['dep']} at {h[:8]}, Cargo.toml pins "
                               f"{', '.join(sorted(r[:8] for r in want))} (run just dev::repin)")
    if bad:
        raise Fail("pins-check failed:\n  " + "\n  ".join(bad))
    say("pins-check: ok")


# ---- pins-reachable and land -----------------------------------------------------

def on_remote(ctx, pin, fetch):
    """(ok, dependency name, detail): is the pinned hash on the dependency's
    remote default branch? From the dependency's checkout when there is one,
    else by fetching the default branch's commits into a scratch repository."""
    name = ctx.name_of(pin.url)
    path = ctx.checkout(pin.url)
    if path:
        remote, rurl, branch = ctx.remote_info(path, ctx.lookup(pin.url))
        if fetch:
            ctx.fetch(path, remote)
        ref = f"refs/remotes/{remote}/{branch}"
        if not gout(path, "rev-parse", "-q", "--verify", ref):
            return False, name, f"{remote}/{branch} unknown in {path}"
        if git(path, "cat-file", "-e", pin.rev + "^{commit}").returncode != 0:
            return False, name, f"{pin.rev[:12]} is not in {path}"
        return (is_ancestor(path, pin.rev, ref), name, f"{pin.rev[:12]} not on {rurl} {branch}")
    with tempfile.TemporaryDirectory(prefix="dev-reach.", dir=os.environ.get("DEV_SCRATCH") or None) as td:
        git(td, "init", "-q", "--bare")
        out = gout(td, "ls-remote", "--symref", pin.url, "HEAD")
        m = re.match(r"ref: refs/heads/(\S+)\s+HEAD", out)
        if not m:
            return False, name, f"cannot reach {pin.url} to check {pin.rev[:12]}"
        p = git(td, "fetch", "-q", "--filter=tree:0", "--no-tags", pin.url, f"refs/heads/{m.group(1)}:refs/heads/tip")
        if p.returncode != 0:
            return False, name, f"cannot fetch {pin.url}: {p.stderr.strip()}"
        return (is_ancestor(td, pin.rev, "refs/heads/tip"), name, f"{pin.rev[:12]} not on {pin.url} {m.group(1)}")


def cmd_pins_reachable(ctx, args):
    fetch = "--no-fetch" not in args
    try:
        self_url = norm_url(ctx.remote_info(ctx.project)[1])
    except Fail:
        self_url = None
    seen, bad, n = set(), [], 0
    for pin in pins_of(ctx.project):
        key = (norm_url(pin.url), pin.rev)
        if key in seen or norm_url(pin.url) == self_url:
            continue
        seen.add(key)
        n += 1
        ok, name, detail = on_remote(ctx, pin, fetch)
        if not ok:
            bad.append(f"push {name} first (or run just land): {pin.where} pins {detail}")
    if bad:
        raise Fail("pins-reachable failed:\n  " + "\n  ".join(bad))
    say(f"pins-reachable: ok ({n} pinned hashes are on their remotes)")


class Node:
    def __init__(self, ctx, path, fetch):
        self.path = os.path.realpath(path)
        e = next((x for x in ctx.entries if any(os.path.realpath(p) == self.path for p in x.paths)), None)
        self.entry = e
        self.name = e.name if e else os.path.basename(self.path)
        self.remote, self.rurl, self.branch = ctx.remote_info(self.path, e)
        self.urlnorm = norm_url(self.rurl)
        if fetch:
            ctx.fetch(self.path, self.remote)
        self.ref = f"refs/remotes/{self.remote}/{self.branch}"
        self.tip = gout(self.path, "rev-parse", "-q", "--verify", self.ref)
        self.head = head(self.path)
        self.pins = pins_of(self.path)
        self.checks = load_pins(self.path)["check"]

    @property
    def ffable(self):
        return not self.tip or self.tip == self.head or is_ancestor(self.path, self.tip, self.head)

    @property
    def ahead(self):
        if not self.tip:
            return int(gout(self.path, "rev-list", "--count", "HEAD") or 0)
        return int(gout(self.path, "rev-list", "--count", f"{self.tip}..{self.head}") or 0) if self.ffable else 0


def cmd_land(ctx, args):
    go = "--go" in args
    fetch = "--no-fetch" not in args
    keep_going = "--keep-going" in args and not go  # a dry run reports every failure
    failures = []
    targets = [a for a in args if not a.startswith("--")] or [ctx.project]
    nodes = {}

    def node(path):
        p = os.path.realpath(path)
        if p not in nodes:
            nodes[p] = Node(ctx, p, fetch)
        return nodes[p]

    def resolve(t):
        if os.path.isdir(t):
            return toplevel(t)
        p = ctx.checkout(t, by_name=True)
        if not p:
            raise Fail(f"no repository {t!r}: not a directory, not in {ctx.registry_file}")
        return p
    plan = [node(resolve(t)) for t in targets]
    plan = list({n.path: n for n in plan}.values())
    dep_of = {}  # (node path, pin index) -> dep Node or None
    changed = True
    while changed:
        changed = False
        for n in list(plan):
            for i, pin in enumerate(n.pins):
                if norm_url(pin.url) == n.urlnorm:
                    continue
                dp = ctx.checkout(pin.url)
                if not dp:
                    dep_of[(n.path, i)] = None
                    continue
                d = node(dp)
                dep_of[(n.path, i)] = d
                if d in plan or d.urlnorm == n.urlnorm:
                    continue
                if d.tip and git(d.path, "cat-file", "-e", pin.rev + "^{commit}").returncode == 0 \
                        and is_ancestor(d.path, pin.rev, d.tip):
                    continue
                plan.append(d)
                changed = True
    ts = graphlib.TopologicalSorter()
    for n in sorted(plan, key=lambda x: x.name):
        deps = {dep_of[(n.path, i)] for i in range(len(n.pins)) if dep_of.get((n.path, i)) in plan}
        deps.discard(n)
        ts.add(n.path, *sorted(d.path for d in deps))
    try:
        order = [nodes[p] for p in ts.static_order()]
    except graphlib.CycleError as e:
        raise Fail(f"the dependency pins form a cycle: {' -> '.join(nodes[p].name for p in e.args[1])}")
    total = len(order)
    say(f"land plan ({'--go: pushing' if go else 'dry run; nothing is pushed without --go'}):")
    say("order: " + " -> ".join(n.name for n in order))
    done = []

    def planned_tip(d):
        return d.head if d.ffable else d.tip

    for idx, n in enumerate(order, 1):
        tag = f"[{idx}/{total}] {n.name}"
        problem = None
        if gout(n.path, "status", "--porcelain", "-uno"):
            problem = "dirty tree (uncommitted changes to tracked files)"
        elif not n.ffable:
            problem = (f"not a fast-forward of {n.remote}/{n.branch}: the remote has {n.tip[:8]}, "
                       f"which HEAD {n.head[:8]} does not contain (rebase or merge it)")
        else:
            for i, pin in enumerate(n.pins):
                d = dep_of.get((n.path, i))
                if norm_url(pin.url) == n.urlnorm:
                    continue
                if d is None:
                    ok, name, detail = on_remote(ctx, pin, fetch)
                    if not ok:
                        problem = f"{pin.where}: {detail}"
                        break
                    continue
                if git(d.path, "cat-file", "-e", pin.rev + "^{commit}").returncode != 0 \
                        or not is_ancestor(d.path, pin.rev, planned_tip(d)):
                    problem = (f"{pin.where} pins {d.name} at {pin.rev[:12]}, which is not an ancestor of "
                               f"{d.name}'s tip {planned_tip(d)[:12]}; push {d.name} first, or repin")
                    break
        if not problem:
            for c in n.checks:
                r = subprocess.run(c["run"], cwd=n.path, capture_output=True, text=True, env=git_env())
                if r.returncode != 0:
                    problem = f"check '{c.get('name', ' '.join(c['run']))}' failed: {(r.stdout + r.stderr).strip()[-400:]}"
                    break
        if problem and keep_going:
            say(f"FAIL {tag}: {problem}")
            failures.append(n.name)
            continue
        if problem:
            say(f"FAIL {tag}: {problem}")
            rest = [x.name for x in order[idx:]]
            say(f"stopped at {n.name}. landed: {', '.join(done) or 'none'}; not reached: {', '.join(rest) or 'none'}")
            raise Fail(f"land failed at {n.name}")
        if n.ahead == 0 and n.tip:
            say(f"{tag}: nothing to push (HEAD {n.head[:8]} is {n.remote}/{n.branch})")
            done.append(n.name)
            continue
        span = f"{n.tip[:8]}..{n.head[:8]}" if n.tip else f"new branch at {n.head[:8]}"
        say(f"{tag}: push {n.ahead} commit(s) ({span}) to {n.remote} {n.branch}  [{n.path}]")
        if go:
            r = subprocess.run(["git", "-C", n.path, "push", n.remote, f"HEAD:refs/heads/{n.branch}"], env=git_env())
            if r.returncode != 0:
                say(f"FAIL {tag}: git push was refused (its hooks, the remote, or branch protection)")
                rest = [x.name for x in order[idx:]]
                say(f"stopped at {n.name}. landed: {', '.join(done) or 'none'}; not reached: {', '.join(rest) or 'none'}")
                raise Fail(f"land failed at {n.name}")
            tip = gout(n.path, "ls-remote", n.remote, f"refs/heads/{n.branch}").split("\t")[0]
            if tip != n.head:
                say(f"FAIL {tag}: after the push {n.remote}/{n.branch} is {tip[:8] or 'missing'}, not {n.head[:8]}")
                raise Fail(f"land failed at {n.name}")
            say(f"{tag}: pushed; remote tip is {tip[:8]}")
        done.append(n.name)
    if failures:
        say("dry run: nothing was pushed")
        raise Fail(f"{len(failures)} repository(ies) would stop the landing: {', '.join(failures)}")
    say("landed: " + ", ".join(done) if go else "dry run: nothing was pushed; run with --go to push")


# ---- doctor ----------------------------------------------------------------------

def cmd_doctor(ctx, args):
    problems = []
    say(f"module version {version()} ({HERE})")
    say(f"project        {ctx.project}")
    say(f"registry       {ctx.registry_file} ({'present' if os.path.isfile(ctx.registry_file) else 'absent: heuristics only'})")
    say(f"root           {ctx.root}")
    for e in ctx.entries:
        miss = [p for p in e.paths if not os.path.isdir(p)]
        say(f"  repo {e.name}: {', '.join(e.paths) or '(by heuristics)'}" + (f"  MISSING {', '.join(miss)}" if miss else ""))
        problems += [f"{e.name}: {p} is missing" for p in miss]
    try:
        pins = load_pins(ctx.project)
        say(f"pins.toml      {'schema ' + str(SCHEMA) + ', ' + str(len(pins['site'])) + ' site(s), ' + str(len(pins['check'])) + ' check(s)' if os.path.isfile(os.path.join(ctx.project, 'pins.toml')) else 'absent'}")
    except Fail as e:
        problems.append(str(e))
    pins_file = os.path.join(ctx.project, LOCAL_PINS)
    say(f"local-pins     {LOCAL_PINS + ' present (cargo local-pins clear removes it)' if os.path.isfile(pins_file) else 'off (no ' + LOCAL_PINS + ')'}")
    src = {}  # normalised url -> the url as first written
    for _, url, _ in lock_sources(ctx.project):
        src.setdefault(norm_url(url), url)
    for m in manifests(ctx.project):
        for d in git_deps(read_toml(os.path.join(ctx.project, m)) or {}):
            src.setdefault(norm_url(d.url), d.url)
    say(f"git sources    {len(src)}")
    for url in (src[n] for n in sorted(src)):
        try:
            co = ctx.checkout(url)
        except Fail as e:
            say(f"  {url}\n      MISSING: {e}")
            problems.append(str(e))
            continue
        say(f"  {url}\n      -> {co or 'no checkout (repin leaves its pins alone)'}")
    for path, url, rev in submodules(ctx.project):
        try:
            co = ctx.checkout(url)
        except Fail as e:
            co = None
            problems.append(str(e))
        say(f"submodule      {path} {rev[:8]} {url} -> {co or 'no registered checkout'}")
    if problems:
        raise Fail("doctor found problems:\n  " + "\n  ".join(problems))
    say("doctor: ok")


def main(argv):
    project = registry = None
    argv = list(argv)
    while argv and argv[0] in ("--project", "--registry"):
        flag, val = argv[0], argv[1] if len(argv) > 1 else ""
        if flag == "--project":
            project = val
        else:
            registry = val
        argv = argv[2:]
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    cmd, args = argv[0], argv[1:]
    cmds = {"repin": cmd_repin, "pins-check": cmd_pins_check, "pins-reachable": cmd_pins_reachable, "land": cmd_land,
            "doctor": cmd_doctor}
    if cmd == "version":
        print(version())
        return 0
    if cmd not in cmds:
        print(f"dev: unknown command {cmd!r} (see --help)", file=sys.stderr)
        return 2
    try:
        cmds[cmd](Ctx(project, registry), args)
    except Fail as e:
        print(f"dev {cmd}: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
