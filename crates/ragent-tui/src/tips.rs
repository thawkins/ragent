//! Rotating tips displayed on the home screen.

use rand::Rng;

const TIPS: &[&str] = &[
    "Press Tab to cycle between Build and General agents",
    "Type your message and press Enter to start a session",
    "Press Ctrl+C at any time to quit",
    "Use Up/Down arrows to scroll through message history",
    "Tool calls show live status: running, completed, or error",
    "Permission requests appear as dialogs — press y/a/n to respond",
    "The status bar shows token usage for the current session",
    "Start a message with / for slash commands",
    "Agent reasoning is shown with a 💭 prefix",
    "Each session tracks input and output token counts",
    "The general agent is a full-access coding assistant",
    "The build agent specialises in building and testing",
    "Press Enter on the home screen to jump straight into a session",
];

/// Return a randomly selected tip string.
pub fn random_tip() -> &'static str {
    let idx = rand::thread_rng().gen_range(0..TIPS.len());
    TIPS[idx]
}
