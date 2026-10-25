---
trigger: always_on
description: Mandatory binary reinstallation rule after code modifications in manga-tui.
---

# Always Install Updated Binary

Whenever you make changes, bug fixes, or enhancements to the `manga-tui` codebase:

1. Always verify the code compiles (`cargo check` or `cargo test`).
2. Run `cargo fmt --all`.
3. **MANDATORY**: Re-install the new binary globally into the user's environment by running:
   ```powershell
   cargo install --path .
   ```
   This ensures that whenever the user runs `manga-tui` in their command prompt or terminal, they are immediately running the newly modified version.
