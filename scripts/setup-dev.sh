#!/bin/bash
# LibreTune Development Environment Setup
# Automatically installs Rust, Node.js, and system dependencies

set -e

# Script options
SKIP_SYSTEM_DEPS=false
CHECK_ONLY=false
MAX_RETRIES=2
RETRY_COUNT=0

# Auto-executable check - make script executable if it isn't already
if [[ ! -x "$0" ]]; then
  chmod +x "$0"
  exec "$0" "$@"
fi

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log() {
  echo -e "$1"
}

log_success() {
  log "${GREEN}✓ $1${NC}"
}

log_error() {
  log "${RED}✗ $1${NC}"
}

log_warning() {
  log "${YELLOW}⚠ $1${NC}"
}

log_info() {
  log "${BLUE}ℹ $1${NC}"
}

log_section() {
  echo ""
  log "${BLUE}═════════════════════════════════════════════════════════${NC}"
  log "${BLUE}$1${NC}"
  log "${BLUE}═══════════════════════════════════════════════════════════${NC}"
  echo ""
}

# Show help message
show_help() {
  cat << EOF
LibreTune Development Environment Setup

Usage: $0 [OPTIONS]

Options:
  --skip-system-deps    Skip system dependency installation
  --check-only          Only check for available packages, don't install
  --help                 Show this help message

Examples:
  $0                      # Full setup (install everything)
  $0 --check-only         # Check what packages are available
  $0 --skip-system-deps  # Skip webkit2gtk installation

This script will install:
  • Rust (latest stable)
  • Node.js (LTS 20 via nvm)
  • System dependencies (webkit2gtk for Tauri)
  • npm packages
EOF
}

# Detect Linux distribution
detect_distro() {
  if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    DISTRO=$ID
    DISTRO_NAME=$PRETTY_NAME
  else
    log_error "Cannot detect Linux distribution"
    exit 1
  fi
}

# Check if command exists
command_exists() {
  command -v "$1" &> /dev/null
}

# Install Rust via rustup
install_rust() {
  log_section "Installing Rust (latest stable via rustup)"

  if command_exists cargo; then
    log_success "Rust is already installed: $(cargo --version)"
    return 0
  fi

  log_info "Installing Rust via rustup..."
  if curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; then
    log_success "Rust installed successfully!"

    # Source cargo environment for this session
    if [[ -f "$HOME/.cargo/env" ]]; then
      source "$HOME/.cargo/env"
      log_success "Cargo environment loaded: $(cargo --version)"
    fi

    log_info "NOTE: Rust has been installed for your shell profile."
    log_info "      Open a new terminal or run: source \$HOME/.cargo/env"
  else
    log_error "Failed to install Rust"
    exit 1
  fi
}

# Install Node.js via nvm
install_nodejs() {
  log_section "Installing Node.js (LTS via nvm)"

  if command_exists node; then
    log_success "Node.js is already installed: $(node --version)"
    return 0
  fi

  # Install nvm if not already installed
  if [[ ! -d "$HOME/.nvm" ]]; then
    log_info "Installing nvm (Node Version Manager)..."
    if curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash; then
      log_success "nvm installed successfully!"
    else
      log_error "Failed to install nvm"
      exit 1
    fi
  else
    log_success "nvm is already installed"
  fi

  # Source nvm
  export NVM_DIR="$HOME/.nvm"
  if [[ -s "$NVM_DIR/nvm.sh" ]]; then
    . "$NVM_DIR/nvm.sh"
  else
    log_error "Failed to source nvm"
    exit 1
  fi

  # Install latest LTS version of Node.js
  log_info "Installing Node.js LTS (20)..."
  if nvm install --lts; then
    log_success "Node.js installed successfully: $(node --version)"
    log_success "npm installed: $(npm --version)"

    # Set LTS as default
    nvm alias default 'lts/*'
    log_success "LTS set as default Node version"
  else
    log_error "Failed to install Node.js"
    exit 1
  fi

  # Inform user about shell configuration
  echo ""
  log_info "═══════════════════════════════════════════════════════════"
  log_info "Node.js has been installed via nvm"
  log_info "═══════════════════════════════════════════════════════════"
  echo ""
  log_warning "IMPORTANT: Node.js (via nvm) needs to be loaded by your shell."
  echo ""
  log_info "Add this line to your shell profile (~/.bashrc or ~/.zshrc):"
  echo ""
  echo "  export NVM_DIR=\"\$HOME/.nvm\""
  echo "  [ -s \"\$NVM_DIR/nvm.sh\" ] && \. \"\$NVM_DIR/nvm.sh\""
  echo ""
  log_info "Then open a new terminal or run: source ~/.bashrc (or ~/.zshrc)"
  echo ""
}

