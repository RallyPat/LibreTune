# Contributing

Thank you for your interest in contributing to LibreTune!

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) 18+ and npm
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### Clone the Repository

```bash
git clone https://github.com/RallyPat/LibreTune.git
cd LibreTune
```

### Install Dependencies

```bash
# Frontend dependencies
cd crates/libretune-app
npm install

# Return to root
cd ../..
```

### Development Mode

```bash
cd crates/libretune-app
npm run tauri dev
```

This starts the app in development mode with hot-reloading.

## Project Structure

```
LibreTune/
├── crates/
│   ├── libretune-core/     # Rust core library
│   │   ├── src/
│   │   │   ├── ini/        # INI parsing
│   │   │   ├── protocol/   # ECU communication
│   │   │   ├── project/    # Project management
│   │   │   └── ...
│   │   └── tests/
│   └── libretune-app/      # Tauri desktop app
│       ├── src/            # React frontend
│       └── src-tauri/      # Tauri backend
├── docs/                   # Documentation
└── scripts/                # Build scripts
```

## Development Guidelines

### Code Style

**Rust:**
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Add doc comments (`///`) for public items

**TypeScript:**
- Use TypeScript strict mode
- Add JSDoc comments for components
- Follow React best practices

### Testing

```bash
# Rust tests
cargo test -p libretune-core

# TypeScript type checking
cd crates/libretune-app
npx tsc --noEmit
```

### Documentation

- Update AGENTS.md for implementation changes
- Add JSDoc/rustdoc for new APIs
- Update user manual for user-facing features

## Submitting Changes

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests: `cargo test && npm run typecheck`
5. Commit with descriptive message
6. Push to your fork
7. Open a Pull Request

### Commit Messages

Use clear, descriptive commit messages:
```
feat: Add wideband O2 support for rusEFI

- Parse lambda sensor configuration from INI
- Add lambda-to-AFR conversion
- Update dashboard to show lambda/AFR toggle
```

## Areas for Contribution

Check [GitHub Issues](https://github.com/RallyPat/LibreTune/issues) for:
- `good first issue` - Good for newcomers
- `help wanted` - Needs community help
- `enhancement` - New features
- `bug` - Bug fixes needed

## License

By contributing, you agree that your contributions will be licensed under the GPL-2.0 license.
