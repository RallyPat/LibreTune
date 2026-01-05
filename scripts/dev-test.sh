#!/bin/bash
# LibreTune Test Runner
# Runs all tests and displays summary

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

# Test counters
RUST_PASSED=0
RUST_FAILED=0
TS_PASSED=0
TS_FAILED=0

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

# Get script directory and project root
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Change to project root
cd "$PROJECT_ROOT"

# Run Rust tests
run_rust_tests() {
  log_section "Running Rust Tests"

  if ! command -v cargo &> /dev/null; then
    log_error "Cargo not found. Please install Rust first."
    return 1
  fi

  log_info "Running: cargo test"
  echo ""

  # Run cargo test and capture output
  if cargo test 2>&1; then
    log_success "All Rust tests passed!"
    RUST_PASSED=1
  else
    log_error "Some Rust tests failed"
    RUST_FAILED=1
  fi
}

# Run TypeScript type checking
run_type_check() {
  log_section "TypeScript Type Checking"

  APP_DIR="$PROJECT_ROOT/crates/libretune-app"

  if [[ ! -d "$APP_DIR" ]]; then
    log_warning "App directory not found: $APP_DIR"
    log_warning "Skipping TypeScript checks"
    return 0
  fi

  if ! command -v npx &> /dev/null; then
    log_error "npx not found. Please install Node.js first."
    return 1
  fi

  if [[ ! -f "$APP_DIR/package.json" ]]; then
    log_warning "package.json not found: $APP_DIR/package.json"
    log_warning "Skipping TypeScript checks"
    return 0
  fi

  cd "$APP_DIR"

  log_info "Running: npm run typecheck (or npx tsc --noEmit)"
  echo ""

  # Try npm run typecheck first, fallback to npx tsc
  if npm run typecheck 2>/dev/null; then
    log_success "TypeScript type checking passed!"
    TS_PASSED=1
  elif npx tsc --noEmit 2>&1; then
    log_success "TypeScript type checking passed!"
    TS_PASSED=1
  else
    log_error "TypeScript type checking failed"
    TS_FAILED=1
  fi

  cd "$PROJECT_ROOT"
}

# Display test summary
display_summary() {
  log_section "Test Summary"

  TOTAL_RUN=0
  TOTAL_PASSED=0
  TOTAL_FAILED=0

  # Rust tests
  if [[ $RUST_PASSED -eq 1 ]]; then
    log_success "Rust tests: PASSED"
    TOTAL_RUN=$((TOTAL_RUN + 1))
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
  elif [[ $RUST_FAILED -eq 1 ]]; then
    log_error "Rust tests: FAILED"
    TOTAL_RUN=$((TOTAL_RUN + 1))
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
  else
    log_warning "Rust tests: NOT RUN"
  fi

  # TypeScript checks
  if [[ $TS_PASSED -eq 1 ]]; then
    log_success "TypeScript checks: PASSED"
    TOTAL_RUN=$((TOTAL_RUN + 1))
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
  elif [[ $TS_FAILED -eq 1 ]]; then
    log_error "TypeScript checks: FAILED"
    TOTAL_RUN=$((TOTAL_RUN + 1))
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
  else
    log_warning "TypeScript checks: NOT RUN"
  fi

  echo ""
  log_info "Total test suites run: $TOTAL_RUN"
  log_info "Passed: $TOTAL_PASSED"
  if [[ $TOTAL_FAILED -gt 0 ]]; then
    log_error "Failed: $TOTAL_FAILED"
  else
    log_info "Failed: 0"
  fi

  # Exit with error code if any tests failed
  if [[ $TOTAL_FAILED -gt 0 ]]; then
    exit 1
  fi
}

# Main execution
main() {
  log_section "LibreTune Test Runner"

  # Run tests
  run_rust_tests
  run_type_check

  # Display summary
  display_summary
}

# Run main function
main