# Install system dependencies for Tauri
install_system_deps() {
  log_section "Installing System Dependencies for Tauri"

  # Cache sudo credentials
  log_info "Checking sudo access..."
  # Use -S flag for non-interactive mode when running in --check-only
  if [ "$CHECK_ONLY" = true ]; then
    if ! sudo -n true 2>/dev/null; then
      log_warning "Cannot verify sudo access in check-only mode"
      log_info "Continuing with package availability check..."
    fi
  elif ! sudo -v; then
    log_error "This script requires sudo access to install system packages"
    exit 1
  fi

  # Install webkit2gtk (required for Tauri on Linux)
  if command_exists apt; then
    install_webkit_apt
  elif command_exists dnf; then
    install_webkit_dnf
  elif command_exists pacman; then
    install_webkit_pacman
  else
    log_warning "Unsupported package manager for webkit2gtk installation"
    log_info "Manual installation may be required"
  fi
}

# Diagnostic function to check available webkit2gtk packages
check_webkit_packages() {
  log_section "Checking for Available webkit2gtk Packages"

  if ! command_exists apt; then
    log_warning "Package check only available for apt-based systems"
    return 1
  fi

  log_info "Available webkit2gtk packages:"
  echo ""
  apt-cache search webkit2gtk 2>/dev/null | grep -E "^webkit2gtk|^libwebkit" | head -10
  echo ""

  FOUND=false

  if apt-cache show webkit2gtk-4.1 &>/dev/null; then
    log_success "✓ webkit2gtk-4.1 available"
    FOUND=true
  elif apt-cache show webkit2gtk-4.0 &>/dev/null; then
    log_success "✓ webkit2gtk-4.0 available (4.1 not found)"
    FOUND=true
  elif apt-cache show libwebkit2gtk-4.1-dev &>/dev/null; then
    log_success "✓ libwebkit2gtk-4.1-dev available"
    FOUND=true
  fi

  if [ "$FOUND" = false ]; then
    log_error "No webkit2gtk package found in repositories"
    log_warning "Please try: sudo apt update && sudo apt search webkit2gtk"
    return 1
  fi

  return 0
}

