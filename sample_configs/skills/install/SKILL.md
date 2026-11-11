---
description: "Build and install ragent locally (cargo install --path .); does not bump the version or commit"
user-invocable: true
disable-model-invocation: true
allowed-tools:
 - any
---
Build and install the current ragent working tree locally by following these steps **in order**:

In the project root directory:

1. **Run `cargo install --path .`** to build and install the new version of ragent locally.
2. **Report** "Completed"
