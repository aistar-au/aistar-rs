# ADR-016: Local Tool-Loop Guard and Correction Path

**Date:** 2026-02-21
**Status:** Accepted
**Deciders:** Core maintainer
**Related tasks:** `TASKS/completed/CRIT-19-local-loop-guard-correction.md`

## Context

Local tool rounds could enter repetitive read/search loops and eventually
surface:

`[error] Exceeded max tool rounds (...)`

This leaked runtime control flow into user-visible output and violated the
intended iterative contract:

tool call -> enrichment -> reassess -> finalize

## Decision

1. Add explicit correction when repeated identical read/search rounds are
   detected.
2. Skip execution of duplicated rounds and request a different action/final
   response.
3. If repetition persists, return a guarded final message rather than bubbling
   an internal runtime error.
4. Convert max-round overflow from hard error to guarded completion text.

## Consequences

- Repetitive loops self-correct once, then terminate cleanly if unresolved.
- Users see loop-guard feedback instead of runtime error noise.
- Loop handling remains in runtime-core conversation orchestration.

## Compliance notes for agents

1. Do not reintroduce hard max-round runtime errors in normal local loop flow.
2. Keep loop correction logic in `src/state/conversation.rs`.
3. Ensure loop guard behavior remains covered by tests.