# Install webkit2gtk on Debian/Ubuntu-based systems
install_webkit_apt() {
  log_info "Detected Debian/Ubuntu-based system"

  # Install Lua 5.4 development libraries (optional but recommended)
  # Note: libretune-core uses vendored Lua by default, so this is optional
  log_info "Installing Lua 5.4 development libraries (optional)..."
  if sudo apt-get install -y liblua5.4-dev 2>/dev/null; then
    log_success "Lua 5.4 dev libraries installed"
  else
    log_warning "Could not install Lua 5.4 (vendored Lua will be used in builds)"
  fi

  # Try to find available webkit2gtk package with automatic retry
  WEBKIT_PKG=""

  while [[ $RETRY_COUNT -lt $MAX_RETRIES ]] && [[ -z "$WEBKIT_PKG" ]]; do
    RETRY_COUNT=$((RETRY_COUNT + 1))
    log_info "Attempt $RETRY_COUNT/$MAX_RETRIES: Finding available webkit2gtk package..."

    # Update package lists on first attempt
    if [ $RETRY_COUNT -eq 1 ]; then
      log_info "Updating package lists..."
      sudo apt update
    fi

    # Try multiple package names in order of preference
    if apt-cache show libwebkit2gtk-4.1-dev &>/dev/null; then
      WEBKIT_PKG="libwebkit2gtk-4.1-dev"
    elif apt-cache show libwebkit2gtk-4.1-0 &>/dev/null; then
      WEBKIT_PKG="libwebkit2gtk-4.1-0"
    elif apt-cache show webkit2gtk-4.1 &>/dev/null; then
      WEBKIT_PKG="webkit2gtk-4.1"
    elif apt-cache show webkit2gtk-4.0 &>/dev/null; then
      WEBKIT_PKG="webkit2gtk-4.0"
    elif apt-cache show webkit2gtk &>/dev/null; then
      WEBKIT_PKG="webkit2gtk"
    fi

    # Linux Mint specific handling
    if [[ "$DISTRO" == "linuxmint" ]] && [[ -z "$WEBKIT_PKG" ]]; then
      log_warning "Linux Mint detected - checking for Mint-specific packages"
      if apt-cache show libwebkit2gtk-4.1-0 &>/dev/null; then
        WEBKIT_PKG="libwebkit2gtk-4.1-0"
      fi
    fi

    # If still not found, try updating again and checking
    if [[ -z "$WEBKIT_PKG" ]] && [ $RETRY_COUNT -lt $MAX_RETRIES ]; then
      log_warning "Package not found, retrying with fresh package list..."
      sleep 1
    fi
  done

  # If all retries failed, fall back to manual installation
  if [[ -z "$WEBKIT_PKG" ]]; then
    log_error "Unable to locate webkit2gtk package after $MAX_RETRIES attempts"
    show_manual_installation_guide
    return 1
  fi

  log_info "Found package: $WEBKIT_PKG"

  # Check-only mode
  if [ "$CHECK_ONLY" = true ]; then
    log_success "Check complete: Would install $WEBKIT_PKG"
    return 0
  fi

  # Attempt installation
  log_info "Installing webkit2gtk ($WEBKIT_PKG)..."

  # Also install development package if available
  DEV_PKG=""
  if [[ "$WEBKIT_PKG" == "webkit2gtk-4.1" ]]; then
    if apt-cache show libwebkit2gtk-4.1-dev &>/dev/null; then
      DEV_PKG="libwebkit2gtk-4.1-dev"
    fi
  elif [[ "$WEBKIT_PKG" == "webkit2gtk-4.0" ]]; then
    if apt-cache show libwebkit2gtk-4.0-dev &>/dev/null; then
      DEV_PKG="libwebkit2gtk-4.0-dev"
    fi
  fi

  PKG_CMD="$WEBKIT_PKG"
  if [[ -n "$DEV_PKG" ]]; then
    PKG_CMD="$WEBKIT_PKG $DEV_PKG"
  fi

  if sudo apt install -y $PKG_CMD; then
    log_success "webkit2gtk installed successfully"
    return 0
  else
    log_error "Failed to install webkit2gtk"
    show_manual_installation_guide
    return 1
  fi
}

# Install webkit2gtk on Fedora/RHEL-based systems
install_webkit_dnf() {
  log_info "Detected Fedora/RHEL-based system"

  # Try to find available webkit2gtk package
  WEBKIT_PKG=""
  if dnf list webkit2gtk4.1-devel &>/dev/null 2>&1; then
    WEBKIT_PKG="webkit2gtk4.1-devel"
  elif dnf list webkit2gtk4.0-devel &>/dev/null 2>&1; then
    WEBKIT_PKG="webkit2gtk4.0-devel"
  elif dnf list webkit2gtk-devel &>/dev/null 2>&1; then
    WEBKIT_PKG="webkit2gtk-devel"
  fi

  if [[ -z "$WEBKIT_PKG" ]]; then
    log_error "Unable to locate webkit2gtk package"
    show_manual_installation_guide
    return 1
  fi

  log_info "Found package: $WEBKIT_PKG"

  if [ "$CHECK_ONLY" = true ]; then
    log_success "Check complete: Would install $WEBKIT_PKG"
    return 0
  fi

  log_info "Installing webkit2gtk ($WEBKIT_PKG)..."

  if sudo dnf install -y "$WEBKIT_PKG"; then
    log_success "webkit2gtk installed successfully"
    return 0
  else
    log_error "Failed to install webkit2gtk"
    show_manual_installation_guide
    return 1
  fi
}

