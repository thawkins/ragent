//! ASCII art logo for the ragent home screen.

/// Logo lines rendered using Unicode block characters.
///
/// The logo spells "ragent" in a chunky block style using full-block (█),
/// half-block (▀, ▄), and shadow characters.
pub const LOGO: &[&str] = &[
    "                                              ",
    " █▀▀▄  █▀▀█  █▀▀▀  █▀▀▀  █▀▀▄  ▀▀█▀▀",
    " █▄▄▀  █▄▄█  █ ▀█  █▀▀   █  █    █  ",
    " ▀  ▀  ▀  ▀  ▀▀▀▀  ▀▀▀▀  ▀  ▀    ▀  ",
];
