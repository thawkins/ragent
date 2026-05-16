---
description: "Increment patch version, commit, build and install"
user-invocable: true
disable-model-invocation: true
argument-hint: "[commit message]"
---

Perform a release of the ragent project by following these steps **in order**:

1. **Read the current version** from the workspace `Cargo.toml` (the `version = "..."` line near the top).

2. **Increment the least-significant version digit** by exactly 1.
   - For a version like `0.1.0-alpha.9`, increment the pre-release number → `0.1.0-alpha.10`.
   - For a version like `1.2.3`, increment the patch number → `1.2.4`.
   - For a version like `0.1.0-beta.2`, increment → `0.1.0-beta.3`.

3. **Update `Cargo.toml`** with the new version string (the workspace-level `version` field only).

4. **Run `cargo build --release`** to build and install the new version of ragent locally.

5. **Run `cp target/release/ragent ~/bin/`** to copy the new binary to a location in the system PATH.

6. **Report** "Completed"