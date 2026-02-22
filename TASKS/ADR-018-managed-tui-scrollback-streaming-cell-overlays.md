# ADR-018: Managed TUI — Scrollback, Streaming Cell, Overlays

**Date:** 2026-02-22
**Status:** Proposed
**Deciders:** Core maintainer
**Related tasks:** CORE-15, CORE-16, CORE-17, FEAT-17, FEAT-18, FEAT-19
**ADR chain:** ADR-006, ADR-007, ADR-009, ADR-010
**Supersedes:** ADR-017 (on acceptance + migration completion)

## Context

Current runtime canonical path is append-terminal (`src/bin/vex.rs`). It is
simple and stable, but it does not provide managed in-app scrollback or a
viewport model that supports transcript navigation while composing input.

For long sessions, users need explicit viewport control (`scroll_offset`,
`auto_follow`) and deterministic streaming behavior in one active render cell.
This aligns with common open-source Rust TUI patterns (`ratatui`,
`crossterm`, `tokio`).

## Decision

1. Move canonical runtime interaction to managed TUI with three panes
   (status/transcript/input).
2. Keep transcript state in one widget (`ChatWidget`) with
   `cells + active + scroll_offset + auto_follow`.
3. Use one active streaming cell; commit on `TurnComplete`.
4. Route keyboard/mouse navigation into widget scrolling APIs.
5. Keep overlays lifecycle-managed (enter/leave paired, panic-safe).

## `UiUpdate` Alignment (normative)

This ADR uses the existing file and shapes in `src/runtime/update.rs`:

- `StreamBlockStart { index, block }`
- `StreamBlockDelta { index, delta }`
- `StreamBlockComplete { index }`
- `TurnComplete`

No duplicate streaming variants are introduced.

If tool lifecycle requires dedicated events, only additive tool-specific
variants are allowed (e.g., `ToolCallStarted`, `ToolCallCompleted`) and must
not overlap existing `StreamBlock*` streaming semantics.

## Terminal Abstraction Compatibility

`CustomTerminal` may use ratatui insertion APIs for inline viewport behavior.
Implementation must be validated against the pinned ratatui version in this
repo (`ratatui = 0.29`) before task dispatch is considered complete.

## Migration

1. CORE-15 adds terminal abstraction and insertion support.
2. CORE-16 adds chat widget state and stream/event mapping.
3. CORE-17 wires app/frontend to the managed viewport and retires direct
   append rendering path.

Until CORE-17 gates are green, ADR-017 remains operational.

## Supersede Mechanics

On acceptance of ADR-018 and successful CORE-17 cutover:

1. Mark `TASKS/completed/ADR-017-append-terminal-single-session.md` as
   `Superseded by ADR-018`.
2. Update `TASKS/ADR-README.md` status row for ADR-017 accordingly.
3. Keep ADR-017 in history (do not delete).

## Consequences

- Resolves managed scrollback/viewport limitations of append-only runtime path.
- Increases UI state complexity and requires strict regression coverage.
- Does not change tool-loop guard policy (that remains ADR-016 scope).

## Compliance Notes for Agents

1. Do not split transcript ownership across multiple modules.
2. Keep runtime-core contract boundaries intact (ADR-006/ADR-007).
3. Do not delete superseded ADRs; mark them superseded.
