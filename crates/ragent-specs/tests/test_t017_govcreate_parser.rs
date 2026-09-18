//! Unit tests for the govcreate argument parser (spec `govdoc` T-017:
//! FR-002, FR-003, FR-017, NFR-002).
//!
//! These complement the per-task parser suite (`test_govcreate_parse`) with
//! the acceptance angles that suite missed at the time it was written:
//! argument-order confusion with interleaved flags, flag-value confusion, the
//! full usage-block surface contract (FR-003), and `--force` binding with no
//! `/new` flags (FR-017).

use ragent_specs::SpecCommand;

#[test]
fn test_t017_flag_token_after_the_positionals_ends_the_positional_run() {
    // The positional contract is "three tokens, then `/new` flags" (A3). A
    // flag token must terminate the positional run, never be treated as a
    // fourth positional; a misordered invocation with an interleaved flag is
    // a missing-positional usage error, not a half-parsed run.
    let cmd = SpecCommand::parse(
        "govcreate payments-arch --language rust doc/sad.md ./payments --type cmdline",
    );
    assert!(cmd.is_usage_error(), "must be usage: {cmd:?}");
}

#[test]
fn test_t017_all_flags_after_positionals_parse_together() {
    // Order *within* the flag run is free: a valid invocation with several
    // flags must parse regardless of flag order.
    let forward = SpecCommand::parse(
        "govcreate payments-arch doc/sad.md ./payments --language rust --type cmdline --force",
    );
    let reverse = SpecCommand::parse(
        "govcreate payments-arch doc/sad.md ./payments --type cmdline --language rust --force",
    );
    let (
        SpecCommand::GovCreate {
            spec_id: f_id,
            content_ref: f_ref,
            target_folder: f_target,
            force: f_force,
            ..
        },
        SpecCommand::GovCreate {
            spec_id: r_id,
            content_ref: r_ref,
            target_folder: r_target,
            force: r_force,
            ..
        },
    ) = (forward, reverse)
    else {
        panic!("both orderings must parse");
    };
    assert_eq!(f_id, "payments-arch");
    assert_eq!(f_ref, "doc/sad.md");
    assert_eq!(f_target, "./payments");
    assert!(f_force);
    assert_eq!(f_id, r_id);
    assert_eq!(f_ref, r_ref);
    assert_eq!(f_target, r_target);
    assert_eq!(f_force, r_force);
}

#[test]
fn test_t017_content_ref_named_like_a_flag_value_is_not_consumed() {
    // `rust`-looking tokens are only flag *values*, never positionals: a
    // content reference named `rust` must still bind positionally.
    let cmd = SpecCommand::parse("govcreate docs rust ./out --language rust --type cmdline");
    let SpecCommand::GovCreate { content_ref, .. } = cmd else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(content_ref, "rust");
}

#[test]
fn test_t017_spec_id_named_like_a_flag_is_usage_error() {
    // FR-002: spec IDs may not begin with `--`; a misordered invocation must
    // surface the usage/help surface rather than a half-parsed run.
    let cmd =
        SpecCommand::parse("govcreate --force doc/sad.md ./out --language rust --type cmdline");
    assert!(cmd.is_usage_error(), "must be a usage error: {cmd:?}");
}

#[test]
fn test_t017_usage_block_is_stable_and_complete() {
    // FR-003 contract: the usage surface names the command, the three
    // positionals, and every accepted /new flag.
    let cmd = SpecCommand::parse("govcreate");
    assert!(
        cmd.is_usage_error(),
        "no-arg invocation must be a usage error"
    );
    let SpecCommand::Unknown(name) = &cmd else {
        panic!("missing positionals must surface Unknown, got {cmd:?}");
    };
    assert_eq!(name, "govcreate");
    // The help block is the contract the dispatch layer renders for a usage
    // error; assert its contents here once so the tokens cannot drift.
    let message = SpecCommand::build_govcreate_help_message();
    for token in [
        "/spec govcreate",
        "<specid>",
        "<content-ref>",
        "<target-folder>",
        "--language",
        "--type",
        "--stack",
        "--github",
        "--gitlab",
        "--force",
    ] {
        assert!(
            message.contains(token),
            "usage must mention '{token}': {message}"
        );
    }
}

#[test]
fn test_t017_force_after_target_alone_sets_force_without_flags() {
    // FR-017: `--force` after the target folder with no /new flags at all
    // must still bind the three positionals and mark the overwrite intent.
    let cmd = SpecCommand::parse("govcreate arch x.md ./out --force");
    let SpecCommand::GovCreate {
        spec_id,
        content_ref,
        target_folder,
        force,
        ..
    } = cmd
    else {
        panic!("expected GovCreate, got {cmd:?}");
    };
    assert_eq!(spec_id, "arch");
    assert_eq!(content_ref, "x.md");
    assert_eq!(target_folder, "./out");
    assert!(force);
}
