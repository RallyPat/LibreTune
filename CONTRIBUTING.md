# Contributing to LibreTune

Thank you for your interest in contributing to LibreTune! This document provides guidelines and instructions for contributing.

## Getting Started

### Prerequisites

- **Rust 1.75+** - Install via [rustup](https://rustup.rs)
- **Node.js 18+** - For the Tauri frontend
- **npm** - Comes with Node.js

### Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/RallyPat/LibreTune.git
   cd LibreTune
   ```

2. Install frontend dependencies:
   ```bash
   cd crates/libretune-app
   npm install
   ```

3. Build the core library:
   ```bash
   cargo build -p libretune-core
   ```

4. Run in development mode:
   ```bash
   cd crates/libretune-app
   npm run tauri dev
   ```

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
npx tsc --noEmit
```

### Testing (Frontend)

We use Vitest + @testing-library/react for unit and integration tests in the frontend.

- Run the full test suite:
```bash
cd crates/libretune-app
npm test
```

- Run a single test file (watch-mode):
```bash
npm test -- <path/to/test-file>
```

- Test helpers:
  - The project provides `src/test-utils/setupTauriMocks.ts` to stub Tauri `core.invoke` and `event.listen` during tests. See `src/test-utils/README.md` for details and examples.
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

### TypeScript/React

- Use functional components with hooks
- Follow existing component patterns in the codebase
- Use TypeScript strict mode (already configured)
- Prefer `useMemo` for expensive computations

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

For release builds, increment `version` in `tauri.conf.json` (e.g., `0.2.0`) and create a git tag matching the version.

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
