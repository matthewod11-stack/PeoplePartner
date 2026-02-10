#!/bin/bash
# HR Command Center - Session Initialization Script
# Run at the start of each development session
#
# Usage: ./scripts/dev-init.sh

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  HR Command Center — Session Init                             ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 1. Confirm directory (case-insensitive)
EXPECTED_DIR="hrcommand"
CURRENT_DIR=$(basename "$PWD" | tr '[:upper:]' '[:lower:]')

if [ "$CURRENT_DIR" != "$EXPECTED_DIR" ]; then
    echo -e "${RED}ERROR: Expected to be in HRCommand directory${NC}"
    echo "Current: $PWD"
    exit 1
fi
echo -e "${GREEN}✓${NC} Working directory: $PWD"

# 2. Check session tracking files
echo ""
echo -e "${BLUE}Checking session tracking files...${NC}"

TRACKING_FILES=(
    "ROADMAP.md"
    "docs/PROGRESS.md"
    "docs/SESSION_PROTOCOL.md"
    "docs/KNOWN_ISSUES.md"
    "features.json"
)

ALL_PRESENT=true
for file in "${TRACKING_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓${NC} $file"
    else
        echo -e "${RED}✗ MISSING: $file${NC}"
        ALL_PRESENT=false
    fi
done

if [ "$ALL_PRESENT" = false ]; then
    echo ""
    echo -e "${YELLOW}Warning: Some tracking files missing. Session continuity may be affected.${NC}"
fi

# 3. Check architecture docs
echo ""
echo -e "${BLUE}Checking architecture docs...${NC}"

ARCH_FILES=(
    "docs/HR-Command-Center-Roadmap.md"
    "docs/HR-Command-Center-Design-Architecture.md"
)

for file in "${ARCH_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}✓${NC} $file"
    else
        echo -e "${YELLOW}⚠${NC} $file (optional)"
    fi
done

# 4. Check tooling (only if package.json exists - meaning we've started coding)
if [ -f "package.json" ]; then
    echo ""
    echo -e "${BLUE}Checking project dependencies...${NC}"

    if [ -d "node_modules" ]; then
        echo -e "${GREEN}✓${NC} node_modules exists"
    else
        echo -e "${YELLOW}Installing dependencies...${NC}"
        npm install
    fi

    # Run verification if scripts exist
    echo ""
    echo -e "${BLUE}Running verification...${NC}"

    if npm run type-check > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Type check passes"
    else
        echo -e "${YELLOW}⚠${NC} Type check not configured or failing"
    fi

    if npm run build > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Build succeeds"
    else
        echo -e "${YELLOW}⚠${NC} Build not configured or failing"
    fi

    if npm test -- --passWithNoTests > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Tests pass"
    else
        echo -e "${YELLOW}⚠${NC} Tests not configured or failing"
    fi
else
    echo ""
    echo -e "${YELLOW}No package.json yet — project scaffolding not started${NC}"
    echo "Next step: Phase 1.1 - Initialize Tauri + React + Vite project"
fi

# 5. Show feature status
echo ""
echo -e "${BLUE}═══ Feature Status ═══${NC}"
if [ -f "features.json" ]; then
    PASS=$(grep -c '"status": "pass"' features.json 2>/dev/null | tr -d '\n' || echo "0")
    FAIL=$(grep -c '"status": "fail"' features.json 2>/dev/null | tr -d '\n' || echo "0")
    IN_PROGRESS=$(grep -c '"status": "in-progress"' features.json 2>/dev/null | tr -d '\n' || echo "0")
    NOT_STARTED=$(grep -c '"status": "not-started"' features.json 2>/dev/null | tr -d '\n' || echo "0")

    # Handle empty values
    [ -z "$PASS" ] && PASS="0"
    [ -z "$FAIL" ] && FAIL="0"
    [ -z "$IN_PROGRESS" ] && IN_PROGRESS="0"
    [ -z "$NOT_STARTED" ] && NOT_STARTED="0"

    echo -e "${GREEN}Pass:${NC} $PASS  ${RED}Fail:${NC} $FAIL  ${YELLOW}In Progress:${NC} $IN_PROGRESS  Not Started: $NOT_STARTED"
else
    echo "features.json not found"
fi

# 6. Show recent progress
echo ""
echo -e "${BLUE}═══ Most Recent Session ═══${NC}"
if [ -f "docs/PROGRESS.md" ]; then
    # Extract most recent session (first ## Session block)
    awk '/^## Session/{if(found)exit; found=1} found' docs/PROGRESS.md | head -30
else
    echo "No PROGRESS.md found"
fi

# 7. Show next tasks
echo ""
echo -e "${BLUE}═══ Next Tasks ═══${NC}"
if [ -f "ROADMAP.md" ]; then
    # Show first 5 unchecked tasks
    grep -n "\- \[ \]" ROADMAP.md | head -5 | while read line; do
        echo "$line"
    done
else
    echo "No ROADMAP.md found"
fi

# 8. Show known issues/blockers
echo ""
echo -e "${BLUE}═══ Known Blockers ═══${NC}"
if [ -f "docs/KNOWN_ISSUES.md" ]; then
    BLOCKERS=$(grep -c "Severity: Blocker" docs/KNOWN_ISSUES.md 2>/dev/null | tr -d '\n' || echo "0")
    [ -z "$BLOCKERS" ] && BLOCKERS="0"
    if [ "$BLOCKERS" -gt 0 ] 2>/dev/null; then
        echo -e "${RED}$BLOCKERS blocker(s) found - check docs/KNOWN_ISSUES.md${NC}"
    else
        echo -e "${GREEN}No blockers${NC}"
    fi
else
    echo "No KNOWN_ISSUES.md found"
fi

# Final message
echo ""
echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Session ready.${NC} Follow single-feature-per-session rule."
echo ""
echo "Quick commands:"
echo "  cat docs/PROGRESS.md     # Full session history"
echo "  cat ROADMAP.md           # Task checklist"
echo "  cat features.json        # Pass/fail status"
echo ""
