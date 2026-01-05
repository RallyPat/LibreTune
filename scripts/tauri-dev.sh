#!/bin/bash
# LibreTune Development Launcher
# Launches the Tauri app with sanitized environment to prevent WebKit crashes

set -e

# Auto-executable check - make script executable if it isn't already
# Only perform this when the script is executed directly (not when sourced)
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  if [[ ! -x "$0" ]]; then
    chmod +x "$0"
    exec "$0" "$@"
  fi
fi

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration variables
MAX_RETRIES=3           # Maximum npm/command retry attempts
RETRY_DELAY=2            # Seconds between retry attempts
SANITIZE_ENV=true        # Whether to sanitize Snap environment (default: true)
NVM_DIR="${NVM_DIR:-$HOME/.nvm}"  # NVM directory path (supports override)

# Script options
VERBOSE=false
LOG_FILE=""
CLEAN=false

# Show help message
show_help() {
  cat << EOF
LibreTune Development Launcher

Usage: $0 [OPTIONS]

Options:
  --verbose              Show detailed output from npm and Tauri
  --log-file <path>      Write output to log file with timestamps
  --clean                Run 'cargo clean' before launching
  --no-sanitize          Disable environment sanitization (advanced)
  --help                 Display this help message

Examples:
  $0                              # Basic launch
  $0 --verbose                    # Launch with detailed output
  $0 --verbose --log-file /tmp/libretune-dev.log    # Launch with logging
  $0 --clean                      # Clean build artifacts then launch
  $0 --no-sanitize                # Disable environment sanitization

Environment Options:
  By default, this script sanitizes the environment to prevent WebKit crashes
  caused by Snap-related environment variables on Linux systems.

  The script automatically detects and sources nvm (Node Version Manager) if needed.
  If Node.js was installed via nvm, it will be loaded automatically.

  Retry behavior:
  The script will attempt to launch up to 3 times if npm is not found,
  with a 2-second delay between retries. During retries, verbose debugging
  is enabled (set -x) to help diagnose issues.

Environment variables preserved:
  PATH, HOME, DISPLAY, XAUTHORITY, XDG_RUNTIME_DIR, TERM, NVM_DIR
  Plus: USER, SHELL, LANG (if set)

Environment variables excluded when sanitizing:
  All SNAP_* variables (SNAP, SNAP_ARCH, SNAP_DATA, etc.)

Advanced usage:
  If you need to disable environment sanitization (e.g., your environment
  doesn't have Snap variables but sanitization breaks something), use the
  --no-sanitize flag.

  NVM directory can be overridden via environment variable:
    export NVM_DIR="/custom/path/to/nvm"
    $0
EOF
}

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --verbose)
      VERBOSE=true
      shift
      ;;
    --log-file)
      LOG_FILE="$2"
      shift 2
      ;;
    --clean)
      CLEAN=true
      shift
      ;;
    --no-sanitize)
      SANITIZE_ENV=false
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

# Logging function - outputs to console and optionally to log file
log() {
  local msg="$1"
  echo -e "$msg"
  if [[ -n "$LOG_FILE" ]]; then
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $msg" >> "$LOG_FILE"
  fi
}

# Log error message
log_error() {
  log "${RED}$1${NC}"
}

# Log success message
log_success() {
  log "${GREEN}$1${NC}"
}

# Log warning message
log_warning() {
  log "${YELLOW}$1${NC}"
}

# Log info message
log_info() {
  log "${BLUE}$1${NC}"
}

# Check if command exists
command_exists() {
  command -v "$1" &> /dev/null
}

# Check if cargo is installed
check_cargo() {
  if ! command_exists cargo; then
    log_error "Rust/Cargo not found!"
    log_info "To install Rust, run:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    echo "  source \$HOME/.cargo/env"
    log_info "Or run: ./scripts/setup-dev.sh"
    return 1
  fi
  return 0
}

# Check if node is installed
check_node() {
  if ! command_exists node; then
    log_error "Node.js not found!"
    log_info "To install Node.js, run: ./scripts/setup-dev.sh"
    return 1
  fi
  return 0
}

# Check if npm is installed
check_npm() {
  if ! command_exists npm; then
    log_error "npm not found!"
    log_info "To install Node.js and npm, run: ./scripts/setup-dev.sh"
    return 1
  fi
  return 0
}

# Get script directory and project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/crates/libretune-app"

# Verify directory structure
if [[ ! -d "$APP_DIR" ]]; then
  log_error "Application directory not found: $APP_DIR"
  log_info "Please run this script from the LibreTune project root"
  exit 1
fi

