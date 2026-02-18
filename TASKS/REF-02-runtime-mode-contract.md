# REF-02: Define core runtime contracts as stub types

**Status:** Active  
**Date:** 2026-02-18  
**ADR:** [`docs/adr/ADR-006-runtime-mode-contracts.md`](../docs/adr/ADR-006-runtime-mode-contracts.md)  
**Depends on:** Nothing (first in REF track)  
**Blocks:** REF-03

---

## Context

ADR-006 locks the four contracts that decouple the TUI from the conversation loop:
`RuntimeMode`, `RuntimeContext`, `RuntimeEvent`, `FrontendAdapter`, and the
`Runtime<M>` loop struct. These must exist as compilable types before any
implementation work begins.

This task creates the stub module only. No logic, no wiring, no changes to
`src/app/mod.rs`. The types are empty or contain `todo!()` bodies where needed.

---

## Target files

- `src/runtime/mod.rs` — module declarations and re-exports
- `src/runtime/mode.rs` — `RuntimeMode` trait
- `src/runtime/context.rs` — `RuntimeContext<'a>`
- `src/runtime/event.rs` — `RuntimeEvent` enum
- `src/runtime/frontend.rs` — `FrontendAdapter` trait
- `src/runtime/loop.rs` — `Runtime<M: RuntimeMode>` struct
- `src/lib.rs` — add `pub mod runtime;`

Do not touch `src/app/mod.rs` or any file outside `src/runtime/` and `src/lib.rs`.

---

## Exact types to define

Copy these signatures verbatim. Do not add fields or methods not listed here.

```rust
// src/runtime/mode.rs
use crate::state::conversation::ConversationManager;
use crate::types::UiUpdate;
use super::context::RuntimeContext;

pub trait RuntimeMode {
    fn on_user_input(&mut self, input: String, ctx: &mut RuntimeContext);
    fn on_model_update(&mut self, update: UiUpdate, ctx: &mut RuntimeContext);
    fn is_turn_in_progress(&self) -> bool;
}
```

```rust
// src/runtime/context.rs
use crate::state::conversation::ConversationManager;
use crate::types::UiUpdate;
use tokio::sync::mpsc;

pub struct RuntimeContext<'a> {
    pub conversation: &'a mut ConversationManager,
}

impl<'a> RuntimeContext<'a> {
    pub fn start_turn(&mut self, _input: String, _tx: mpsc::UnboundedSender<UiUpdate>) {
        todo!("wired in REF-04")
    }
    pub fn cancel_turn(&mut self) {
        todo!("wired in REF-04")
    }
}
```

```rust
// src/runtime/event.rs
use crate::types::ToolApprovalRequest;

pub enum RuntimeEvent {
    TurnStarted { id: u64 },
    StreamDelta { text: String },
    ToolApprovalRequest(ToolApprovalRequest),
    TurnComplete,
    Error(String),
}
```

```rust
// src/runtime/frontend.rs
use super::mode::RuntimeMode;

pub trait FrontendAdapter {
    fn poll_user_input(&mut self) -> Option<String>;
    fn render<M: RuntimeMode>(&mut self, mode: &M);
    fn should_quit(&self) -> bool;
}
```

```rust
// src/runtime/loop.rs
use crate::types::UiUpdate;
use tokio::sync::mpsc;
use super::{context::RuntimeContext, frontend::FrontendAdapter, mode::RuntimeMode};

pub struct Runtime<M: RuntimeMode> {
    pub mode: M,
    update_rx: mpsc::UnboundedReceiver<UiUpdate>,
}

impl<M: RuntimeMode> Runtime<M> {
    pub fn new(mode: M, update_rx: mpsc::UnboundedReceiver<UiUpdate>) -> Self {
        Self { mode, update_rx }
    }
    // run() wired in REF-05
}
```

---

## Anchor test

Add this test at the bottom of `src/runtime/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    // Verify all runtime contract types compile and are importable.
    use super::{
        context::RuntimeContext,
        event::RuntimeEvent,
        frontend::FrontendAdapter,
        mode::RuntimeMode,
    };

    #[test]
    fn test_ref_02_runtime_types_compile() {
        // Types exist and the module compiles. No logic to assert.
        let _ = std::mem::size_of::<RuntimeEvent>();
    }
}
```

---

## Verification after CI check

```bash
cargo check --all-targets
cargo test test_ref_02_runtime_types_compile -- --nocapture
cargo test --all   # all prior tests must remain green
```

Also run:

```bash
cargo check --target x86_64-unknown-linux-musl -p aistar \
  --manifest-path Cargo.toml 2>&1 | grep "src/runtime"
```

This must produce no errors related to `ratatui` or `crossterm` — those crates
must not appear in `src/runtime/` imports.

---

## Definition of done

- [ ] `src/runtime/` module exists with all five files.
- [ ] `pub mod runtime;` declared in `src/lib.rs`.
- [ ] `test_ref_02_runtime_types_compile` passes.
- [ ] `cargo test --all` is green.
- [ ] No `ratatui`/`crossterm` imports in `src/runtime/`.
- [ ] `src/app/mod.rs` is unchanged (diff must be empty for that file).
