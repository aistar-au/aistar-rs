#!/usr/bin/env bash
set -euo pipefail

PATTERNS=(
    "message_tx"
    "message_rx"
    "send_message("
    "ConversationStreamUpdate"
    "update_rx\.recv"
    "update_rx\.try_recv"
)

RUNTIME_POLICY_PATTERNS=(
    "fn strip_tagged_tool_markup"
    "fn request_likely_requires_tools"
)

FAIL=0
for pattern in "${PATTERNS[@]}"; do
    if grep -rn "$pattern" src/app/; then
        echo "FAIL: forbidden pattern '$pattern' found in src/app/"
        FAIL=1
    fi
done

for pattern in "${RUNTIME_POLICY_PATTERNS[@]}"; do
    if grep -rn "$pattern" src/app/ src/state/; then
        echo "FAIL: runtime policy duplicate '$pattern' found outside src/runtime/policy.rs"
        FAIL=1
    fi
done

if [ $FAIL -eq 1 ]; then
    echo ""
    echo "Alternate routing is forbidden. See ADR-007."
    echo "Runtime policy centralization is required. See ADR-014."
    exit 1
fi

echo "check_no_alternate_routing: clean"
