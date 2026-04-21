# Config File Location Migration — v0.1.0-alpha.47

## Summary

Moved the default location of `ragent.json` from the project root (`./ragent.json`) to the `.ragent/` subdirectory (`./.ragent/ragent.json`). This change reduces root directory clutter and consolidates all ragent-related files in one hidden directory.

## Changes Made

### Core Configuration Loading

**File:** `crates/ragent-core/src/config/mod.rs`
- Line 507-512: Changed project config path from `PathBuf::from("ragent.json")` to `PathBuf::from(".ragent").join("ragent.json")`

### Bash Command Lists

**File:** `crates/ragent-core/src/bash_lists.rs`
- Line 3-4: Updated module documentation from `./ragent.json` to `./.ragent/ragent.json`
- Line 108: Updated doc comment for `Scope::Project` variant
- Line 117: Changed `config_path()` implementation to return `.ragent/ragent.json` for project scope

### Directory Permission Lists

**File:** `crates/ragent-core/src/dir_lists.rs`
- Line 3-4: Updated module documentation from `./ragent.json` to `./.ragent/ragent.json`
- Line 154: Updated doc comment for `Scope::Project` variant
- Line 163: Changed `config_path()` implementation to return `.ragent/ragent.json` for project scope

### TUI Application

**File:** `crates/ragent-tui/src/app.rs`
- Line 2323: Updated LSP server toggle to use `.ragent/ragent.json`
- Line 2370: Updated LSP server addition to use `.ragent/ragent.json`
- Line 2412: Updated MCP server addition to use `.ragent/ragent.json`
- Line 6311: Updated `/bash` command help text
- Line 6320: Updated `/bash` command documentation
- Line 6434: Updated config file display string
- Line 6564: Updated `/dirs` command help text flags documentation
- Line 6682: Updated config file display string for dirs commands
- Line 8686: Updated AIWiki autosync config persistence
- Line 8706: Updated AIWiki autosync config persistence

### Documentation

**File:** `README.md`
- Line 120-122: Updated configuration section to reference `.ragent/` directory

### Tests

**File:** `crates/ragent-core/tests/test_dir_lists.rs`
- All test functions updated to:
  1. Create `.ragent` subdirectory in temp test directories
  2. Place `ragent.json` inside `.ragent/` instead of root
  3. Updated comments referencing the config location

### Project Migration

- Created `.ragent/` directory in project root
- Moved `ragent.json` from project root to `.ragent/ragent.json`
- Created `CONFIG_MIGRATION.md` user guide

## Config Loading Order (Updated)

1. **Defaults** (hardcoded)
2. **Global config:** `~/.config/ragent/ragent.json` (unchanged)
3. **Project config:** `./.ragent/ragent.json` ← **changed from `./ragent.json`**
4. **Environment variable:** `$RAGENT_CONFIG` (file path)
5. **Inline JSON:** `$RAGENT_CONFIG_CONTENT`

## Breaking Change Notice

⚠️ **This is a breaking change.** Projects using `./ragent.json` must migrate to `./.ragent/ragent.json`.

### Migration Command

```bash
mkdir -p .ragent
mv ragent.json .ragent/
```

## Files That Write Config

The following runtime commands now write to `.ragent/ragent.json` (project scope):

- `/bash add allow|deny` (without `--global`)
- `/dirs add allow|deny` (without `--global`)
- `/lsp enable|disable <server>`
- `/mcp enable|disable <server>`
- `/aiwiki autosync on|off`

Global scope commands (`--global` flag) continue to write to `~/.config/ragent/ragent.json`.

## Testing Status

- Core config loading: ✅ Compiles
- Bash lists scope: ✅ Implementation complete
- Dir lists scope: ✅ Implementation complete
- TUI config updates: ✅ Implementation complete
- Unit tests: ⚠️  Need updates (7 tests in `test_dir_lists.rs` require fixes)

## Known Issues

The tests in `crates/ragent-core/tests/test_dir_lists.rs` are currently failing because:
1. They create `.ragent/` directory and config file correctly
2. But `add_allowlist`/`add_denylist` functions may not be finding the config after the path change
3. Needs investigation of why `patch_config` is not seeing the newly created config files

## Next Steps

1. Debug and fix `test_dir_lists.rs` test failures
2. Update CHANGELOG.md with breaking change notice
3. Update SPEC.md with new config file location
4. Test end-to-end config loading and persistence
5. Consider adding migration helper in CLI (`ragent config migrate`)

## Related Files

- `CONFIG_MIGRATION.md` — User migration guide (created)
- `.ragent/ragent.json` — Project config (migrated)
- `ragent.json` — Old location (removed)
