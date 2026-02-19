#!/usr/bin/env bash
set -euo pipefail

FORBIDDEN_MODULES=(
    "runtime::context"
    "runtime::mode"
    "runtime::r#loop"
    "runtime::frontend"
    "runtime::update"
    "runtime::event"
    "crate::app"
)

DIRS_TO_CHECK=("src/state" "src/api" "src/tools")

FAIL=0
for dir in "${DIRS_TO_CHECK[@]}"; do
    for mod in "${FORBIDDEN_MODULES[@]}"; do
        if grep -rn "use.*$mod\|extern.*$mod" "$dir/" 2>/dev/null; then
            echo "FAIL: $dir imports forbidden module $mod"
            FAIL=1
        fi
    done
done

if [ $FAIL -eq 1 ]; then
    echo ""
    echo "Layer violation detected. See ADR-007."
    exit 1
fi

echo "check_forbidden_imports: clean"
