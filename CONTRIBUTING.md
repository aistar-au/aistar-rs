# Contributing to aistar

## 🛠️ The Agentic Workflow (TDD Manifest)

We use a Test-Driven Manifest strategy for all bug fixes and features:

1. **Identify Task:** Check the `TASKS/` directory for open items.
2. **Anchor Test:** Every task must have a failing regression test in the codebase before work begins.
3. **Module Isolation:** Work should be confined to the file specified in the task manifest.
4. **Verification:** Success is defined as `cargo test` passing for the anchor.

See `docs/dev/manifest-strategy.md` for the full technical breakdown.

## 📋 Task Naming Convention

| Prefix | Type | Example |
|--------|------|---------|
| `CRIT-XX` | Critical bugs | `CRIT-02-serde-fix.md` |
| `FEAT-XX` | Feature requests | `FEAT-01-streaming-ui.md` |
| `REF-XX` | Refactoring tasks | `REF-01-error-handling.md` |
| `DOC-XX` | Documentation tasks | `DOC-01-api-docs.md` |

## 🚀 Quick Start

```bash
# 1. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# 2. Run tests to verify environment
cargo test

# 3. Pick a task from TASKS/
# Read the task file and implement the fix

# 4. Run the specific anchor test
cargo test test_crit_XX_regression -- --nocapture

# 5. Iterate until the test passes
```

## 📁 Project Structure

```
aistar/
├── CONTRIBUTING.md          # This file
├── TASKS/                   # Active task manifests
│   └── CRIT-XX-*.md        # Individual task files
├── docs/
│   └── dev/
│       └── manifest-strategy.md  # TDD Manifest deep-dive
├── src/
│   ├── api/                # API client code
│   ├── app/                # Application state
│   ├── state/              # Conversation management
│   ├── terminal/           # Terminal setup
│   ├── tools/              # Tool execution
│   ├── types/              # Type definitions
│   └── ui/                 # UI rendering
└── tests/                  # Integration tests
```

## 🔗 Useful Links

- [Development Setup](docs/dev/setup.md)
- [Agentic Repair Strategy](docs/dev/manifest-strategy.md)
- [API Documentation](docs/api.md)
