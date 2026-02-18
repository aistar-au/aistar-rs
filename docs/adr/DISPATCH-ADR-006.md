# Agent Dispatch: ADR-006 Runtime Contracts (REF-02 → REF-03)

Paste one of the blocks below into your CLI agent (Aider, Claude Code, etc.)
to dispatch a task. Always start with REF-02. Do not dispatch REF-03 until
`test_ref_02_runtime_types_compile` is green.

---

## Dispatch REF-02 (start here)

```
Refer to CONTRIBUTING.md.

I am assigning you Task REF-02.

1. Read TASKS/REF-02-runtime-mode-contract.md.
2. Read docs/adr/ADR-006-runtime-mode-contracts.md for full context on the
   contract shapes.
3. Create the stub types exactly as specified in the task manifest. Do not
   add logic or modify src/app/mod.rs.
4. Your anchor test is test_ref_02_runtime_types_compile in src/runtime/mod.rs.
5. Verify with:
   cargo test test_ref_02_runtime_types_compile -- --nocapture
   cargo test --all
6. Confirm no ratatui or crossterm imports appear in src/runtime/.

Do not proceed to REF-03. Stop when the anchor test is green and all other
tests remain passing.
```

---

## Dispatch REF-03 (only after REF-02 anchor is green)

```
Refer to CONTRIBUTING.md.

I am assigning you Task REF-03.

1. Read TASKS/REF-03-tui-mode-implement.md.
2. Read docs/adr/ADR-006-runtime-mode-contracts.md — specifically the TuiMode
   section and the CORE-10/CORE-11 requirements.
3. Your only target file is src/app/mod.rs. Do not create new files.
4. Add TuiMode, HistoryState, InputState, OverlayState, OverlayKind, and
   implement RuntimeMode for TuiMode exactly as specified.
5. Your anchor test is test_ref_03_tui_mode_overlay_blocks_input.
6. Verify with:
   cargo test test_ref_03_tui_mode_overlay_blocks_input -- --nocapture
   cargo test --all
7. Confirm test_ref_02_runtime_types_compile still passes.

Do not move the ratatui draw loop. Do not implement FrontendAdapter.
Do not add CLI flags. Stop when both anchor tests are green.
```

---

## Giving app permission

The agent needs read/write access to:

```
src/
  lib.rs
  runtime/          (REF-02 creates this)
  app/mod.rs        (REF-03 only)
docs/adr/           (read-only reference)
TASKS/              (read-only reference)
CONTRIBUTING.md     (read-only reference)
```

In Aider:
```bash
aider src/lib.rs src/app/mod.rs \
  --read docs/adr/ADR-006-runtime-mode-contracts.md \
  --read TASKS/REF-02-runtime-mode-contract.md \
  --read CONTRIBUTING.md
```

For REF-03, swap the task file:
```bash
aider src/app/mod.rs \
  --read docs/adr/ADR-006-runtime-mode-contracts.md \
  --read TASKS/REF-03-tui-mode-implement.md \
  --read CONTRIBUTING.md \
  --read src/runtime/mode.rs \
  --read src/runtime/context.rs
```

In Claude Code (claude command):
```bash
claude "Refer to CONTRIBUTING.md. I am assigning you Task REF-02. \
Read TASKS/REF-02-runtime-mode-contract.md and \
docs/adr/ADR-006-runtime-mode-contracts.md. \
Create the stub types as specified. \
Anchor: cargo test test_ref_02_runtime_types_compile"
```

---

## Sequence checkpoint

After each task, verify before moving on:

| Task | Command | Must be green |
| :--- | :--- | :--- |
| REF-02 | `cargo test test_ref_02_runtime_types_compile` | ✓ |
| REF-02 | `cargo test --all` | ✓ |
| REF-03 | `cargo test test_ref_03_tui_mode_overlay_blocks_input` | ✓ |
| REF-03 | `cargo test test_ref_02_runtime_types_compile` | ✓ (must stay green) |
| REF-03 | `cargo test --all` | ✓ |

REF-04 through REF-06 follow the same pattern.
Read `docs/adr/ADR-006-runtime-mode-contracts.md` for their task descriptions,
then create the manifest files in `TASKS/` before dispatching.
