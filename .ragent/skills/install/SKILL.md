---
description: "Increment patch version, commit, build and install"
user-invocable: true
disable-model-invocation: true
argument-hint: "[commit message]"
---

Perform a release of the ragent project by following these steps **in order**:

1. **Run `cargo build --release`** to build and install the new version of ragent locally.

2. **Run `cp target/release/ragent ~/bin/`** to copy the new binary to a location in the system PATH.

3. **Report** "Completed"