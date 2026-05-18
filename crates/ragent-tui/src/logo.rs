//! ASCII art logo for the ragent home screen.

/// Logo lines rendered using Unicode block characters.
///
/// The logo spells "ragent" in a chunky block style using full-block (█),
/// half-block (▀, ▄), and shadow characters.
///
/// This is the original size version.
pub const LOGO: &[&str] = &[
    "",
    " █▀▀▄  █▀▀█  █▀▀▀  █▀▀▀  █▀▀▄  ▀▀█▀▀",
    " █▄▄▀  █▄▄█  █ ▀█  █▀▀   █  █    █  ",
    " ▀  ▀  ▀  ▀  ▀▀▀▀  ▀▀▀▀  ▀  ▀    ▀  ",
];

/// Double-size logo — each cell is repeated 2×2 so the banner is twice
/// as tall and twice as wide. Uses only full-block characters for
/// maximum terminal compatibility.
pub const LOGO_2X: &[&str] = &[
    "",
    "  ████████    ████████    ████████    ████████    ██    ██    ████████",
    "  ████████    ████████    ████████    ████████    ██    ██    ████████",
    "  ██  ████    ██  ████    ██          ██          ████  ██      ████",
    "  ██  ████    ██  ████    ██          ██          ████  ██      ████",
    "  ████████    ████████    ██  ████    ██████      ██  ████      ████",
    "  ████████    ████████    ██  ████    ██████      ██  ████      ████",
    "  ██  ████    ██  ████    ██    ██    ██          ██    ██      ████",
    "  ██  ████    ██  ████    ██    ██    ██          ██    ██      ████",
    "  ██  ████    ██    ██    ██████      ████████    ██    ██      ████",
    "  ██  ████    ██    ██    ██████      ████████    ██    ██      ████",
];