# Install webkit2gtk on Arch-based systems
install_webkit_pacman() {
  log_info "Detected Arch-based system"

  # Try to find available webkit2gtk package
  WEBKIT_PKG=""
  if pacman -Si webkit2gtk-4.1 &>/dev/null; then
    WEBKIT_PKG="webkit2gtk-4.1"
  elif pacman -Si webkit2gtk-4.0 &>/dev/null; then
    WEBKIT_PKG="webkit2gtk-4.0"
  elif pacman -Si webkit2gtk &>/dev/null; then
    WEBKIT_PKG="webkit2gtk"
  fi

  if [[ -z "$WEBKIT_PKG" ]]; then
    log_error "Unable to locate webkit2gtk package"
    show_manual_installation_guide
    return 1
  fi

  log_info "Found package: $WEBKIT_PKG"

  if [ "$CHECK_ONLY" = true ]; then
    log_success "Check complete: Would install $WEBKIT_PKG"
    return 0
  fi

  log_info "Installing webkit2gtk ($WEBKIT_PKG)..."

  if sudo pacman -S --noconfirm "$WEBKIT_PKG"; then
    log_success "webkit2gtk installed successfully"
    return 0
  else
    log_error "Failed to install webkit2gtk"
    show_manual_installation_guide
    return 1
  fi
}

# Show manual installation guide when automatic installation fails
show_manual_installation_guide() {
  log_section "Manual webkit2gtk Installation Required"

  cat << EOF
Automatic installation failed or package not found. Please install webkit2gtk manually:

${YELLOW}Step 1: Update package lists${NC}
  sudo apt update

${YELLOW}Step 2: Search for available packages${NC}
  apt search webkit2gtk

${YELLOW}Step 3: Install the appropriate package${NC}
  Try these package names in order:
  - webkit2gtk-4.1 (preferred)
  - webkit2gtk-4.0 (fallback)
  - libwebkit2gtk-4.1-dev (development package)

  Example commands:
    sudo apt install webkit2gtk-4.1
    sudo apt install webkit2gtk-4.1 libwebkit2gtk-4.1-dev

${YELLOW}Step 4: Verify installation${NC}
  pkg-config --exists webkit2gtk-4.1

${YELLOW}Step 5: After manual installation, run setup again${NC}
  ./scripts/setup-dev.sh --skip-system-deps
EOF
}

# Install npm dependencies
install_npm_deps() {
  log_section "Installing npm Dependencies"

  # Navigate to app directory
  SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
  PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
  APP_DIR="$PROJECT_ROOT/crates/libretune-app"

  if [[ ! -d "$APP_DIR" ]]; then
    log_error "Application directory not found: $APP_DIR"
    exit 1
  fi

  if [[ ! -f "$APP_DIR/package.json" ]]; then
    log_error "package.json not found: $APP_DIR/package.json"
    exit 1
  fi

  # Make sure Node.js and npm are available
  if ! command_exists node || ! command_exists npm; then
    log_error "Node.js or npm not found in PATH"
    log_info "Make sure you've sourced your shell profile after setup"
    exit 1
  fi

  log_info "Installing npm packages in: $APP_DIR"
  cd "$APP_DIR"

  if npm install; then
    log_success "npm dependencies installed successfully"
  else
    log_error "Failed to install npm dependencies"
    exit 1
  fi
}

