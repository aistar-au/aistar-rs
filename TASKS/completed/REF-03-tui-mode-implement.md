# REF-03: Implement `TuiMode` — migrate `App` state into runtime structs

**Status:** Blocked on REF-02  
**Date:** 2026-02-18  
**ADR:** [`TASKS/completed/ADR-006-runtime-mode-contracts.md`](ADR-006-runtime-mode-contracts.md)  
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

**On `RuntimeContext` shape:** REF-02's current stub defines
`RuntimeContext<'a>` with a borrowed `ConversationManager` field. This task
uses that borrowed shape. `dummy_ctx(&mut conversation)` below constructs that
borrowed form.

No new files. No changes outside `src/app/mod.rs`.

---

## Target file

`src/app/mod.rs` only.

---

## Import paths

All `UiUpdate` and `ToolApprovalRequest` references in this task come from
the `runtime` and `state` modules respectively. Do not import from
`crate::types`.

```rust
use crate::runtime::UiUpdate;           // defined in src/runtime/update.rs (REF-02)
use crate::runtime::mode::RuntimeMode;
use crate::runtime::context::RuntimeContext;
use crate::state::ToolApprovalRequest;  // defined in src/state/conversation.rs
```

If `ToolApprovalRequest` does not resolve from `crate::state`, search for
its definition:

```bash
grep -rn "pub struct ToolApprovalRequest" src/
```

Use whatever path the search returns.

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
    ToolPermission(crate::state::ToolApprovalRequest),
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
    /// Index into `history.messages` for the active assistant turn slot.
    /// Set when a new user input is accepted; cleared on TurnComplete/Error.
    current_assistant_msg: Option<usize>,
}

impl TuiMode {
    pub fn new() -> Self {
        Self {
            history: HistoryState { messages: Vec::new(), scroll: 0 },
            input: InputState { buffer: String::new(), cursor_byte: 0 },
            overlay: None,
            turn_in_progress: false,
            current_assistant_msg: None,
        }
    }
}
```

### `RuntimeMode` implementation

```rust
use crate::runtime::UiUpdate;
use crate::runtime::mode::RuntimeMode;
use crate::runtime::context::RuntimeContext;

impl RuntimeMode for TuiMode {
    fn on_user_input(&mut self, input: String, _ctx: &mut RuntimeContext) {
        // CORE-10: overlay active → reject all normal input
        if self.overlay.is_some() {
            return;
        }
        // Turn guard: only one turn in flight at a time
        if self.turn_in_progress {
            return;
        }
        self.turn_in_progress = true;

        // Reserve a dedicated message slot for this turn so successive turns
        // are never merged into the same history entry.
        self.history.messages.push(format!("> {}", input.trim()));
        let slot = self.history.messages.len();
        self.history.messages.push(String::new());
        self.current_assistant_msg = Some(slot);

        // TODO(REF-04): replace the two lines below with ctx.start_turn(input)
        // once RuntimeContext is wired to the API dispatch path.
        let _ = input;
        let _ = _ctx;
    }

