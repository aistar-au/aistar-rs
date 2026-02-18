# REF-03: Implement `TuiMode` — migrate `App` state into runtime structs

**Status:** Blocked on REF-02  
**Date:** 2026-02-18  
**ADR:** [`docs/adr/ADR-006-runtime-mode-contracts.md`](../docs/adr/ADR-006-runtime-mode-contracts.md)  
**Depends on:** REF-02 (`test_ref_02_runtime_types_compile` must be green)  
**Blocks:** REF-04

---

## Context

REF-02 defined the `RuntimeMode` trait and supporting types as stubs.
This task wires the TUI as the first implementation.

`App` in `src/app/mod.rs` currently holds overlay state, input state,
message history, and turn progress as ad-hoc fields. This task groups
them into explicit structs and implements `RuntimeMode for TuiMode`,
enforcing the overlay routing contract (CORE-10) and the one-shot
approval guarantee (CORE-11) structurally.

No new files. No changes outside `src/app/mod.rs`.

---

## Target file

`src/app/mod.rs` only.

---

## What to add

### State structs (add near the top of the file, before `App`)

```rust
struct HistoryState {
    messages: Vec<String>,
    scroll: usize,
}

struct InputState {
    buffer: String,
    cursor_byte: usize,
}

enum OverlayKind {
    ToolPermission(crate::types::ToolApprovalRequest),
    Error(String),
}

struct OverlayState {
    kind: OverlayKind,
}

pub struct TuiMode {
    history: HistoryState,
    input: InputState,
    overlay: Option<OverlayState>,
    turn_in_progress: bool,
}

impl TuiMode {
    pub fn new() -> Self {
        Self {
            history: HistoryState { messages: Vec::new(), scroll: 0 },
            input: InputState { buffer: String::new(), cursor_byte: 0 },
            overlay: None,
            turn_in_progress: false,
        }
    }
}
```

### `RuntimeMode` implementation

```rust
use crate::runtime::mode::RuntimeMode;
use crate::runtime::context::RuntimeContext;
use crate::types::UiUpdate;

impl RuntimeMode for TuiMode {
    fn on_user_input(&mut self, input: String, ctx: &mut RuntimeContext) {
        // CORE-10: overlay active → reject all normal input
        if self.overlay.is_some() {
            return;
        }
        // Turn guard: only one turn in flight at a time
        if self.turn_in_progress {
            return;
        }
        self.turn_in_progress = true;
        // ctx.start_turn() is wired in REF-04; call is a no-op (todo!) until then
        let _ = input;
        let _ = ctx;
    }

    fn on_model_update(&mut self, update: UiUpdate, _ctx: &mut RuntimeContext) {
        match update {
            UiUpdate::StreamDelta(text) => {
                if let Some(last) = self.history.messages.last_mut() {
                    last.push_str(&text);
                } else {
                    self.history.messages.push(text);
                }
            }
            UiUpdate::ToolApprovalRequest(req) => {
                // CORE-11: set exactly once; only cleared in overlay dismissal path
                self.overlay = Some(OverlayState {
                    kind: OverlayKind::ToolPermission(req),
                });
            }
            UiUpdate::TurnComplete => {
                self.turn_in_progress = false;
            }
            UiUpdate::Error(msg) => {
                self.history.messages.push(format!("[error] {msg}"));
                self.turn_in_progress = false;
            }
            _ => {}
        }
    }

    fn is_turn_in_progress(&self) -> bool {
        self.turn_in_progress
    }
}
```

---

## Anchor test

Add inside the existing `#[cfg(test)]` block at the bottom of `src/app/mod.rs`:

```rust
#[test]
fn test_ref_03_tui_mode_overlay_blocks_input() {
    use crate::types::{ToolApprovalRequest, UiUpdate};
    use crate::runtime::mode::RuntimeMode;

    // Build a minimal RuntimeContext with a dummy ConversationManager.
    // This test does not start a real turn — ctx is only passed through.
    // If RuntimeContext requires a live ConversationManager, use
    // ConversationManager::new_mock() (see ADR-005 / mock_client.rs).

    let mut mode = TuiMode::new();
    assert!(!mode.is_turn_in_progress());

    // Simulate a ToolApprovalRequest opening an overlay.
    let req = ToolApprovalRequest::test_stub(); // add a test_stub() if not present
    mode.on_model_update(UiUpdate::ToolApprovalRequest(req), &mut dummy_ctx());

    // Overlay is now active — on_user_input must be a no-op.
    assert!(mode.overlay.is_some());
    mode.on_user_input("should be ignored".to_string(), &mut dummy_ctx());
    assert!(!mode.is_turn_in_progress(), "turn must not start while overlay is active");

    // Clear overlay (simulating dismissal).
    mode.overlay = None;
    mode.on_user_input("now accepted".to_string(), &mut dummy_ctx());
    assert!(mode.is_turn_in_progress(), "turn should start after overlay cleared");
}
```

You will need a `dummy_ctx()` helper in the test module:

```rust
fn dummy_ctx<'a>() -> crate::runtime::context::RuntimeContext<'a> {
    // This requires an actual &mut ConversationManager.
    // Construct one using ConversationManager::new_for_test() or the existing
    // mock pattern from src/api/mock_client.rs.
    todo!("construct minimal RuntimeContext for test")
}
```

If `ConversationManager` cannot be constructed without an `ApiClient`, use
`ApiClient::new_mock()` (ADR-005). The test must not make real HTTP requests.

---

## Verification

```bash
cargo test test_ref_03_tui_mode_overlay_blocks_input -- --nocapture
cargo test --all   # all prior tests must remain green
```

---

## Definition of done

- [ ] `TuiMode`, `HistoryState`, `InputState`, `OverlayState`, `OverlayKind` defined in `src/app/mod.rs`.
- [ ] `RuntimeMode for TuiMode` implemented with overlay block and turn guard.
- [ ] `test_ref_03_tui_mode_overlay_blocks_input` passes without a real TTY.
- [ ] `cargo test --all` is green.
- [ ] `test_ref_02_runtime_types_compile` still passes (do not break REF-02).
- [ ] No new files created outside `src/app/mod.rs`.

---

## What NOT to do

- Do not call `ctx.start_turn()` in a way that makes a real API request — it is `todo!()` until REF-04.
- Do not move `App`'s existing ratatui draw loop — that is REF-05's job.
- Do not implement `FrontendAdapter` — that is REF-06's job.
- Do not add CLI flags or environment variables.
