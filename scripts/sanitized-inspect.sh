#!/usr/bin/env bash
# LibreTune Sanitized Environment Inspector
# Use this script to diagnose issues with the tauri-dev.sh sanitized shell
# It prints PATH, NVM_DIR, and node/npm availability inside the sanitized environment

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Define helper functions directly (avoid sourcing tauri-dev.sh which has set -e)
get_nvm_path() {
  echo "${NVM_DIR:-$HOME/.nvm}"
}

build_sanitized_env() {
  local nvm_path="$(get_nvm_path)"
  local node_bin=""
  
  # Strategy 1: Use currently active node binary's directory
  if command -v node &>/dev/null; then
    local current_node_path
    current_node_path=$(command -v node 2>/dev/null || true)
    if [[ -n "$current_node_path" ]]; then
      node_bin=$(dirname "$current_node_path")
    fi
  fi
  
  # Strategy 2: Fall back to nvm versions directory
  if [[ -z "$node_bin" && -d "$nvm_path/versions/node" ]]; then
    local node_ver
    node_ver=$(ls -1 "$nvm_path/versions/node" 2>/dev/null | sort -V | tail -1 || true)
    if [[ -n "$node_ver" && -d "$nvm_path/versions/node/$node_ver/bin" ]]; then
      node_bin="$nvm_path/versions/node/$node_ver/bin"
    fi
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

  local env_vars=""
  if [[ -n "$path_prefix" ]]; then
    env_vars="PATH=$path_prefix:$PATH"
  else
    env_vars="PATH=$PATH"
  fi

  env_vars="$env_vars HOME=$HOME"
  env_vars="$env_vars DISPLAY=${DISPLAY:-}"
  env_vars="$env_vars TERM=${TERM:-xterm}"
  env_vars="$env_vars NVM_DIR=$nvm_path"
  env_vars="$env_vars RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}"
  env_vars="$env_vars CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}"
  env_vars="$env_vars RUSTUP_TOOLCHAIN=stable"

  echo "$env_vars"
}

echo "========================================"
echo "LibreTune Sanitized Environment Inspector"
echo "========================================"
echo ""

echo "--- Parent shell environment ---"
echo "NVM_DIR=${NVM_DIR:-<not set>}"
echo "PATH=$PATH"
echo "node: $(command -v node 2>/dev/null || echo '<not found>')"
if command -v node &>/dev/null; then
  echo "node --version: $(node --version 2>&1)"
fi
echo "npm: $(command -v npm 2>/dev/null || echo '<not found>')"
if command -v npm &>/dev/null; then
  echo "npm --version: $(npm --version 2>&1)"
fi
echo ""

# Build sanitized env
echo "--- Building sanitized environment ---"
SAN_ENV=$(build_sanitized_env sanitized 2>/dev/null || echo "PATH=$PATH HOME=$HOME NVM_DIR=${NVM_DIR:-$HOME/.nvm}")
echo "Sanitized env string (truncated to 500 chars):"
echo "${SAN_ENV:0:500}..."
echo ""

# Get nvm path
NVM_PATH=$(get_nvm_path 2>/dev/null || echo "${NVM_DIR:-$HOME/.nvm}")
echo "NVM_PATH: $NVM_PATH"
echo ""

# Build inner prelude (same as tauri-dev.sh)
INNER_PRELUDE="export NVM_DIR='$NVM_PATH'"
INNER_PRELUDE="$INNER_PRELUDE; [ -s \"\$NVM_DIR/nvm.sh\" ] && . \"\$NVM_DIR/nvm.sh\" || true"
INNER_PRELUDE="$INNER_PRELUDE; command -v nvm &>/dev/null && { nvm use default --silent 2>/dev/null || nvm use node --silent 2>/dev/null || true; }"
INNER_PRELUDE="$INNER_PRELUDE; if [[ -d \"$HOME/.cargo/bin\" ]]; then export PATH=\"$HOME/.cargo/bin:\$PATH\"; fi"

echo "--- Inner prelude ---"
echo "$INNER_PRELUDE"
echo ""

echo "--- Sanitized shell inspection ---"
echo "Running: env -i \$SAN_ENV bash -lc '<inner_prelude>; <diagnostics>'"
echo ""

# Run inspection inside sanitized shell
env -i $SAN_ENV bash -lc "$INNER_PRELUDE; echo 'NVM_DIR='\$NVM_DIR; echo 'PATH='\$PATH; echo 'node: '\$(command -v node 2>/dev/null || echo '<not found>'); command -v node &>/dev/null && node --version 2>&1 || true; echo 'npm: '\$(command -v npm 2>/dev/null || echo '<not found>'); command -v npm &>/dev/null && npm --version 2>&1 || true" 2>&1 || {
  echo ""
  echo "!!! Sanitized shell failed with exit code $? !!!"
}

echo ""
echo "========================================"
echo "Inspection complete"
echo "========================================"