    fn on_model_update(&mut self, update: UiUpdate, _ctx: &mut RuntimeContext) {
        match update {
            UiUpdate::StreamDelta(text) => {
                // Write into the reserved turn slot, not the last message.
                let idx = match self.current_assistant_msg {
                    Some(idx) => idx,
                    None => {
                        // Streaming arrived before on_user_input — allocate on the fly.
                        let idx = self.history.messages.len();
                        self.history.messages.push(String::new());
                        self.current_assistant_msg = Some(idx);
                        idx
                    }
                };
                if idx < self.history.messages.len() {
                    self.history.messages[idx].push_str(&text);
                }
            }
            UiUpdate::ToolApprovalRequest(req) => {
                // CORE-11: one-shot guarantee.
                // Only accept the first request per overlay lifecycle.
                // Dropping `req` closes response_tx, which the sender treats as denial.
                if self.overlay.is_none() {
                    self.overlay = Some(OverlayState {
                        kind: OverlayKind::ToolPermission(req),
                    });
                }
            }
            UiUpdate::TurnComplete => {
                self.turn_in_progress = false;
                self.current_assistant_msg = None;
            }
            UiUpdate::Error(msg) => {
                self.history.messages.push(format!("[error] {msg}"));
                self.turn_in_progress = false;
                self.current_assistant_msg = None;
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

Add inside the existing `#[cfg(test)]` block at the bottom of `src/app/mod.rs`.

`dummy_ctx()` constructs the borrowed `RuntimeContext<'a>` shape that exists in
`src/runtime/context.rs` today.

```rust
#[cfg(test)]
fn dummy_ctx<'a>(
    conversation: &'a mut crate::state::ConversationManager,
) -> crate::runtime::context::RuntimeContext<'a> {
    crate::runtime::context::RuntimeContext { conversation }
}
```

Use `MockApiClient` from `src/api/mock_client.rs` when building test
`ConversationManager` instances for this anchor test.

```rust
#[tokio::test]
async fn test_ref_03_tui_mode_overlay_blocks_input() {
    use crate::api::{mock_client::MockApiClient, ApiClient};
    use crate::runtime::UiUpdate;
    use crate::runtime::mode::RuntimeMode;
    use crate::state::ToolApprovalRequest;
    use std::collections::HashMap;
    use std::sync::Arc;

    let mock_api_client = ApiClient::new_mock(Arc::new(MockApiClient::new(vec![])));
    let mut conversation = crate::state::ConversationManager::new_mock(
        mock_api_client,
        HashMap::new(),
    );
    let mut ctx = dummy_ctx(&mut conversation);

    let mut mode = TuiMode::new();
    assert!(!mode.is_turn_in_progress());

    let (resp_tx, _resp_rx) = tokio::sync::oneshot::channel();
    let req = ToolApprovalRequest {
        tool_name: "read_file".to_string(),
        input_preview: "{}".to_string(),
        response_tx: resp_tx,
    };
    mode.on_model_update(UiUpdate::ToolApprovalRequest(req), &mut ctx);

    // Overlay is now active — on_user_input must be a no-op.
    assert!(mode.overlay.is_some());
    mode.on_user_input("should be ignored".to_string(), &mut ctx);
    assert!(
        !mode.is_turn_in_progress(),
        "turn must not start while overlay is active"
    );

    // Clear overlay (simulating dismissal).
    mode.overlay = None;
    mode.on_user_input("now accepted".to_string(), &mut ctx);
    assert!(
        mode.is_turn_in_progress(),
        "turn should start after overlay cleared"
    );
}
```

---

## Verification

```bash
cargo test test_ref_03_tui_mode_overlay_blocks_input -- --nocapture
cargo test --all   # all prior tests must remain green
```

---

## Definition of done

- [ ] `TuiMode`, `HistoryState`, `InputState`, `OverlayState`, `OverlayKind` defined in `src/app/mod.rs`.
- [ ] `RuntimeMode for TuiMode` implemented with overlay block, turn guard, and per-turn message slot.
- [ ] All `UiUpdate` imports use `crate::runtime::UiUpdate` — not `crate::types::UiUpdate`.
- [ ] All `ToolApprovalRequest` imports use `crate::state::ToolApprovalRequest` — not `crate::types::ToolApprovalRequest`.
- [ ] `dummy_ctx(&mut conversation)` returns borrowed `RuntimeContext<'a>`.
- [ ] `test_ref_03_tui_mode_overlay_blocks_input` passes without a real TTY or real API calls.
- [ ] `cargo test --all` is green.
- [ ] `test_ref_02_runtime_types_compile` still passes (do not break REF-02).
- [ ] No new files created outside `src/app/mod.rs`.

---

## What NOT to do

- Do not import `UiUpdate` or `ToolApprovalRequest` from `crate::types`. Both
  live in `crate::runtime` and `crate::state` respectively after REF-02.
- Do not call `ctx.start_turn()` in a way that makes a real API request — it
  is a no-op stub until REF-04 wires it.
- Do not move `App`'s existing ratatui draw loop — that is REF-05's job.
- Do not implement `FrontendAdapter` — that is REF-06's job.
- Do not add CLI flags or environment variables.