# Display environment summary
display_summary() {
  log_section "Environment Summary"

  echo "Detected distribution: $DISTRO_NAME ($DISTRO)"
  echo ""

  if command_exists cargo; then
    log_success "Rust/Cargo: $(cargo --version)"
  else
    log_warning "Rust/Cargo: Not found"
  fi

  if command_exists node; then
    log_success "Node.js: $(node --version)"
  else
    log_warning "Node.js: Not found"
  fi

  if command_exists npm; then
    log_success "npm: $(npm --version)"
  else
    log_warning "npm: Not found"
  fi

  # Check for webkit2gtk
  if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
    log_success "webkit2gtk: Installed"
  else
    log_warning "webkit2gtk: Not found or not configured"
  fi
}

# Main execution
main() {
  # Parse command-line arguments
  while [[ $# -gt 0 ]]; do
    case $1 in
      --skip-system-deps)
        SKIP_SYSTEM_DEPS=true
        shift
        ;;
      --check-only)
        CHECK_ONLY=true
        shift
        ;;
      --help|-h)
        show_help
        exit 0
        ;;
      *)
        echo -e "${RED}Error: Unknown option '$1'${NC}"
        show_help
        exit 1
        ;;
    esac
  done

  log_section "LibreTune Development Environment Setup"

  log_info "This script will install:"
  echo "  • Rust (latest stable)"
  echo "  • Node.js (LTS 20 via nvm)"
  if [ "$SKIP_SYSTEM_DEPS" = false ]; then
    echo "  • System dependencies (webkit2gtk for Tauri)"
  fi
  echo "  • npm packages"
  echo ""
  log_warning "This may require sudo access and internet connection"
  echo ""

  read -p "Continue? (y/N): " -n 1 -r
  echo ""
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "Setup cancelled"
    exit 0
  fi

  # Detect distribution
  detect_distro

  # Install components
  install_rust
  install_nodejs

  # System dependencies
  if [ "$SKIP_SYSTEM_DEPS" = false ]; then
    if [ "$CHECK_ONLY" = true ]; then
      check_webkit_packages
      if [ $? -ne 0 ]; then
        log_error "Package check failed. Please fix package repository issues."
        exit 1
      fi
    else
      install_system_deps
      if [ $? -ne 0 ]; then
        log_error "System dependency installation failed."
        log_info "You can retry with: ./scripts/setup-dev.sh"
        log_info "Or install manually and skip with: ./scripts/setup-dev.sh --skip-system-deps"
        exit 1
      fi
    fi
  else
    log_info "Skipping system dependency installation (--skip-system-deps)"
  fi

  # Skip npm deps in check-only mode
  if [ "$CHECK_ONLY" = false ]; then
    install_npm_deps
  else
    log_info "Skipping npm dependency installation (--check-only)"
  fi

  # Display summary
  display_summary

  # If check-only, don't show complete message
  if [ "$CHECK_ONLY" = true ]; then
    log_section "Check Complete!"
    log_success "All required packages are available for installation"
    echo ""
    log_info "Run without --check-only to perform actual installation:"
    echo "  ./scripts/setup-dev.sh"
    exit 0
  fi

  # Final instructions
  log_section "Setup Complete!"

  log_success "Development environment is ready!"
  echo ""
  log_info "Next steps:"
  echo "  1. Open a new terminal to ensure environment is loaded"
  echo "  2. Run: ./scripts/tauri-dev.sh"
  echo ""

  if [[ ! -d "$HOME/.nvm" ]] || ! command_exists node; then
    log_warning "Node.js via nvm needs to be sourced in your shell"
    log_info "Add these lines to your ~/.bashrc or ~/.zshrc:"
    echo ""
    echo "  export NVM_DIR=\"\$HOME/.nvm\""
    echo "  [ -s \"\$NVM_DIR/nvm.sh\" ] && \. \"\$NVM_DIR/nvm.sh\""
    echo ""
    log_info "Then open a new terminal"
  fi
}

# Run main function
main
