# Contributing to LibreTune

Thank you for your interest in contributing to LibreTune! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- **Rust (latest stable)** - Install via [rustup](https://rustup.rs)
- **Node.js 20+** - For the Tauri frontend
- **npm** - Comes with Node.js

### Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/RallyPat/LibreTune.git
   cd LibreTune
   ```

2. **Enable the pre-commit format hook (recommended):** This runs the same
   `cargo fmt --check` that CI enforces on every commit, so formatting
   failures never reach a PR. One-time setup, per clone:
   ```bash
   # Windows PowerShell:
   pwsh ./scripts/setup-git-hooks.ps1
   # macOS / Linux / Git Bash:
   ./scripts/setup-git-hooks.sh
   ```
   (What it does: `git config core.hooksPath .githooks`. The hook lives in
   `.githooks/pre-commit`. Skip it once with `git commit --no-verify`.)

3. Install frontend dependencies:
   ```bash
   cd crates/libretune-app
   npm install
   ```

4. Build the core library:
   ```bash
   cargo build -p libretune-core
   ```

5. Run in development mode:
   ```bash
   cd crates/libretune-app
   npm run tauri dev
   ```

> **Formatting:** Rust code must pass `cargo fmt --all -- --check` (the CI
> "Format Check" job). Run `cargo fmt --all` to auto-format before committing.
> The hook above does this automatically.

## Project Structure

```
libretune/
├── crates/
│   ├── libretune-core/      # Rust library (ECU communication, INI parsing)
│   └── libretune-app/       # Tauri desktop app
│       ├── src/             # React frontend (TypeScript)
│       └── src-tauri/       # Tauri backend (Rust)
├── docs/                    # Documentation and screenshots
└── scripts/                 # Development helper scripts
```

## Lua Scripting Feature

LibreTune includes sandboxed Lua 5.4 scripting for advanced automation. The build uses **vendored Lua**, which means:

- ✅ No system Lua installation required
- ✅ Works consistently across Windows, macOS, and Linux
- ✅ First build takes ~30-60 seconds (Lua C code compiled), subsequent builds use cache

**Optional**: To use system Lua instead (faster rebuilds if already installed):
```bash
# Ubuntu/Debian
sudo apt-get install liblua5.4-dev

# macOS
brew install lua@5.4

# Fedora/RHEL
sudo dnf install lua-devel
```

For details on Lua scripting implementation, see [Technical Reference: Lua Scripting](./docs/src/technical/lua-scripting.md).

## Development Commands

### Pre-Push Testing (Recommended)

**Always run local CI checks before pushing to GitHub** to catch errors early:

```bash
# Run all CI checks locally (mirrors GitHub Actions)
./scripts/pre-push.sh

# Install as automatic Git pre-push hook (runs before every push)
./scripts/pre-push.sh --install-hook
```

The pre-push script runs:
- ✅ Rust build, tests, and clippy
- ✅ Frontend TypeScript check, build, and tests
- ✅ Code formatting verification

**This prevents CI failures** by catching issues before they reach GitHub Actions.

### Backend (Rust)

```bash
# Build core library
cargo build -p libretune-core

# Run tests
cargo test -p libretune-core

# Run clippy lints
cargo clippy -p libretune-core

# Format code
cargo fmt
```

### Logging during development

LibreTune initializes a [`tracing`](https://docs.rs/tracing) subscriber in the
Tauri backend. By default the log level is `info`, so verbose ECU protocol debug
output is hidden. Use the `RUST_LOG` environment variable to control verbosity:

```bash
# Show detailed ECU communication logs
cd crates/libretune-app
RUST_LOG=libretune_core::protocol=debug npm run tauri dev

# Show plugin debug logs
RUST_LOG=libretune_core::plugin_system=debug npm run tauri dev

# Quiet everything except errors
RUST_LOG=error npm run tauri dev
```

### Frontend (React/TypeScript)

```bash
cd crates/libretune-app

# Development mode
npm run dev

# Full Tauri app development
npm run tauri dev

# Build for production
npm run build

# Type checking
npm run typecheck
```

### Testing (Frontend)

We use Vitest + @testing-library/react for unit and integration tests in the frontend.

- Run the full test suite (one-shot, as CI does):
```bash
cd crates/libretune-app
npm run test:run
```

- Run a single test file (watch-mode):
```bash
npm test -- <path/to/test-file>
```

- Test helpers:
  - The project provides `src/test-utils/tauriMocks.ts` to stub Tauri `core.invoke` and `event.listen` during tests. See `src/test-utils/README.md` for details and examples.
  - Use `setupTauriMocks()` in tests to provide deterministic `invoke` responses and deterministic event emission via `emit()` / `listen()` helpers.

- Test-writing tips:
  - Use `await waitFor(...)` or `findBy*` queries when asserting asynchronous DOM updates.
  - Wrap direct state updates in `act()` when manipulating stores in tests to avoid React warnings.
  - Add stubs for canvas drawing (already configured in `src/setupTests.ts`) if your component uses Canvas APIs.

Please update or extend the test utilities if you find repeated patterns that can be abstracted.

## Code Style

### Rust

- Follow standard Rust formatting (`cargo fmt`)
- Pass `cargo clippy` without warnings
- Write doc comments for public APIs
- Include unit tests for new functionality
- Comments should explain **why**, not restate **what** the code/test does.
  In tests, document fixture invariants (e.g. what state a helper guarantees)
  and the rationale behind each assertion rather than restating the test name.
  Module-level `//!` headers belong on every integration test file — see
  `crates/libretune-core/tests/{bit_extraction,f64_and_precision,plugin_api}.rs`
  for the target style.

### TypeScript/React

- Use functional components with hooks
- Follow existing component patterns in the codebase
- Use TypeScript strict mode (already configured)
- Prefer `useMemo` for expensive computations
- Document non-obvious algorithms, state machines, and magic numbers with the
  **why** (rationale, bug/issue references, edge cases). Keep JSDoc on
  component props. Reference files:
  `src/components/gauges/useGaugeRenderer.ts` and
  `src/hooks/useRealtimeStream.ts` exemplify the target style.

## Submitting Changes

### Pull Request Process

1. **Fork the repository** and create a feature branch from `main`
2. **Make your changes** following the code style guidelines
3. **Test your changes** - run `cargo test` and verify the UI works
4. **Update documentation** if needed (README, inline comments)
5. **Submit a pull request** with a clear description of the changes

### Commit Messages

Use clear, descriptive commit messages:
- `feat: Add AutoTune heatmap visualization`
- `fix: Resolve table editor cell selection bug`
- `docs: Update README with new screenshots`
- `refactor: Extract dialog components from App.tsx`

### PR Checklist

- [ ] **Pre-push checks pass** (`./scripts/pre-push.sh`)
- [ ] Code compiles without errors (`cargo build`, `npm run build`)
- [ ] Tests pass (`cargo test`, `npm run test:run`)
- [ ] No new clippy warnings (`cargo clippy --workspace`)
- [ ] Code is properly formatted (`cargo fmt --all -- --check`)
- [ ] UI changes tested in the app
- [ ] Documentation updated if needed

## Version Management & Build Info

### Nightly Builds

LibreTune uses semantic versioning with nightly build metadata:

- **Version**: `0.1.0-nightly` (fixed in [crates/libretune-app/src-tauri/tauri.conf.json](crates/libretune-app/src-tauri/tauri.conf.json))
- **Build ID**: Generated at compile-time from git metadata in [crates/libretune-app/src-tauri/build.rs](crates/libretune-app/src-tauri/build.rs)
- **Format**: `YYYY.MM.DD+g<short-sha>` (e.g., `2026.01.31+g3ab2a3f`)

The build ID is displayed in the **About** dialog for bug reporting and is verified by the CI workflow test `Verify build info format`.

### Release Builds

Push a tag `vX.Y.Z`. The release workflow stamps `X.Y.Z` into `tauri.conf.json`
via `tauri build --config`, so the version in the repo can stay at the nightly
placeholder; you only need to bump it when you want local builds to report it.

Tagged releases also produce **signed updater artifacts** that the in-app
updater (`Help → About LibreTune`) installs. The workflow refuses to run
without the signing key, so a maintainer sets it up once:

```bash
# 1. Generate a key pair. Keep the private key OUT of the repo and backed up:
#    losing it means existing installs can never be updated again.
cd crates/libretune-app && npx tauri signer generate -w ~/.tauri/libretune.key
```

2. Paste the printed **public** key into `plugins.updater.pubkey` in
   `crates/libretune-app/src-tauri/tauri.conf.json` and commit it.
3. Add two repository secrets: `TAURI_SIGNING_PRIVATE_KEY` (contents of
   `~/.tauri/libretune.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (the
   password you chose, empty if none).

The upload job rebuilds `latest.json` from the emitted `.sig` files and
verifies every signature against the committed public key before publishing,
so a key mismatch fails the release instead of surfacing on users' machines.

## Reporting Issues

When reporting bugs, please include:
- Operating system and version
- ECU type (Speeduino, rusEFI, etc.)
- **Build ID** (from About dialog, e.g., `2026.01.31+g3ab2a3f`)
- Steps to reproduce the issue
- Expected vs actual behavior
- Any error messages or logs

## Feature Requests

Feature requests are welcome! Please:
- Check existing issues to avoid duplicates
- Describe the use case and expected behavior
- Consider how it fits with existing functionality

## Questions?

Feel free to open a GitHub issue for questions or discussion.

## License

By contributing to LibreTune, you agree that your contributions will be licensed under the GPL-2.0 license.
