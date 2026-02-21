# ADR-014: Runtime-Core Policy Deduplication and Enforcement

**Date:** 2026-02-21
**Status:** Accepted
**Deciders:** Core maintainer
**Related tasks:** `TASKS/completed/CRIT-17-tui-stream-leak-sanitization.md`

## Context

Recent TUI/runtime hardening introduced useful behavior, but policy logic was split
across multiple modules:

- assistant text sanitization lived in more than one place
- tool-evidence retry heuristics lived in conversation-only helpers
- retry guidance strings were inline instead of centralized

This pattern increases code size, creates drift risk, and weakens the runtime-core
contract from ADR-007/ADR-009/ADR-010 by allowing feature policy to spread beyond
the runtime boundary.

## Decision

1. Introduce a runtime-core policy module (`src/runtime/policy.rs`) as the single
   source of truth for:
   - assistant text sanitization rules
   - tool-evidence requirement heuristics
   - corrective retry instruction text

2. Define a shared runtime trait:
   - `RuntimeCorePolicy`
   - default implementation: `DefaultRuntimeCorePolicy`

3. Require TUI-facing paths to consume runtime policy helpers instead of duplicating
   local helper functions:
   - `src/state/conversation.rs`
   - `src/app/mod.rs`

4. Remove duplicated local helpers that are superseded by runtime-core policy.

## Rationale

- Keeps behavior consistent between conversation loop and TUI rendering.
- Reduces code growth by deduplicating logic and constants.
- Preserves ADR-007 no-alternate-routing intent by centralizing behavior behind
  runtime-core-owned abstractions.
- Makes future policy changes testable in one place.

## Alternatives considered

1. Keep helper functions local in each module
   - Rejected: duplicates continue to drift and inflate maintenance cost.

2. Move policy into `src/app/`
   - Rejected: violates runtime-core ownership of cross-cutting interaction policy.

3. Add policy into `src/state/` only
   - Rejected: TUI and runtime share the behavior; runtime module is the stable seam.

## Consequences

- Runtime policy logic is explicit and reusable.
- Existing behavior remains, but source of truth moves to runtime-core.
- New policy logic should be added to `src/runtime/policy.rs`, not ad hoc helpers.

## Compliance notes for agents

1. Do not add duplicate assistant-sanitization or tool-evidence helper functions in
   `src/app/` or `src/state/`.
2. Changes to retry enforcement must be implemented through runtime-core policy.
3. TUI features that depend on shared interaction policy must call runtime-core
   helpers/traits.
4. Keep `cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets`
   green after refactors.
