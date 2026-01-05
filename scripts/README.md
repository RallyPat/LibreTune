# LibreTune Development Scripts

This directory contains utility scripts for developing LibreTune. All scripts are designed for Linux development environments.

## Quick Start

## Quick Start

1. **First-time setup** (install all dependencies):
   ```bash
   ./setup-dev.sh
   ```

2. **Launch the development server**:
   ```bash
   ./tauri-dev.sh
   ```

3. **Run tests**:
   ```bash
   ./dev-test.sh
   ```

4. **Clean build artifacts** (optional):
   ```bash
   ./dev-clean.sh --all
   ```

## Script Details

### `tauri-dev.sh` - Main Development Launcher

Launches the LibreTune Tauri application in development mode with sanitized environment to prevent WebKit crashes on Linux (caused by Snap environment variables).

**Usage:**
```bash
./tauri-dev.sh [OPTIONS]
```

**Options:**
  - `--verbose` - Show detailed output from npm and Tauri
  - `--log-file <path>` - Write output to log file (with timestamps)
  - `--clean` - Run `cargo clean` before launching
  - `--no-sanitize` - Disable environment sanitization (advanced, for debugging or special environments)
  - `--help` - Display help message

**Examples:**
```bash
# Basic launch
./tauri-dev.sh

# Launch with detailed output and logging
./tauri-dev.sh --verbose --log-file /tmp/libretune-dev.log

# Clean build artifacts before launch
./tauri-dev.sh --clean
```

**Preserved Environment Variables:**
- PATH, HOME, DISPLAY, XAUTHORITY, XDG_RUNTIME_DIR, TERM, NVM_DIR
- RUSTUP_HOME, CARGO_HOME, RUSTUP_TOOLCHAIN (for Rust toolchain)

**Notes:**
- `tauri-dev.sh` automatically detects and sources nvm (Node Version Manager) if needed
- Environment sanitization prevents Snap-related variables from causing WebKit crashes
- The script will attempt to launch up to 3 times if npm is not found, with 2-second delays
- NVM directory can be customized via `NVM_DIR` environment variable
- Use `--no-sanitize` flag if sanitization causes issues (advanced usage)

---

### `setup-dev.sh` - One-Time Environment Setup

Automatically installs and configures the complete development environment.

**What it installs:**
- Rust (latest stable via rustup)
- Node.js (LTS 20 via nvm)
- System dependencies (webkit2gtk for Tauri)
- npm packages

**Usage:**
```bash
./setup-dev.sh
```

**Note:** This script requires sudo access for installing system packages and internet connection for downloading dependencies.

**After installation:**
You may need to add these lines to your shell profile (`~/.bashrc` or `~/.zshrc`):
```bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
```

Then open a new terminal or run: `source ~/.bashrc` (or `~/.zshrc`)

---

### `dev-test.sh` - Test Runner

Runs all tests and displays a summary report.

**What it tests:**
- Rust unit tests (`cargo test`)
- TypeScript type checking (`npm run typecheck`)

**Usage:**
```bash
./dev-test.sh
```

**Exit codes:**
- 0 - All tests passed
- 1 - Some tests failed

---

### `dev-clean.sh` - Build Artifact Cleanup

Removes build artifacts and dependencies.

**Usage:**
```bash
./dev-clean.sh [OPTIONS]
```

**Options:**
- `--cargo` - Clean Rust build artifacts (cargo clean)
- `--node` - Remove node_modules directory
- `--dist` - Remove dist directory
- `--all` - Clean all with single confirmation
- `--help` - Display help message

**Examples:**
```bash
# Clean Rust artifacts only
./dev-clean.sh --cargo

# Remove node_modules only
./dev-clean.sh --node

# Clean everything with single confirmation
./dev-clean.sh --all

# Clean all with individual confirmations
./dev-clean.sh --cargo --node --dist
```

**Note:** This will permanently remove build artifacts and dependencies. You'll need to rebuild or reinstall them afterwards.

---

## Common Workflows

### First Time Setup
```bash
cd libretune
./scripts/setup-dev.sh
# Follow instructions to source shell profile if needed
./scripts/tauri-dev.sh
```

### Daily Development
```bash
cd libretune
./scripts/tauri-dev.sh
```

### Testing Changes
```bash
./scripts/dev-test.sh
```

### Fresh Build
```bash
./scripts/dev-clean.sh --all
./scripts/tauri-dev.sh
```

### Debugging with Logs
```bash
./scripts/tauri-dev.sh --verbose --log-file /tmp/libretune-dev.log
# Check logs with: tail -f /tmp/libretune-dev.log
```

---

## Troubleshooting

### "Rust/Cargo not found"
Run `./scripts/setup-dev.sh` to install Rust.

### "Node.js not found"
Run `./scripts/setup-dev.sh` to install Node.js.

### "webkit2gtk not found"
Run `./scripts/setup-dev.sh` to install system dependencies (requires sudo).

### "Permission denied" when running scripts
Scripts are designed to automatically make themselves executable on first run. If that fails, run:
```bash
chmod +x scripts/*.sh
```

### Tauri app crashes on startup with WebKit error
The `tauri-dev.sh` script sanitizes the environment to prevent this. If you still have issues, ensure you're using `tauri-dev.sh` and not running `npm run tauri dev` directly.

### nvm/Node.js not available in new terminal
Add these lines to your `~/.bashrc` or `~/.zshrc`:
```bash
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
```

Then run: `source ~/.bashrc` (or `~/.zshrc`)

---

## System Requirements

- Linux operating system (tested on Ubuntu/Debian, Fedora/RHEL, Arch)
- Internet connection for downloading dependencies
- Sudo access for installing system packages (webkit2gtk)
- Approximately 5GB free disk space for Rust toolchain and dependencies

---

## File Structure

```
libretune/
├── scripts/
│   ├── tauri-dev.sh       # Main development launcher
│   ├── setup-dev.sh       # Environment setup
│   ├── dev-test.sh        # Test runner
│   ├── dev-clean.sh       # Cleanup utility
│   └── README.md          # This file
├── crates/
│   ├── libretune-core/    # Rust core library
│   └── libretune-app/     # Tauri application
└── Cargo.toml
```

---

## Support

For issues or questions about the LibreTune project, please refer to the main project documentation.
