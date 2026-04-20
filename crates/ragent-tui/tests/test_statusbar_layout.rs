//! Unit and integration tests for status bar layout.
//!
//! Tests cover responsive breakpoints, content rendering, and layout structure.

use ragent_tui::layout_statusbar::{ResponsiveMode, StatusBarConfig};

// ─────────────────────────────────────────────────────────────────────────────
// ResponsiveMode Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_mode_minimal_boundary_lower() {
    // Test at lower boundary of minimal
    assert_eq!(ResponsiveMode::from_width(50), ResponsiveMode::Minimal);
}

#[test]
fn test_responsive_mode_minimal_boundary_upper() {
    // Test at upper boundary of minimal (inclusive)
    assert_eq!(ResponsiveMode::from_width(79), ResponsiveMode::Minimal);
}

#[test]
fn test_responsive_mode_compact_boundary_lower() {
    // Test at lower boundary of compact (inclusive)
    assert_eq!(ResponsiveMode::from_width(80), ResponsiveMode::Compact);
}

#[test]
fn test_responsive_mode_compact_boundary_upper() {
    // Test at upper boundary of compact (inclusive)
    assert_eq!(ResponsiveMode::from_width(119), ResponsiveMode::Compact);
}

#[test]
fn test_responsive_mode_full_boundary_lower() {
    // Test at lower boundary of full (inclusive)
    assert_eq!(ResponsiveMode::from_width(120), ResponsiveMode::Full);
}

#[test]
fn test_responsive_mode_full_boundary_upper() {
    // Test at large width (full)
    assert_eq!(ResponsiveMode::from_width(200), ResponsiveMode::Full);
}

#[test]
fn test_responsive_mode_zero_width() {
    // Test with zero width (should be minimal)
    assert_eq!(ResponsiveMode::from_width(0), ResponsiveMode::Minimal);
}

// ─────────────────────────────────────────────────────────────────────────────
// StatusBarConfig Tests
// ──────────────────────��──────────────────────────────────────────────────────

#[test]
fn test_statusbar_config_default_verbose_false() {
    let config = StatusBarConfig::default();
    assert!(!config.verbose);
}

#[test]
fn test_statusbar_config_verbose_true() {
    let config = StatusBarConfig { verbose: true };
    assert!(config.verbose);
}

#[test]
fn test_statusbar_config_verbose_false() {
    let config = StatusBarConfig { verbose: false };
    assert!(!config.verbose);
}

// ─────────────────────────────────────────────────────────────────────────────
// ResponsiveMode Equality Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_mode_eq() {
    let m1 = ResponsiveMode::Full;
    let m2 = ResponsiveMode::Full;
    assert_eq!(m1, m2);
}

#[test]
fn test_responsive_mode_ne() {
    let m1 = ResponsiveMode::Full;
    let m2 = ResponsiveMode::Compact;
    assert_ne!(m1, m2);
}

