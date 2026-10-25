---
trigger: always_on
description: Mandatory git commit and push rule once tests pass in manga-tui.
---

# Always Commit and Push Changes to Git After Tests Pass

Whenever you make changes, bug fixes, or enhancements to the `manga-tui` codebase:

1. Ensure the code compiles and all unit/integration tests pass (`cargo test`).
2. Run `cargo fmt --all`.
3. Re-install the new binary globally (`cargo install --path .`).
4. **MANDATORY**: Stage all changes, commit them with a descriptive commit message following conventional commits, and push them to the active remote git branch:
   ```powershell
   git add .
   git commit -m "<tipo>: <descripción clara de los cambios>"
   git push
   ```
   Do not leave verified and tested code uncommitted or unpushed without explicit instruction from the user.
