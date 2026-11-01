---
name: ci-preflight
description: >-
  Use this skill before committing code or opening a pull request in manga-tui.
  Executes the full CI verification pipeline (cargo fmt, clippy, check, tests),
  diagnoses compiler and lint warnings, and fixes formatting issues automatically.
---

# CI Preflight & Code Quality Skill

This skill enforces and automates the verification pipeline required by `manga-tui`'s CI configuration before pushing code.

---

## 1. Quick Full CI Execution

If `cargo-make` is installed:
```powershell
cargo make full-ci
```

Or using native `cargo` commands in sequence:
```powershell
cargo fmt --all --check
cargo clippy -- -D warnings
cargo check
cargo build
cargo test
```

---

## 2. Step-by-Step Triage & Automatic Fixes

### Phase 1: Formatting (`cargo fmt`)
Check formatting:
```powershell
cargo fmt --all --check
```
If formatting differences are reported, fix them automatically:
```powershell
cargo fmt --all
```

### Phase 2: Clippy Linter (`cargo clippy -- -D warnings`)
CI treats **all warnings as fatal errors** (`-D warnings`).
Run clippy:
```powershell
cargo clippy -- -D warnings
```

Common Clippy warnings and fixes in this codebase:
- `clippy::needless_borrow`: Remove unnecessary `&` references.
- `clippy::redundant_closure`: Replace `|x| foo(x)` with `foo`.
- `clippy::clone_on_copy`: Remove `.clone()` on types implementing `Copy` (like enums or numbers).
- `clippy::collapsible_if`: Merge nested `if` statements into `if cond1 && cond2`.
- `clippy::manual_unwrap_or_default`: Use `.unwrap_or_default()` instead of matching `None => Default::default()`.

If there are many mechanical lints, attempt automatic fixes:
```powershell
cargo clippy --fix --allow-dirty --allow-staged
```

### Phase 3: Fast Type Check (`cargo check`)
Ensures all feature flags, target dependencies, and module exports compile without running full codegen:
```powershell
cargo check
```

### Phase 4: Unit & Integration Tests
Run core unit tests:
```powershell
cargo test
```

Run ignored tests (includes downloads and live providers):
```powershell
cargo test -- --ignored
```
*(Note: If working in an offline environment, ignored tests requiring active network connections can be skipped or filtered using `cargo test <module>`)*.

---

## 3. Pull Request Readiness Checklist

Before marking the task complete:
- [ ] `cargo fmt --all --check` passes with no output.
- [ ] `cargo clippy -- -D warnings` finishes with 0 warnings.
- [ ] `cargo test` passes 100% of non-ignored unit tests.
- [ ] Any new public methods or structs preserve documentation and existing comments.
- [ ] **Install updated binary**: Run `cargo install --path .` so the user's global `manga-tui` command runs the new version.

