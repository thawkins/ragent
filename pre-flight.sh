#!/bin/bash
# pre-flight.sh - Run all GitHub Actions CI checks locally before pushing
#
# Usage: ./pre-flight.sh [--quick]
#   --quick: Skip slow checks (security audit, full test suite)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track results
PASSED=0
FAILED=0
SKIPPED=0

# Parse arguments
QUICK_MODE=false
for arg in "$@"; do
    case $arg in
        --quick)
            QUICK_MODE=true
            shift
            ;;
    esac
done

# Helper functions
print_header() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

print_step() {
    echo -e "${YELLOW}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
    PASSED=$((PASSED + 1))
}

print_failure() {
    echo -e "${RED}✗ $1${NC}"
    FAILED=$((FAILED + 1))
}

print_skip() {
    echo -e "${YELLOW}⊘ $1 (skipped)${NC}"
    SKIPPED=$((SKIPPED + 1))
}

# Ensure we're in the repo root
cd "$(dirname "$0")"

print_header "Pre-flight CI Checks"
echo "Running the same checks as GitHub Actions CI..."
if $QUICK_MODE; then
    echo -e "${YELLOW}Quick mode enabled - skipping slow checks${NC}"
fi
echo ""

# Set RUSTFLAGS to match CI
export RUSTFLAGS="-D warnings"
export CARGO_TERM_COLOR=always

# =============================================================================
# 1. Format Check
# =============================================================================
print_header "1. Format Check (cargo fmt)"
print_step "Checking code formatting..."

if cargo fmt --all -- --check; then
    print_success "Format check passed"
else
    print_failure "Format check failed - run 'cargo fmt' to fix"
fi

# =============================================================================
# 2. Cargo Check
# =============================================================================
print_header "2. Cargo Check"
print_step "Checking workspace compiles..."

if cargo check --workspace 2>&1; then
    print_success "Cargo check passed"
else
    print_failure "Cargo check failed"
fi

# =============================================================================
# 3. Clippy
# =============================================================================
print_header "3. Clippy Lints"
print_step "Running clippy lints..."

if cargo clippy --workspace -- -D warnings 2>&1; then
    print_success "Clippy check passed"
else
    print_failure "Clippy check failed"
fi

# =============================================================================
# 4. Tests
# =============================================================================
print_header "4. Test Suite"

if $QUICK_MODE; then
    print_step "Running quick tests (lib only)..."
    if cargo test --workspace --lib 2>&1; then
        print_success "Quick tests passed"
    else
        print_failure "Quick tests failed"
    fi
else
    print_step "Running full test suite..."
    if timeout 600 cargo test --workspace 2>&1; then
        print_success "All tests passed"
    else
        print_failure "Tests failed"
    fi
fi

# =============================================================================
# 5. Security Audit (cargo-audit)
# =============================================================================
print_header "5. Security Audit (cargo-audit)"

if $QUICK_MODE; then
    print_skip "Security audit"
else
    if ! command -v cargo-audit &> /dev/null; then
        print_step "Installing cargo-audit..."
        cargo install cargo-audit --locked --quiet
    fi

    print_step "Running security audit..."
    if cargo audit 2>&1; then
        print_success "Security audit passed"
    else
        print_failure "Security audit found vulnerabilities"
    fi
fi

# =============================================================================
# 6. Cargo Deny
# =============================================================================
print_header "6. Cargo Deny (license & advisory check)"

if $QUICK_MODE; then
    print_skip "Cargo deny"
else
    if ! command -v cargo-deny &> /dev/null; then
        print_step "Installing cargo-deny..."
        cargo install cargo-deny --locked --quiet
    fi

    print_step "Running cargo deny..."
    if cargo deny check 2>&1; then
        print_success "Cargo deny passed"
    else
        print_failure "Cargo deny found issues"
    fi
fi

# =============================================================================
# Summary
# =============================================================================
print_header "Summary"

TOTAL=$((PASSED + FAILED + SKIPPED))

echo ""
echo -e "  ${GREEN}Passed:  $PASSED${NC}"
echo -e "  ${RED}Failed:  $FAILED${NC}"
echo -e "  ${YELLOW}Skipped: $SKIPPED${NC}"
echo -e "  Total:   $TOTAL"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}════════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  ✓ All checks passed! Safe to push.${NC}"
    echo -e "${GREEN}════════════════════════════════════════════════════════════════${NC}"
    exit 0
else
    echo -e "${RED}════════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}  ✗ $FAILED check(s) failed. Fix issues before pushing.${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════════${NC}"
    exit 1
fi
