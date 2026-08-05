---
description: "Increment patch version, commit, push, and tag the release"
user-invocable: true
disable-model-invocation: true
argument-hint: "[commit message]"
---
Perform a release of the ragent project by following these steps **in order**:

1. **Read the current version** from the workspace `Cargo.toml` (the `version = "..."` line near the top).
2. **Increment the least-significant version digit** by exactly 1.

   - For a version like `0.1.0-beta.9`, increment the pre-release number → `0.1.0-beta.10`.
   - For a version like `1.2.3`, increment the patch number → `1.2.4`.
3. **Update `Cargo.toml`** with the new version string (the workspace-level `version` field only).
4. **Run `cargo check`** to ensure the version change doesn't break the build.
5. **Run `cargo audit`** to ensure that there are no new security issues introduced, stop if there are security issues..
6. **Update **`CHANGELOG.md` with the new version number and any recent changes.
7. **Stage all modified files** with `git add -A`.
8. **Commit** with the message: `Version: <new-version>` followed by any additional message the user provided via $ARGUMENTS.
9. **Push** to the remote with `git push`.
10. **Tag** the commit with `v<new-version>` (e.g. `v1.0.0 `) and push the tag with `git push origin v<new-version>`.
11. **Report** the old version, new version, and the tag that was pushed.
