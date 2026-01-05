#!/bin/bash
# LibreTune Build Artifact Cleanup
# Removes build artifacts and dependencies

set -e

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

# Show help message
show_help() {
  cat << EOF
LibreTune Build Artifact Cleanup

Usage: $0 [OPTIONS]

Options:
  --cargo        Clean Rust build artifacts (cargo clean)
  --node         Remove node_modules directory
  --dist         Remove dist directory
  --all          Clean all (cargo, node, dist) with single confirmation
  --help         Show this help message

Examples:
  $0 --cargo                    # Clean Rust artifacts only
  $0 --node                     # Remove node_modules only
  $0 --all                      # Clean everything with single confirmation
  $0 --cargo --node --dist      # Clean all with individual confirmations

Note: This will permanently remove build artifacts and dependencies.
      You'll need to rebuild or reinstall them afterwards.
EOF
}

# Cleanup flags
CLEAN_CARGO=false
CLEAN_NODE=false
CLEAN_DIST=false
CONFIRM_ALL=false

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
  log "${BLUE}═══════════════════════════════════════════════════════════${NC}"
  log "${BLUE}$1${NC}"
  log "${BLUE}═══════════════════════════════════════════════════════════${NC}"
  echo ""
}

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --cargo)
      CLEAN_CARGO=true
      shift
      ;;
    --node)
      CLEAN_NODE=true
      shift
      ;;
    --dist)
      CLEAN_DIST=true
      shift
      ;;
    --all)
      CLEAN_CARGO=true
      CLEAN_NODE=true
      CLEAN_DIST=true
      CONFIRM_ALL=true
      shift
      ;;
    --help)
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

# Get script directory and project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/crates/libretune-app"

# Confirm action
confirm_action() {
  local action="$1"
  if $CONFIRM_ALL; then
    return 0
  fi

  read -p "Remove $action? (y/N): " -n 1 -r
  echo ""
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "Skipped: $action"
    return 1
  fi
  return 0
}

# Clean Rust artifacts
clean_cargo() {
  if ! command -v cargo &> /dev/null; then
    log_warning "Cargo not found. Skipping Rust cleanup."
    return 0
  fi

  if [[ ! -d "$PROJECT_ROOT/target" ]]; then
    log_warning "No Rust build artifacts found (target/ directory missing)"
    return 0
  fi

  if confirm_action "Rust build artifacts (target/)"; then
    log_info "Running: cargo clean"
    cd "$PROJECT_ROOT"

    if cargo clean; then
      log_success "Rust artifacts cleaned successfully"
    else
      log_error "Failed to clean Rust artifacts"
      return 1
    fi
  fi
}

# Clean node_modules
clean_node() {
  if [[ ! -d "$APP_DIR/node_modules" ]]; then
    log_warning "No node_modules directory found"
    return 0
  fi

  if confirm_action "node_modules directory"; then
    log_info "Removing: $APP_DIR/node_modules"

    if rm -rf "$APP_DIR/node_modules"; then
      log_success "node_modules removed successfully"
      log_info "Run 'npm install' in $APP_DIR to reinstall"
    else
      log_error "Failed to remove node_modules"
      return 1
    fi
  fi
}

# Clean dist directory
clean_dist() {
  if [[ ! -d "$APP_DIR/dist" ]]; then
    log_warning "No dist directory found"
    return 0
  fi

  if confirm_action "dist directory"; then
    log_info "Removing: $APP_DIR/dist"

    if rm -rf "$APP_DIR/dist"; then
      log_success "dist removed successfully"
    else
      log_error "Failed to remove dist"
      return 1
    fi
  fi
}

# Main execution
main() {
  log_section "LibreTune Build Artifact Cleanup"

  # If no options specified, ask what to clean
  if [[ $CLEAN_CARGO == false ]] && [[ $CLEAN_NODE == false ]] && [[ $CLEAN_DIST == false ]]; then
    log_info "No cleanup options specified. What would you like to clean?"
    echo ""
    read -p "Clean all? (y/N): " -n 1 -r
    echo ""

    if [[ $REPLY =~ ^[Yy]$ ]]; then
      CLEAN_CARGO=true
      CLEAN_NODE=true
      CLEAN_DIST=true
      CONFIRM_ALL=true
    else
      echo ""
      log_info "Cleaning options:"
      echo "  1. Rust artifacts (cargo clean)"
      echo "  2. node_modules"
      echo "  3. dist directory"
      echo "  a. All of the above"
      echo "  q. Quit"
      echo ""

      read -p "Select option [1,2,3,a,q]: " -n 1 -r
      echo ""

      case $REPLY in
        1)
          CLEAN_CARGO=true
          ;;
        2)
          CLEAN_NODE=true
          ;;
        3)
          CLEAN_DIST=true
          ;;
        a|A)
          CLEAN_CARGO=true
          CLEAN_NODE=true
          CLEAN_DIST=true
          CONFIRM_ALL=true
          ;;
        q|Q)
          log_info "Cleanup cancelled"
          exit 0
          ;;
        *)
          log_error "Invalid option"
          exit 1
          ;;
      esac
    fi
  fi

  # Perform cleanup
  CLEANED=0
  if $CLEAN_CARGO; then
    clean_cargo || true
    CLEANED=$((CLEANED + 1))
  fi

  if $CLEAN_NODE; then
    clean_node || true
    CLEANED=$((CLEANED + 1))
  fi

  if $CLEAN_DIST; then
    clean_dist || true
    CLEANED=$((CLEANED + 1))
  fi

  # Final summary
  if [[ $CLEANED -gt 0 ]]; then
    log_section "Cleanup Complete!"

    log_success "Cleaned $CLEANED item(s)"
    echo ""
    log_info "Next steps:"
    echo "  • If you cleaned Rust artifacts: They will be rebuilt on next compile"
    echo "  • If you cleaned node_modules: Run 'npm install' in $APP_DIR"
    echo "  • If you cleaned dist: Run 'npm run build' to rebuild"
  else
    log_info "Nothing to clean"
  fi
}

# Run main function
main
