# Agent Dispatch: ADR-006 Runtime Contracts (REF-02 → REF-06)

Paste one of the blocks below into your CLI agent (Aider, Claude Code, etc.)
to dispatch a task. Always start with REF-02. Do not dispatch a task until
its predecessor anchor test is green.

---

## Dispatch REF-02 (start here)

```
Refer to CONTRIBUTING.md.

I am assigning you Task REF-02.

Read TASKS/REF-02-runtime-mode-contract.md.
Read docs/adr/ADR-006-runtime-mode-contracts.md for full context on the contract shapes.

Create the stub types exactly as specified in the task manifest.
Do not add logic or modify src/app/mod.rs.
Do not proceed to REF-03.

Anchor: cargo test test_ref_02_runtime_types_compile -- --nocapture
```

---

## Dispatch REF-03 (only after REF-02 anchor is green)

```
Refer to CONTRIBUTING.md.

I am assigning you Task REF-03.

Read TASKS/REF-03-tui-mode-implement.md.
Read docs/adr/ADR-006-runtime-mode-contracts.md — specifically the TuiMode section and the CORE-10/CORE-11 requirements.

Your only target file is src/app/mod.rs. Do not create new files.
Add TuiMode, HistoryState, InputState, OverlayState, OverlayKind, and implement RuntimeMode for TuiMode exactly as specified.
Do not move the ratatui draw loop. Do not implement FrontendAdapter. Do not add CLI flags.
Do not proceed to REF-04.

Anchor: cargo test test_ref_03_tui_mode_overlay_blocks_input -- --nocapture
```

---

## Dispatch REF-04 (only after REF-03 anchor is green)

```
Refer to CONTRIBUTING.md.

I am assigning you Task REF-04.

Read TASKS/REF-04-runtime-context-start-turn.md.
Read docs/adr/ADR-006-runtime-mode-contracts.md — specifically the RuntimeContext section.

Your target files are src/runtime/context.rs, src/app/mod.rs (call sites only), and tests/ref_04_start_turn.rs (new file).
Implement RuntimeContext::start_turn and RuntimeContext::cancel_turn as specified.
Remove or annotate old dispatch call sites in src/app/mod.rs as directed in the task manifest.
Do not move the ratatui draw loop. Do not implement Runtime<M>::run(). Do not add CLI flags.
Do not proceed to REF-05.

Anchor: cargo test test_ref_04_start_turn_dispatches_message -- --nocapture
```

---

## Giving app permission

The agent needs read/write access to:

```
src/
  runtime.rs        (REF-02 start point; moved to runtime/mod.rs)
  runtime/          (REF-02 creates this; REF-04 edits context.rs)
  app/mod.rs        (REF-03 and REF-04 call sites)
tests/              (REF-04 creates ref_04_start_turn.rs)
docs/adr/           (read-only reference)
TASKS/              (read-only reference)
CONTRIBUTING.md     (read-only reference)
```

In Aider:
```bash
# REF-02
aider src/runtime.rs \
  --read docs/adr/ADR-006-runtime-mode-contracts.md \
  --read TASKS/REF-02-runtime-mode-contract.md \
  --read CONTRIBUTING.md

# REF-03
aider src/app/mod.rs \
  --read docs/adr/ADR-006-runtime-mode-contracts.md \
  --read TASKS/REF-03-tui-mode-implement.md \
  --read CONTRIBUTING.md \
  --read src/runtime/mode.rs \
  --read src/runtime/context.rs

# REF-04
aider src/runtime/context.rs src/app/mod.rs tests/ref_04_start_turn.rs \
  --read docs/adr/ADR-006-runtime-mode-contracts.md \
  --read TASKS/REF-04-runtime-context-start-turn.md \
  --read CONTRIBUTING.md \
  --read src/runtime/mod.rs
```

In Claude Code:
```bash
# REF-02
claude "Refer to CONTRIBUTING.md. I am assigning you Task REF-02. \
Read TASKS/REF-02-runtime-mode-contract.md and \
docs/adr/ADR-006-runtime-mode-contracts.md. \
Create the stub types as specified. \
Anchor: cargo test test_ref_02_runtime_types_compile"

# REF-03
claude "Refer to CONTRIBUTING.md. I am assigning you Task REF-03. \
Read TASKS/REF-03-tui-mode-implement.md and \
docs/adr/ADR-006-runtime-mode-contracts.md. \
Implement TuiMode in src/app/mod.rs as specified. \
Anchor: cargo test test_ref_03_tui_mode_overlay_blocks_input"

# REF-04
claude "Refer to CONTRIBUTING.md. I am assigning you Task REF-04. \
Read TASKS/REF-04-runtime-context-start-turn.md and \
docs/adr/ADR-006-runtime-mode-contracts.md. \
Implement RuntimeContext::start_turn and ::cancel_turn. \
Remove old dispatch call sites in src/app/mod.rs as directed. \
Anchor: cargo test test_ref_04_start_turn_dispatches_message"
```

---

## Sequence checkpoint

| Task | Anchor test | Must stay green |
| :--- | :--- | :--- |
| REF-02 | `cargo test test_ref_02_runtime_types_compile` | — |
| REF-03 | `cargo test test_ref_03_tui_mode_overlay_blocks_input` | REF-02 anchor |
| REF-04 | `cargo test test_ref_04_start_turn_dispatches_message` | REF-02, REF-03 anchors |

REF-05 (generic `Runtime<M>` loop) and REF-06 (`TuiFrontend` adapter) follow the
same pattern. Read `docs/adr/ADR-006-runtime-mode-contracts.md` §5 for their
scope, then create the manifest files in `TASKS/` before dispatching.
