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

- [ ] Code compiles without errors (`cargo build`, `npm run build`)
- [ ] Tests pass (`cargo test`)
- [ ] No new clippy warnings
- [ ] UI changes tested in the app
- [ ] Documentation updated if needed

## Reporting Issues

When reporting bugs, please include:
- Operating system and version
- ECU type (Speeduino, rusEFI, etc.)
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
