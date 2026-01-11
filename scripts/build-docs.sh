#!/bin/bash
# LibreTune Documentation Build Script
# Generates all documentation: Rust API, TypeScript API, and User Manual

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  LibreTune Documentation Builder       ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# Parse arguments
BUILD_RUST=true
BUILD_FRONTEND=true
BUILD_USER=true
SERVE=false
OPEN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --rust-only)
            BUILD_FRONTEND=false
            BUILD_USER=false
            shift
            ;;
        --frontend-only)
            BUILD_RUST=false
            BUILD_USER=false
            shift
            ;;
        --user-only)
            BUILD_RUST=false
            BUILD_FRONTEND=false
            shift
            ;;
        --serve)
            SERVE=true
            shift
            ;;
        --open)
            OPEN=true
            shift
            ;;
        --help)
            echo "Usage: build-docs.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --rust-only      Build only Rust API documentation"
            echo "  --frontend-only  Build only TypeScript/React documentation"
            echo "  --user-only      Build only user manual (mdBook)"
            echo "  --serve          Serve documentation locally after building"
            echo "  --open           Open documentation in browser after building"
            echo "  --help           Show this help message"
            echo ""
            echo "Output locations:"
            echo "  Rust API:      target/doc/"
            echo "  Frontend API:  docs/api/"
            echo "  User Manual:   docs/book/"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

cd "$PROJECT_ROOT"

# Build Rust API documentation
if [ "$BUILD_RUST" = true ]; then
    echo -e "${YELLOW}📦 Building Rust API documentation...${NC}"
    
    if command -v cargo &> /dev/null; then
        cargo doc --no-deps --document-private-items -p libretune-core 2>&1 | tail -5
        echo -e "${GREEN}✓ Rust docs: target/doc/libretune_core/index.html${NC}"
    else
        echo -e "${RED}✗ Cargo not found. Install Rust to build Rust documentation.${NC}"
    fi
    echo ""
fi

# Build TypeScript/React documentation
if [ "$BUILD_FRONTEND" = true ]; then
    echo -e "${YELLOW}📦 Building TypeScript API documentation...${NC}"
    
    cd "$PROJECT_ROOT/crates/libretune-app"
    
    if command -v npx &> /dev/null; then
        # Check if typedoc is installed
        if ! npm list typedoc &> /dev/null; then
            echo -e "${YELLOW}Installing typedoc...${NC}"
            npm install --save-dev typedoc typedoc-plugin-missing-exports
        fi
        
        npx typedoc 2>&1 | tail -5
        echo -e "${GREEN}✓ Frontend docs: docs/api/index.html${NC}"
    else
        echo -e "${RED}✗ npm/npx not found. Install Node.js to build TypeScript documentation.${NC}"
    fi
    
    cd "$PROJECT_ROOT"
    echo ""
fi

# Build User Manual
if [ "$BUILD_USER" = true ]; then
    echo -e "${YELLOW}📖 Building User Manual...${NC}"
    
    if command -v mdbook &> /dev/null; then
        cd "$PROJECT_ROOT/docs"
        mdbook build 2>&1 | tail -5
        echo -e "${GREEN}✓ User manual: docs/book/index.html${NC}"
        cd "$PROJECT_ROOT"
    else
        echo -e "${RED}✗ mdBook not found. Install with: cargo install mdbook${NC}"
        echo -e "${YELLOW}  Alternatively: brew install mdbook (macOS) or download from GitHub releases${NC}"
    fi
    echo ""
fi

# Summary
echo -e "${BLUE}════════════════════════════════════════${NC}"
echo -e "${GREEN}Documentation build complete!${NC}"
echo ""
echo "Output locations:"
if [ "$BUILD_RUST" = true ]; then
    echo -e "  Rust API:      ${BLUE}file://$PROJECT_ROOT/target/doc/libretune_core/index.html${NC}"
fi
if [ "$BUILD_FRONTEND" = true ]; then
    echo -e "  Frontend API:  ${BLUE}file://$PROJECT_ROOT/docs/api/index.html${NC}"
fi
if [ "$BUILD_USER" = true ]; then
    echo -e "  User Manual:   ${BLUE}file://$PROJECT_ROOT/docs/book/index.html${NC}"
fi
echo ""

# Open in browser
if [ "$OPEN" = true ]; then
    if [ "$BUILD_USER" = true ] && [ -f "$PROJECT_ROOT/docs/book/index.html" ]; then
        echo -e "${YELLOW}Opening user manual in browser...${NC}"
        if command -v xdg-open &> /dev/null; then
            xdg-open "$PROJECT_ROOT/docs/book/index.html"
        elif command -v open &> /dev/null; then
            open "$PROJECT_ROOT/docs/book/index.html"
        fi
    elif [ "$BUILD_RUST" = true ] && [ -f "$PROJECT_ROOT/target/doc/libretune_core/index.html" ]; then
        echo -e "${YELLOW}Opening Rust docs in browser...${NC}"
        if command -v xdg-open &> /dev/null; then
            xdg-open "$PROJECT_ROOT/target/doc/libretune_core/index.html"
        elif command -v open &> /dev/null; then
            open "$PROJECT_ROOT/target/doc/libretune_core/index.html"
        fi
    fi
fi

# Serve documentation
if [ "$SERVE" = true ]; then
    if [ "$BUILD_USER" = true ] && command -v mdbook &> /dev/null; then
        echo -e "${YELLOW}Serving user manual at http://localhost:3000 ...${NC}"
        cd "$PROJECT_ROOT/docs"
        mdbook serve --open --port 3000
    elif command -v python3 &> /dev/null; then
        echo -e "${YELLOW}Serving documentation at http://localhost:8000 ...${NC}"
        cd "$PROJECT_ROOT/docs"
        python3 -m http.server 8000
    else
        echo -e "${RED}No suitable server found. Install mdbook or python3.${NC}"
    fi
fi
