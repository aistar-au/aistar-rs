# Test-Driven Manifest (TDM) Strategy

## Overview

The Test-Driven Manifest strategy is designed for **small context window agents** (8k tokens or less). It provides a binary definition of success: the task is done when `cargo test` returns green.

## The Problem

When working with AI agents that have limited context windows:

1. **Hallucination:** Agents may "guess" that a fix works without verification
2. **Token Waste:** Loading entire codebases consumes precious context
3. **Merge Conflicts:** Multiple agents working on the same files cause conflicts
4. **Ambiguity:** "It should work" is not a clear success criteria

## The Solution: Anchor Tests

An **Anchor Test** is a failing regression test that defines success:

```rust
#[test]
fn test_crit_02_regression() {
    // This test FAILS before the fix, PASSES after the fix
    let msg = ApiMessage { 
        role: "user".into(), 
        content: Content::Text("Hello".into()) 
    };
    let serialized = serde_json::to_value(&msg).unwrap();
    assert!(serialized.get("content").is_some(), "Missing 'content' key!");
}
```

### Properties of a Good Anchor

1. **Minimal:** Only tests the specific bug, nothing else
2. **Named with Task ID:** `test_crit_02_regression` links back to `CRIT-02`
3. **Self-Documenting:** The assertion message explains what's wrong
4. **Fast:** Unit test, not integration test

## Workflow Steps

### Step 1: Create the Task File

Each task gets its own file in `TASKS/`:

```markdown
# Task CRIT-XX: Brief Description
**Target File:** `src/path/to/file.rs`
**Issue:** One-sentence description of the bug.
**Definition of Done:**
1. Specific action 1.
2. Anchor test passes.
**Anchor Test:** `test_crit_XX_regression` in `src/path/to/file.rs`
```

### Step 2: Create the Anchor

Add a failing test to the target file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crit_XX_regression() {
        // ANCHOR: This will FAIL until the bug is fixed
        assert!(false, "Not yet implemented");
    }
}
```

### Step 3: Run the Test

```bash
cargo test test_crit_XX_regression -- --nocapture
# Expected: FAILED
```

### Step 4: Prompt the Agent

```
I need you to fix TASKS/CRIT-XX-description.md.
1. Read src/path/to/file.rs.
2. Look at the failing test test_crit_XX_regression.
3. Modify the code to make the test pass.
4. Run cargo test and iterate until the test passes.
5. Do not touch any other files.
```

### Step 5: Verify and Merge

```bash
cargo test test_crit_XX_regression
# Expected: PASSED
```

## Token Efficiency

| Approach | Tokens Used |
|----------|-------------|
| Load entire codebase | ~50,000+ |
| Load task file + target file | ~300-500 |
| **Savings** | **99%** |

## Modular Safety

When tasks target different files, agents cannot conflict:

```
Agent A: TASKS/CRIT-02 -> src/types/api.rs
Agent B: TASKS/CRIT-03 -> src/app/mod.rs
```

Each agent only reads and writes its assigned file.

## Example: CRIT-02 Serde Fix

### Task File (`TASKS/CRIT-02-serde-fix.md`)

```markdown
# Task CRIT-02: Serde Serialization Repair
**Target File:** `src/types/api.rs`
**Issue:** The `ApiMessage` struct uses `#[serde(flatten)]` on the `content` field.
This causes serialization to fail with "can only flatten structs and maps".
**Definition of Done:**
1. Remove `#[serde(flatten)]` from line 6.
2. Test `test_crit_02_regression` passes.
```

### Anchor Test

```rust
#[test]
fn test_crit_02_regression() {
    let msg = ApiMessage { 
        role: "user".into(), 
        content: Content::Text("Hello".into()) 
    };
    let serialized = serde_json::to_value(&msg).unwrap();
    assert!(serialized.get("content").is_some());
}
```

### Before Fix

```
test types::api::tests::test_crit_02_regression ... FAILED
Error: "can only flatten structs and maps (got a string)"
```

### After Fix

```
test types::api::tests::test_crit_02_regression ... ok
```

## Best Practices

1. **One Task Per File:** Don't combine multiple issues
2. **Minimal Context:** Only include what's necessary
3. **Clear Target:** Specify exact file and line numbers
4. **Test First:** Always create the anchor before the fix
5. **No Side Effects:** Tasks should not modify files outside their target

## Integration with mdbook

Add this document to your `SUMMARY.md`:

```markdown
# Contributor Guide
- [Development Setup](./dev/setup.md)
- [Agentic Repair Strategy](./dev/manifest-strategy.md)
```

This keeps user documentation clean while providing developers (and agents) with the workflow details.