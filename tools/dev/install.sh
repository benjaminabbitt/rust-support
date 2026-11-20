#!/usr/bin/env bash
# Link this module into a repository: .dev -> this directory (gitignored), the
# target of the repository's `mod? dev '.dev'` line. A link, not a copy, so the
# module is one directory however many repositories use it; it may point at the
# main checkout or at a worktree of rust-support.
# usage: install.sh [repository (default: the current directory's)]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "${1:-.}" rev-parse --show-toplevel)
ln -sfn "$here" "$repo/.dev"
if ! git -C "$repo" check-ignore -q .dev; then
  echo "install: add '/.dev' to $repo/.gitignore" >&2
fi
echo "install: $repo/.dev -> $here"
