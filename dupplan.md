# Duplication Resolution Plan (dupplan.md)

## Overview
Analysis of the codebase has revealed significant code duplication across several crates. It appears that logic was copied between `ragent-agent` and other specialized crates (`ragent-config`, `ragent-team`, `ragent-tools-extended`) rather than being properly factored into shared libraries.

## Identified Duplications

### 1. Configuration Logic
- **Symbol**: `InternalLlmDownloadPolicy`
- **Locations**:
  - `crates/ragent-config/src/config.rs`
  - `crates/ragent-agent/src/config/mod.rs`
- **Issue**: Duplicate definition of configuration enums and potentially the logic surrounding them.

### 2. Memory Storage Logic
- **Symbol**: `load_all_blocks` and related memory storage functions.
- **Locations**:
  - `crates/ragent-tools-extended/src/memory/storage.rs`
  - `crates/ragent-agent/src/memory/storage.rs`
- **Issue**: The entire memory storage implementation seems to be duplicated.

### 3. Team Store Logic
- **Symbol**: `load_by_name` and team store management.
- **Locations**:
  - `crates/ragent-team/src/team/store.rs`
  - `crates/ragent-agent/src/team/store.rs`
- **Issue**: Team state persistence logic is duplicated.

### 4. List Management (Bash/Dir Lists)
- **Symbol**: `load_from_config`
- **Locations**:
  - `crates/ragent-config/src/bash_lists.rs` & `crates/ragent-config/src/dir_lists.rs`
  - `crates/ragent-agent/src/bash_lists.rs` & `crates/ragent-agent/src/dir_lists.rs`
- **Issue**: Logic for loading safe/banned lists from configuration is duplicated.

---

## Milestones & Tasks

### Milestone 1: Configuration & List Consolidation
**Goal**: Move all configuration and list-related logic into `ragent-config`.

- [ ] **Task 1.1**: Audit `ragent-agent/src/config/mod.rs` vs `ragent-config/src/config.rs`. Remove duplicates from `ragent-agent` and update imports.
- [ ] **Task 1.2**: Remove `ragent-agent/src/bash_lists.rs` and `ragent-agent/src/dir_lists.rs`. Update all references to use the versions in `ragent-config`.
- [ ] **Task 1.3**: Verify that `ragent-config` is correctly exported and used as a dependency in `ragent-agent`.

### Milestone 2: Memory Storage Refactoring
**Goal**: Centralize memory storage in a single location (preferably `ragent-storage` or `ragent-tools-extended`).

- [ ] **Task 2.1**: Compare `ragent-agent/src/memory/storage.rs` and `ragent-tools-extended/src/memory/storage.rs`.
- [ ] **Task 2.2**: Determine the "source of truth" (likely `ragent-tools-extended` or a new `ragent-storage` module).
- [ ] **Task 2.3**: Delete the duplicate implementation in `ragent-agent` and update the dependency graph.

### Milestone 3: Team Store Consolidation
**Goal**: Move all team persistence logic into `ragent-team`.

- [ ] **Task 3.1**: Compare `ragent-agent/src/team/store.rs` and `ragent-team/src/team/store.rs`.
- [ ] **Task 3.2**: Remove the `team/store.rs` module from `ragent-agent`.
- [ ] **Task 3.3**: Update `ragent-agent` to depend on `ragent-team` for all team-related storage operations.

### Milestone 4: Verification & Cleanup
**Goal**: Ensure no regressions and a clean build.

- [ ] **Task 4.1**: Run `cargo check` across the entire workspace.
- [ ] **Task 4.2**: Run all tests in `ragent-agent`, `ragent-config`, `ragent-team`, and `ragent-tools-extended`.
- [ ] **Task 4.3**: Perform a final grep search for the identified duplicate symbols to ensure they only exist in one place.
