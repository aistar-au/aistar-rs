# ADR-007: Runtime-core canonical dispatch — no alternate routing

**Date:** 2026-02-19
**Status:** Accepted
**Deciders:** Core maintainer
**Supersedes operationally:** ADR-004 (headless-first seam — now fully realized)
**Related tasks:** TASKS/REF-08-full-runtime-cutover.md

## Decision (normative)

After REF-08:

- MUST: All user input MUST flow only through `Runtime<M>::run` →
  `RuntimeMode::on_user_input` → `RuntimeContext::start_turn`.
- MUST NOT: No code outside `RuntimeContext::start_turn` may call
  `ConversationManager::send_message` in the production path.
- MUST NOT: `src/app` MUST NOT own any `mpsc` channel for conversation dispatch.
- MUST NOT: `src/state`, `src/api`, `src/tools` MUST NOT import runtime dispatch
  interfaces (`runtime::context`, `runtime::mode`, `runtime::loop`,
  `runtime::frontend`, `runtime::update`, `runtime::event`) or `crate::app`.
- MUST: `RuntimeContext::start_turn` MUST emit exactly one terminal event per turn
  (`TurnComplete` or `Error`).
- MUST: `RuntimeContext::start_turn` MUST check for an active Tokio runtime before
  spawning; on failure it emits `UiUpdate::Error` and returns without touching history.

These rules are enforced by `scripts/check_no_alternate_routing.sh` and
`scripts/check_forbidden_imports.sh` in CI.