if [[ ! -f "$APP_DIR/package.json" ]]; then
  log_error "package.json not found: $APP_DIR/package.json"
  log_info "Did you run 'npm install' in the app directory?"
  exit 1
fi

# Helper: Check nvm installation
check_nvm_installation() {
  local nvm_dir="${NVM_DIR:-$HOME/.nvm}"

  if [[ ! -d "$nvm_dir" ]]; then
    log_warning "nvm directory not found: $nvm_dir" >&2
    echo "not-found"
    return
  fi

  if [[ ! -f "$nvm_dir/nvm.sh" ]]; then
    log_warning "nvm.sh not found: $nvm_dir/nvm.sh" >&2
    echo "not-found"
    return
  fi

  if command -v nvm &>/dev/null; then
    log_success "nvm is already loaded in current shell" >&2
    echo "loaded"
    return
  fi

  log_info "nvm found but not loaded: $nvm_dir" >&2
  echo "installed"
}

# Helper: Get nvm path
get_nvm_path() {
  echo "${NVM_DIR:-$HOME/.nvm}"
}

# Helper: Detect Snap environment
detect_snap_environment() {
  local snap_vars=""
  # Collect any Snap environment variables
  for var in SNAP SNAP_ARCH SNAP_DATA SNAP_COMMON SNAP_INSTANCE_NAME \
              SNAP_LIBRARY_PATH SNAP_REVISION SNAP_VERSION \
              SNAP_NAME SNAP_COOKIE_DIR; do
    if [[ -n "${!var+x}" ]]; then
      snap_vars="$snap_vars $var"
    fi
  done

  if [[ -n "$snap_vars" ]]; then
    log_warning "Snap environment detected" >&2
    log_info "Snap variables set: $snap_vars" >&2
    log_info "Will sanitize environment to prevent WebKit crashes" >&2
    echo "snap"
  else
    log_info "No Snap environment detected - using normal environment" >&2
    echo "no-snap"
  fi
}

# Helper: Build sanitized environment
build_sanitized_env() {
  local sanitize="$1"

  # Attempt to include nvm's node bin and cargo bin in the sanitized PATH
  local nvm_path="$(get_nvm_path)"
  local node_bin=""
  
  # Strategy 1: Use currently active node binary's directory (most reliable)
  if command -v node &>/dev/null; then
    local current_node_path
    current_node_path=$(command -v node 2>/dev/null || true)
    if [[ -n "$current_node_path" ]]; then
      node_bin=$(dirname "$current_node_path")
    fi
  fi
  
  # Strategy 2: Fall back to nvm versions directory (pick latest with sort -V)
  if [[ -z "$node_bin" && -d "$nvm_path/versions/node" ]]; then
    local node_ver
    node_ver=$(ls -1 "$nvm_path/versions/node" 2>/dev/null | sort -V | tail -1 || true)
    if [[ -n "$node_ver" && -d "$nvm_path/versions/node/$node_ver/bin" ]]; then
      node_bin="$nvm_path/versions/node/$node_ver/bin"
    fi
  fi
  
  # Strategy 3: Check common global install locations
  if [[ -z "$node_bin" ]]; then
    for try_path in /usr/local/bin /usr/bin; do
      if [[ -x "$try_path/node" && -x "$try_path/npm" ]]; then
        node_bin="$try_path"
        break
      fi
    done
  fi
  
  local cargo_bin="$HOME/.cargo/bin"

  local path_prefix=""
  if [[ -n "$node_bin" ]]; then
    path_prefix="$node_bin"
  fi
  if [[ -d "$cargo_bin" ]]; then
    if [[ -n "$path_prefix" ]]; then
      path_prefix="$path_prefix:$cargo_bin"
    else
      path_prefix="$cargo_bin"
    fi
  fi

  if [[ -n "$path_prefix" ]]; then
    local env_vars="PATH=$path_prefix:$PATH"
  else
    local env_vars="PATH=$PATH"
  fi

  # Always preserve critical variables
  env_vars="$env_vars HOME=$HOME"
  env_vars="$env_vars DISPLAY=$DISPLAY"
  env_vars="$env_vars XAUTHORITY=$XAUTHORITY"
  env_vars="$env_vars XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR"
  env_vars="$env_vars TERM=$TERM"
  env_vars="$env_vars NVM_DIR=$(get_nvm_path)"

  # If verbose, log what will be in the sanitized PATH
  if $VERBOSE; then
    if [[ -n "$path_prefix" ]]; then
      log_info "Sanitized PATH prefix: $path_prefix" >&2
    else
      log_info "Sanitized PATH will be the current PATH (no nvm/cargo bins discovered)" >&2
    fi
  fi

  # Preserve Rust/Cargo environment (required for rustup to find toolchain)
  # Set defaults if not already set (rustup uses ~/.rustup and ~/.cargo by default)
  local rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  env_vars="$env_vars RUSTUP_HOME=$rustup_home"
  env_vars="$env_vars CARGO_HOME=$cargo_home"
  # Always set stable toolchain to ensure rustup can find cargo
  env_vars="$env_vars RUSTUP_TOOLCHAIN=stable"

  # Preserve pkg-config path for finding system libraries (libudev, etc.)
  if [[ -n "$PKG_CONFIG_PATH" ]]; then
    env_vars="$env_vars PKG_CONFIG_PATH=$PKG_CONFIG_PATH"
  fi

  # Preserve other useful variables
  if [[ -n "$USER" ]]; then
    env_vars="$env_vars USER=\"$USER\""
  fi

  if [[ -n "$SHELL" ]]; then
    env_vars="$env_vars SHELL=\"$SHELL\""
  fi

  if [[ -n "$LANG" ]]; then
    env_vars="$env_vars LANG=\"$LANG\""
  fi

  # If sanitizing (mode 'sanitized' or legacy 'true'), exclude Snap variables by not adding them
  if [[ "$sanitize" == "true" || "$sanitize" == "sanitized" ]]; then
    log_info "Excluding Snap environment variables from new environment" >&2
  else
    # Keep all environment variables
    local all_vars=""
    for var in $(env | awk -F= '{print $1}'); do
      case "$var" in
        SNAP*|snap*)
          # Skip Snap variables
          ;;
        *)
          # Preserve other variables
          all_vars="$all_vars $var=\${!var}"
          ;;
      esac
    done
    env_vars="$all_vars $env_vars"
  fi

  echo "$env_vars"
}

