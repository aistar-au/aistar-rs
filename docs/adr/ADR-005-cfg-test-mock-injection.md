# ADR-005: `#[cfg(test)]` mock injection field on production `ApiClient` struct

**Date:** 2026-02-18  
**Status:** Accepted  
**Deciders:** Core maintainer  
**Related tasks:** `TASKS/CRIT-01-protocol.md`  
**Implemented in:** `src/api/client.rs` — `mock_stream_producer` field; `src/api/mock_client.rs`

---

## Context

`test_crit_01_protocol_flow` and related multi-turn tests in `src/state/conversation.rs` require a controllable, deterministic API client that returns pre-scripted SSE streams without making real HTTP requests.

Rust provides two conventional approaches to this:

1. **Trait objects for dependency injection** — define an `ApiBackend` trait and inject a mock implementation in tests.
2. **Conditional compilation via `#[cfg(test)]`** — add test-only fields and constructors to the production struct.

---

## Decision

Use `#[cfg(test)]` mock injection: the production `ApiClient` struct carries an optional `mock_stream_producer` field, present only when compiled with the `test` profile. A `new_mock()` constructor populates this field. At call time, `create_stream()` checks the field and short-circuits the HTTP request if a mock is present.

```rust
#[derive(Clone)]
pub struct ApiClient {
    // ... production fields ...
    #[cfg(test)]
    mock_stream_producer: Option<Arc<dyn MockStreamProducer>>,
}
```

The `MockStreamProducer` trait and `MockApiClient` implementation live in `src/api/mock_client.rs`, which is compiled only under `#[cfg(test)]`.

---

## Rationale

### Why not a trait for the full client?

A trait-based approach requires `ConversationManager` to hold a `Box<dyn ApiBackend>` (or a generic `<T: ApiBackend>`) instead of a concrete `ApiClient`. This propagates the generic or the box through `App` as well, since `App` owns `ConversationManager`.

At v0.1.0-alpha the cost of this propagation is disproportionate to the benefit. The `#[cfg(test)]` approach keeps the production type concrete and avoids dynamic dispatch or generic bounds proliferating through the call stack. `ApiClient` is `Clone`, which a trait object (`Box<dyn Trait>`) is not without additional bounds.

### Why not a separate `TestApiClient` type?

A separate type would require `ConversationManager::new()` to accept either type — again requiring a trait or an enum. The mock field is invisible in production binaries (confirmed by the release build stripping all `#[cfg(test)]` blocks) and does not affect the production API surface.

### Precedent

This pattern appears in several production Rust codebases where full trait extraction would require pervasive generics. It is acknowledged as a pragmatic tradeoff, not a permanent ideal.

---

## Alternatives considered

### Full trait extraction: `trait ApiStream`

```rust
trait ApiStream: Send {
    async fn create_stream(&self, messages: &[ApiMessage]) -> Result<ByteStream>;
}
struct ApiClient { ... }
impl ApiStream for ApiClient { ... }
struct MockApiClient { ... }
impl ApiStream for MockApiClient { ... }
```

Cleanest long-term design. Deferred because it would require `ConversationManager<T: ApiStream>` which propagates to `App<T: ApiStream>` and makes the type system harder to reason about at this stage. Planned as a follow-up for v0.2.0 as part of the REF track (see ADR-004).

### `mockall` crate

Generates trait mock implementations via proc macro. Introduces a dev-dependency and requires the trait extraction described above. Deferred for the same reason.

### HTTP-level mocking (e.g., `wiremock`)

Intercepts real HTTP calls at the transport layer. Does not require any changes to `ApiClient` but is significantly slower (real network stack, server spin-up) and makes tests dependent on port availability. Inappropriate for unit tests of conversation protocol logic.

---

## Consequences

**Easier:**
- Multi-turn protocol tests are fast, deterministic, and self-contained.
- No generics or trait objects leak into `ConversationManager` or `App` at v0.1.0.
- `MockApiClient` can precisely script SSE chunk sequences, including fragmented packets, to test the stream parser under realistic conditions.

**Harder:**
- The production `ApiClient` struct has a field that only exists in test builds. This is a minor cognitive burden for contributors reading the code.
- If a test accidentally constructs an `ApiClient::new()` instead of `ApiClient::new_mock()`, the mock field is `None` and the test will attempt a real HTTP request — and fail in CI. The `new_mock()` constructor name makes this mistake visible but not impossible.
- `ApiClient` cannot be `#[derive(Debug)]` without `MockStreamProducer: Debug`. Current workaround: `Debug` is not derived.

**Planned migration path:**
When the REF track reaches REF-05 (generic runtime loop), extract `ApiStream` as a proper trait at that point. The `#[cfg(test)]` field is then replaced by a generic parameter at the `ConversationManager` level, and this ADR is superseded.

**Constraints imposed on future work:**
- Do not add additional `#[cfg(test)]` fields to `ApiClient`. If a second injectable dependency is needed, that is the signal to do the trait extraction instead.
- `src/api/mock_client.rs` must remain gated behind `#[cfg(test)]`. It must never be compiled into release builds.
- Any new test that uses `ConversationManager` with network calls must use `ApiClient::new_mock()`. Tests that reach the real Anthropic API are integration tests and must be gated behind a feature flag or ignored by default.
