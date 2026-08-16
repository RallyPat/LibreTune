# setup-git-hooks.ps1
# One-time setup: point Git at the repo's shared hooks (.githooks/) so the
# pre-commit rustfmt check runs on every commit.
#
# Usage (from repo root):
#   pwsh ./scripts/setup-git-hooks.ps1      # or just:  powershell ./scripts/setup-git-hooks.ps1

$ErrorActionPreference = "Stop"
$repoRoot = git rev-parse --show-toplevel
if ($LASTEXITCODE -ne 0) { throw "Not inside a git repository." }

git -C $repoRoot config core.hooksPath ".githooks"
if ($LASTEXITCODE -ne 0) { throw "Failed to set core.hooksPath." }

# On Windows, Git runs *.sh hooks via Git Bash, so no exec bit is needed.
# Verify cargo fmt is callable, since that's what the hook runs.
$cargo = Get-Command cargo -ErrorAction SilentlyContinue
if (-not $cargo) {
    Write-Warning "cargo not found on PATH. Install Rust (https://rustup.rs) so the pre-commit hook can run cargo fmt."
} else {
    Write-Host "✔ Pre-commit hook enabled (core.hooksPath = .githooks)." -ForegroundColor Green
    Write-Host "  cargo fmt --check will run on every commit. Skip once with: git commit --no-verify"
}
