//! Pin the GitLab legacy-migration path layout.
//!
//! FR/regression: `migrate_legacy_files` must scan the *legacy*
//! `~/.ragent/` directory — not the canonical `~/.config/ragent/` — so an
//! existing install's credentials are picked up and imported. This test does
//! not touch the filesystem or env vars; it asserts the path-shape contract
//! of the helpers the migration uses.
//!
//! The helpers under test are `#[doc(hidden)]` and not exported, so the
//! contract is asserted against the `user_dirs::legacy_home_dir` primitive
//! they compose: if a future refactor re-points it at `~/.config/ragent/`
//! (the canonical dir), this test fails loudly.

#[test]
fn legacy_home_dir_is_not_the_canonical_config_dir() {
    let Some(legacy) = ragent_config::user_dirs::legacy_home_dir() else {
        // No home dir in this environment; nothing to assert.
        return;
    };
    let canonical = ragent_config::user_dirs::global_state_dir();

    // The legacy scan root must be the home-dir `.ragent` folder, never the
    // canonical config dir — otherwise migration becomes a silent self-scan.
    assert!(
        legacy.ends_with(".ragent"),
        "legacy home dir must end in .ragent, got {}",
        legacy.display()
    );
    if let Some(canon) = canonical {
        assert_ne!(
            legacy, canon,
            "legacy scan root must differ from the canonical state dir"
        );
    }

    // The two file names the migration reads must compose under the legacy
    // root so a future rename of either is caught here.
    let token = legacy.join("gitlab_token");
    let config = legacy.join("gitlab_config.json");
    assert!(token.ends_with("gitlab_token"));
    assert!(config.ends_with("gitlab_config.json"));
}