# Helper: Show npm fix guide
show_npm_fix_guide() {
  local nvm_status="$1"
  local env_mode="$2"

  log_section "Node.js/npm Not Available"
  cat << EOF
${RED}The development server requires Node.js and npm.${NC}
${RED}These were installed via nvm but are not available in the current shell.${NC}

${YELLOW}═════════════════════════════════════════════════════${NC}

${BLUE}Solution 1: Open a new terminal${NC}
  This is the easiest solution. Open a new terminal and run:
  ${GREEN}cd "$APP_DIR" && npm run tauri dev${NC}

${BLUE}Solution 2: Source nvm in current terminal${NC}
  Run the following commands:
  ${GREEN}export NVM_DIR="$HOME/.nvm"${NC}
  ${GREEN}[ -s "\$NVM_DIR/nvm.sh" ] && . "\$NVM_DIR/nvm.sh"${NC}
  ${GREEN}cd "$APP_DIR" && npm run tauri dev${NC}

${BLUE}Solution 3: Add to ~/.bashrc (permanent)${NC}
  Permanently enable nvm by adding these lines to ~/.bashrc:
  ${YELLOW}export NVM_DIR="\$HOME/.nvm"${NC}
  ${YELLOW}[ -s "\$NVM_DIR/nvm.sh" ] && \. "\$NVM_DIR/nvm.sh"${NC}
  Then run: ${GREEN}source ~/.bashrc${NC}

${BLUE}Solution 4: Check nvm installation${NC}
EOF

  # Show nvm status
  local nvm_path=$(get_nvm_path)

  if [[ "$nvm_status" == "not-found" ]]; then
    cat << EOF
${RED}nvm was not found at expected location!${NC}
  Expected location: ${YELLOW}$nvm_path${NC}
  If nvm is installed elsewhere, set NVM_DIR:
  ${GREEN}export NVM_DIR="/path/to/nvm"${NC}
EOF
  elif [[ "$nvm_status" == "installed" ]]; then
    cat << EOF
${YELLOW}nvm is installed but not loaded in the current shell.${NC}
  The script attempted to source nvm automatically but failed.
  This may indicate a shell compatibility issue.
EOF
  fi

  # Show environment information
  cat << EOF
${YELLOW}═════════════════════════════════════════════════════${NC}

${BLUE}Environment Information:${NC}
  Environment mode: $env_mode
  NVM_DIR: ${NVM_DIR:-not set}
  NVM directory exists: $([[ -d "${NVM_DIR:-$HOME/.nvm}" ]] && echo "yes" || echo "no")
  nvm.sh exists: $([[ -f "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]] && echo "yes" || echo "no")
  Node.js in PATH: $(command -v node &>/dev/null && node --version || echo "no")
  npm in PATH: $(command -v npm &>/dev/null && npm --version || echo "no")

${YELLOW}═════════════════════════════════════════════════════${NC}

${BLUE}Press Enter to exit...${NC}
EOF
  read
}

# Helper: Verify Node.js and npm availability
verify_node_npm() {
  local nvm_path=$(get_nvm_path)

  log_info "Checking Node.js and npm availability..."

  # Check if nvm is available
  if [[ -d "$nvm_path" ]]; then
    log_success "nvm directory found: $nvm_path"
  else
    log_warning "nvm directory not found: $nvm_path"
    return 1
  fi

  # Check if Node.js is in PATH
  if command -v node &>/dev/null; then
    log_success "Node.js found: $(node --version)"
  else
    log_warning "Node.js not in PATH"
  fi

  # Check if npm is in PATH
  if command -v npm &>/dev/null; then
    log_success "npm found: $(npm --version)"
    return 0
  else
    log_warning "npm not in PATH"
    return 1
  fi
}

# Helper: Log section header
log_section() {
  echo ""
  log "${BLUE}═════════════════════════════════════════════════════════${NC}"
  log "${BLUE}$1${NC}"
  log "${BLUE}═════════════════════════════════════════════════════════${NC}"
  echo ""
}

# Helper: Launch dev server with retry logic
launch_dev_server_with_retry() {
  local retry_count=0

  while [[ $retry_count -lt $MAX_RETRIES ]]; do
    retry_count=$((retry_count + 1))

    log_info "Launch attempt $retry_count/$MAX_RETRIES..."

    # Build launch command with an inner-shell fallback that sources nvm and ensures cargo bin is in PATH
    local nvm_path=$(get_nvm_path)
    
    # Detect node bin for explicit PATH injection as ultimate fallback
    local detected_node_bin=""
    if [[ -d "$nvm_path/versions/node" ]]; then
      local latest_ver
      latest_ver=$(ls -1 "$nvm_path/versions/node" 2>/dev/null | sort -V | tail -1 || true)
      if [[ -n "$latest_ver" && -d "$nvm_path/versions/node/$latest_ver/bin" ]]; then
        detected_node_bin="$nvm_path/versions/node/$latest_ver/bin"
      fi
    fi
    
    # Inner prelude: source nvm, try nvm use, then explicit PATH fallback
    local inner_prelude="export NVM_DIR='$nvm_path'"
    inner_prelude="$inner_prelude; [ -s \"\$NVM_DIR/nvm.sh\" ] && . \"\$NVM_DIR/nvm.sh\" || true"
    inner_prelude="$inner_prelude; command -v nvm &>/dev/null && { nvm use default --silent 2>/dev/null || nvm use node --silent 2>/dev/null || true; }"
    if [[ -n "$detected_node_bin" ]]; then
      inner_prelude="$inner_prelude; if ! command -v npm &>/dev/null; then export PATH='$detected_node_bin':\$PATH; fi"
    fi
    inner_prelude="$inner_prelude; if [[ -d \"$HOME/.cargo/bin\" ]]; then export PATH=\"$HOME/.cargo/bin:\$PATH\"; fi"
    
    log_info "Inner-shell fallback will source nvm and try 'nvm use default'"
    local launch_cmd="$inner_prelude; cd '$APP_DIR' && npm run tauri dev"

    # Prepare environment
    local sanitized_env=$(build_sanitized_env "$ENV_MODE")

    # Check if dev port is already in use (provide clearer diagnostics)
    local dev_port=1420
    if ss -ltnp 2>/dev/null | grep -q ":${dev_port} \|:${dev_port}\b"; then
      local holder
      holder=$(ss -ltnp 2>/dev/null | grep ":${dev_port} \|:${dev_port}\b" | awk '{print $NF}' | head -1 || true)
      log_error "Port ${dev_port} is already in use: ${holder}"
      log_info "Run 'lsof -i :${dev_port}' to inspect or kill the process if appropriate"
      return 1
    fi

    # Pre-flight check: verify npm is available in sanitized shell
    if [[ "$ENV_MODE" == "sanitized" ]]; then
      local preflight_check
      preflight_check=$(env -i $sanitized_env bash -lc "$inner_prelude; command -v npm || echo __NPM_NOT_FOUND__" 2>/dev/null)
      if [[ "$preflight_check" == *"__NPM_NOT_FOUND__"* ]]; then
        log_warning "Pre-flight check: npm not found in sanitized shell, attempting direct PATH injection..."
        # Try to find npm and inject its directory
        local npm_path
        npm_path=$(command -v npm 2>/dev/null || true)
        if [[ -n "$npm_path" ]]; then
          local npm_dir
          npm_dir=$(dirname "$npm_path")
          sanitized_env="PATH=$npm_dir:\${PATH#PATH=} $sanitized_env"
          log_info "Injected $npm_dir into sanitized PATH"
        fi
      fi
    fi

    # Execute command
    local exit_code

    if [[ "$ENV_MODE" == "sanitized" ]]; then
      if $VERBOSE; then
        set -x
        env -i $sanitized_env bash -lc "$launch_cmd"
        local exit_code=$?
        set +x
      else
        env -i $sanitized_env bash -lc "$launch_cmd"
        local exit_code=$?
      fi
    else
      if $VERBOSE; then
        set -x
        bash -lc "$launch_cmd"
        local exit_code=$?
        set +x
      else
        bash -lc "$launch_cmd"
        local exit_code=$?
      fi
    fi

    # Check if command was not found
    if [[ $exit_code -eq 127 ]]; then
      log_error "Command not found (exit code 127)"

      if [[ $retry_count -lt $MAX_RETRIES ]]; then
        log_warning "Retrying in $RETRY_DELAY seconds..."
        sleep $RETRY_DELAY

        # Re-verify after delay
        NVM_STATUS=$(check_nvm_installation)
        continue
      fi

      # Max retries reached
      log_error "Max retries ($MAX_RETRIES) reached"
      log_error "Unable to launch development server"
      show_npm_fix_guide "$NVM_STATUS" "$ENV_MODE"
      exit 1

    elif [[ $exit_code -ne 0 ]]; then
      log_error "Development server exited with code: $exit_code"
      log_info "Check the logs above for error details"
      exit $exit_code
    else
      # Success - server is running
      log_success "Development server started successfully"
      return 0
    fi
  done
}

# Main execution
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  # 1. Detect environment
  SNAP_MODE=$(detect_snap_environment)

  # 2. Check nvm installation
  NVM_STATUS=$(check_nvm_installation)

# 2b. Source nvm if installed but not loaded (so PATH includes node/npm)
if [[ "$NVM_STATUS" == "installed" ]]; then
  nvm_path="$(get_nvm_path)"
  if [[ -f "$nvm_path/nvm.sh" ]]; then
    log_info "Sourcing nvm.sh to add node/npm to PATH..."
    source "$nvm_path/nvm.sh"
    NVM_STATUS="loaded"
  fi
fi

# 2c. Ensure cargo is in PATH
if [[ -d "$HOME/.cargo/bin" ]] && [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
  log_info "Added ~/.cargo/bin to PATH"
fi

# 3. Verify prerequisites (non-fatal, just warnings)
if ! verify_node_npm; then
  log_warning "Node.js/npm verification warning - attempting to fix..."
fi

# 4. Determine environment mode
if [[ "$SANITIZE_ENV" == "false" ]]; then
  ENV_MODE="normal"
  log_info "Environment sanitization disabled (--no-sanitize)"
elif [[ "$SNAP_MODE" == "snap" ]]; then
  ENV_MODE="sanitized"
else
  # No Snap detected, but still sanitize if default
  if [[ "$SANITIZE_ENV" == "true" ]]; then
    ENV_MODE="sanitized"
  else
    ENV_MODE="normal"
  fi
fi

# Display startup banner
log_section "LibreTune Development Launcher"
log_info "Project directory: $PROJECT_ROOT"
log_info "App directory: $APP_DIR"
log_info "Environment mode: $ENV_MODE"
log_info "NVM status: $NVM_STATUS"
echo ""

# Clean if requested
if $CLEAN; then
  log_info "🧹 Cleaning build artifacts..."
  cd "$PROJECT_ROOT"
  if $VERBOSE; then
    cargo clean
  else
    cargo clean > /dev/null 2>&1
  fi
  log_success "Build artifacts cleaned"
  echo ""
fi

# 5. Display versions if verbose
if $VERBOSE; then
  log_info "Environment check:"
  echo "  Rust/Cargo: $(cargo --version)"
  if command -v node &>/dev/null; then
    echo "  Node.js: $(node --version)"
  fi
  if command -v npm &>/dev/null; then
    echo "  npm: $(npm --version)"
  fi
  echo ""
fi

# 6. Launch dev server with retry logic
launch_dev_server_with_retry
launch_result=$?

# Handle result
if [[ $launch_result -eq 0 ]]; then
  log_success "LibreTune development server started successfully"
else
  log_error "Failed to start development server (exit code: $launch_result)"

  # Only show fix guide if it was npm-related
  if [[ $launch_result -eq 127 ]]; then
    show_npm_fix_guide "$NVM_STATUS" "$ENV_MODE"
    exit 1
  fi

  exit $launch_result
fi

fi