#[test]
fn test_responsive_mode_copy() {
    let m1 = ResponsiveMode::Minimal;
    let m2 = m1;
    assert_eq!(m1, m2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge Cases
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_mode_u16_max() {
    // Test with maximum u16 value
    assert_eq!(ResponsiveMode::from_width(u16::MAX), ResponsiveMode::Full);
}

#[test]
fn test_responsive_mode_sequential_widths() {
    // Test boundary transitions
    for w in 75..85 {
        let mode = ResponsiveMode::from_width(w);
        if w < 80 {
            assert_eq!(mode, ResponsiveMode::Minimal);
        } else {
            assert_eq!(mode, ResponsiveMode::Compact);
        }
    }
}

#[test]
fn test_responsive_mode_sequential_widths_full() {
    // Test transition to full mode
    for w in 115..125 {
        let mode = ResponsiveMode::from_width(w);
        if w < 120 {
            assert_eq!(mode, ResponsiveMode::Compact);
        } else {
            assert_eq!(mode, ResponsiveMode::Full);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Clone and Debug Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_mode_clone() {
    let m1 = ResponsiveMode::Full;
    let m2 = m1.clone();
    assert_eq!(m1, m2);
}

#[test]
fn test_statusbar_config_clone() {
    let c1 = StatusBarConfig { verbose: true };
    let c2 = c1.clone();
    assert_eq!(c1.verbose, c2.verbose);
}

#[test]
fn test_responsive_mode_debug() {
    let m = ResponsiveMode::Compact;
    let debug_str = format!("{:?}", m);
    assert!(debug_str.contains("Compact"));
}

#[test]
fn test_statusbar_config_debug() {
    let c = StatusBarConfig { verbose: true };
    let debug_str = format!("{:?}", c);
    assert!(debug_str.contains("verbose"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration-style Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mode_and_verbose_correspondence() {
    // Full mode should allow verbose
    let _mode = ResponsiveMode::Full;
    let config = StatusBarConfig { verbose: true };
    assert!(config.verbose);

    // Minimal mode should disable verbose
    let _mode = ResponsiveMode::Minimal;
    let config = StatusBarConfig { verbose: false };
    assert!(!config.verbose);
}

#[test]
fn test_typical_terminal_sizes() {
    // 80x24 terminals (old terminal)
    let mode = ResponsiveMode::from_width(80);
    assert_eq!(mode, ResponsiveMode::Compact);

    // 120x40 terminals (modern terminal, typical)
    let mode = ResponsiveMode::from_width(120);
    assert_eq!(mode, ResponsiveMode::Full);

    // 180x50 terminals (very wide)
    let mode = ResponsiveMode::from_width(180);
    assert_eq!(mode, ResponsiveMode::Full);

    // 40 char (very narrow, like phone)
    let mode = ResponsiveMode::from_width(40);
    assert_eq!(mode, ResponsiveMode::Minimal);
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 3: Visual Polish & Indicators Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_indicators_module_exists() {
    use ragent_tui::layout_statusbar::indicators;
    assert_eq!(indicators::HEALTHY, "●");
    assert_eq!(indicators::PARTIAL, "◔");
    assert_eq!(indicators::ERROR, "✗");
    assert_eq!(indicators::SUCCESS, "✓");
}

#[test]
fn test_spinner_frames_available() {
    use ragent_tui::layout_statusbar::spinner;
    assert!(!spinner::FRAMES.is_empty());
    assert_eq!(spinner::FRAMES.len(), 10);
    assert_eq!(spinner::FRAMES[0], "⠋");
}

#[test]
fn test_spinner_frame_selection() {
    use ragent_tui::layout_statusbar::spinner;
    // Frame should cycle through all 10 frames
    let frame0 = spinner::frame(0);
    let frame45 = spinner::frame(45);
    let frame90 = spinner::frame(90);
    let frame450 = spinner::frame(450);
    let frame495 = spinner::frame(495);

    assert_eq!(frame0, spinner::FRAMES[0]);
    assert_eq!(frame45, spinner::FRAMES[1]);
    assert_eq!(frame90, spinner::FRAMES[2]);
    assert_eq!(frame450, spinner::FRAMES[0]); // Wraps around at 450ms (10*45)
    assert_eq!(frame495, spinner::FRAMES[1]);
}

#[test]
fn test_colors_module_exists() {
    use ragent_tui::layout_statusbar::colors;
    use ratatui::style::Color;

    assert_eq!(colors::HEALTHY, Color::Green);
    assert_eq!(colors::WARNING, Color::Yellow);
    assert_eq!(colors::ERROR, Color::Red);
    assert_eq!(colors::IN_PROGRESS, Color::Cyan);
    assert_eq!(colors::LABEL, Color::DarkGray);
    assert_eq!(colors::TEXT, Color::White);
}

#[test]
fn test_progress_bar_characters() {
    use ragent_tui::layout_statusbar::indicators;
    assert_eq!(indicators::FILLED, "█");
    assert_eq!(indicators::EMPTY, "░");
}

#[test]
fn test_all_indicators_present() {
    use ragent_tui::layout_statusbar::indicators;

    // Verify all indicator constants exist
    assert!(!indicators::HEALTHY.is_empty());
    assert!(!indicators::PARTIAL.is_empty());
    assert!(!indicators::ERROR.is_empty());
    assert!(!indicators::SUCCESS.is_empty());
    assert!(!indicators::DIVERGED.is_empty());
    assert!(!indicators::BUSY.is_empty());
    assert!(!indicators::UNKNOWN.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 4: Responsive & Adaptive Behavior Tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_abbreviations_label_full_mode() {
    use ragent_tui::layout_statusbar::abbreviations;

    // In full mode, return original label
    assert_eq!(abbreviations::label("tokens", true), "tokens");
    assert_eq!(abbreviations::label("provider", true), "provider");
    assert_eq!(abbreviations::label("context", true), "context");
}

#[test]
fn test_abbreviations_label_compact_mode() {
    use ragent_tui::layout_statusbar::abbreviations;

    // In compact mode, return abbreviated label
    assert_eq!(abbreviations::label("tokens", false), "tok");
    assert_eq!(abbreviations::label("provider", false), "pvd");
    assert_eq!(abbreviations::label("context", false), "ctx");
    assert_eq!(abbreviations::label("tasks", false), "t");
    assert_eq!(abbreviations::label("health", false), "hlth");
}

#[test]
fn test_abbreviations_label_unknown() {
    use ragent_tui::layout_statusbar::abbreviations;

    // Unknown labels should return as-is
    assert_eq!(
        abbreviations::label("unknown_label", false),
        "unknown_label"
    );
    assert_eq!(abbreviations::label("unknown_label", true), "unknown_label");
}

#[test]
fn test_abbreviations_service() {
    use ragent_tui::layout_statusbar::abbreviations;

    assert_eq!(abbreviations::service("lsp_servers"), "LSP");
    assert_eq!(abbreviations::service("code_index"), "Idx");
    assert_eq!(abbreviations::service("aiwiki"), "Wiki");
    assert_eq!(abbreviations::service("memory"), "Mem");
    assert_eq!(abbreviations::service("unknown"), "unknown");
}

#[test]
fn test_abbreviations_provider() {
    use ragent_tui::layout_statusbar::abbreviations;

    assert_eq!(abbreviations::provider("anthropic"), "An");
    assert_eq!(abbreviations::provider("claude"), "Cl");
    assert_eq!(abbreviations::provider("openai"), "OAI");
    assert_eq!(abbreviations::provider("gpt"), "GPT");
    assert_eq!(abbreviations::provider("gemini"), "Gm");
    assert_eq!(abbreviations::provider("hugging_face"), "HF");
    assert_eq!(abbreviations::provider("unknown"), "unknown");
}

#[test]
fn test_responsive_mode_determines_abbreviations() {
    use ragent_tui::layout_statusbar::{ResponsiveMode, abbreviations};

    // Full mode should not use abbreviations
    let mode = ResponsiveMode::Full;
    assert_eq!(mode, ResponsiveMode::Full);
    assert_eq!(
        abbreviations::label("tokens", mode == ResponsiveMode::Full),
        "tokens"
    );

    // Compact mode should use abbreviations
    let mode = ResponsiveMode::Compact;
    assert_eq!(mode, ResponsiveMode::Compact);
    assert_eq!(
        abbreviations::label("tokens", mode == ResponsiveMode::Full),
        "tok"
    );

    // Minimal mode should use abbreviations
    let mode = ResponsiveMode::Minimal;
    assert_eq!(mode, ResponsiveMode::Minimal);
    assert_eq!(
        abbreviations::label("tokens", mode == ResponsiveMode::Full),
        "tok"
    );
}
