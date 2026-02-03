#!/usr/bin/env bash
#
# pre-push.sh - Local CI checks before pushing to GitHub
# Mirrors the GitHub Actions CI workflow to catch failures before they reach CI
#
# Usage: ./scripts/pre-push.sh
# Or install as Git hook: ./scripts/pre-push.sh --install-hook

set -e  # Exit on first error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track overall status
FAILED=0

print_header() {
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
    FAILED=1
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# Install as Git pre-push hook
if [[ "$1" == "--install-hook" ]]; then
    HOOK_PATH=".git/hooks/pre-push"
    cat > "$HOOK_PATH" << 'EOF'
#!/usr/bin/env bash
# Auto-installed pre-push hook - runs local CI checks before pushing

echo "Running pre-push checks..."
./scripts/pre-push.sh

if [ $? -ne 0 ]; then
    echo ""
    echo "❌ Pre-push checks failed! Push aborted."
    echo "Fix the errors above or use 'git push --no-verify' to skip checks."
    exit 1
fi
EOF
    chmod +x "$HOOK_PATH"
    print_success "Installed Git pre-push hook at $HOOK_PATH"
    exit 0
fi

# Main test sequence
echo ""
print_header "🚀 Running Local CI Checks (mirrors GitHub Actions)"
echo ""

#
# Job 1: Rust Check & Test
#
print_header "📦 Rust Check & Test"

echo "→ Building workspace..."
if cargo build --workspace 2>&1 | tee /tmp/cargo-build.log; then
    print_success "Cargo build passed"
else
    print_error "Cargo build failed"
    cat /tmp/cargo-build.log
fi

echo ""
echo "→ Running Rust tests..."
if cargo test --workspace --quiet 2>&1; then
    print_success "Cargo tests passed"
else
    print_error "Cargo tests failed"
fi

echo ""
echo "→ Verifying build info format..."
if cargo test --package libretune-app --test build_info -- --nocapture 2>&1 | tail -5; then
    print_success "Build info test passed"
else
    print_error "Build info test failed"
fi

echo ""
echo "→ Running Clippy (linter)..."
if cargo clippy --workspace -- -D warnings 2>&1 | tee /tmp/clippy.log; then
    print_success "Clippy passed (no warnings)"
else
    print_warning "Clippy found issues (non-blocking in CI, but please fix)"
    tail -20 /tmp/clippy.log
fi

echo ""

#
# Job 2: Frontend Check
#
print_header "🎨 Frontend Check"

cd crates/libretune-app

echo "→ Installing npm dependencies..."
if npm ci --quiet 2>&1; then
    print_success "npm ci passed"
else
    print_error "npm ci failed"
fi

echo ""
echo "→ TypeScript type checking..."
if npx tsc --noEmit 2>&1 | tee /tmp/tsc.log; then
    print_success "TypeScript check passed"
else
    print_error "TypeScript check failed"
    tail -30 /tmp/tsc.log
fi

echo ""
echo "→ Building frontend..."
if npm run build 2>&1 | tail -10; then
    print_success "Frontend build passed"
else
    print_error "Frontend build failed"
fi

echo ""
echo "→ Running frontend unit tests..."
if npm run test:run 2>&1; then
    print_success "Frontend tests passed"
else
    print_error "Frontend tests failed"
fi

cd ../..

echo ""

#
# Job 3: Format Check
#
print_header "✨ Format Check"

echo "→ Checking Rust formatting..."
if cargo fmt --all -- --check 2>&1; then
    print_success "Rust formatting passed"
else
    print_error "Rust formatting failed - run 'cargo fmt --all' to fix"
fi

echo ""
print_header "📊 Summary"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}✓ All checks passed! Safe to push.${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    exit 0
else
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}✗ Some checks failed! Please fix before pushing.${NC}"
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "To push anyway (not recommended): git push --no-verify"
    exit 1
fi
