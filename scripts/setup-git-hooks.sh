#!/usr/bin/env bash
# setup-git-hooks.sh
# One-time setup: point Git at the repo's shared hooks (.githooks/) so the
# pre-commit rustfmt check runs on every commit.
#
# Usage (from repo root):
#   ./scripts/setup-git-hooks.sh

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

git config core.hooksPath ".githooks"
# On Unix the hook needs the executable bit.
chmod +x .githooks/pre-commit

if ! command -v cargo >/dev/null 2>&1; then
    echo "⚠️  cargo not found on PATH. Install Rust (https://rustup.rs) so the pre-commit hook can run cargo fmt."
else
    echo "✔ Pre-commit hook enabled (core.hooksPath = .githooks)."
    echo "   cargo fmt --check will run on every commit. Skip once with: git commit --no-verify"
fi
