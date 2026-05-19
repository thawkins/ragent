//! Single-line text input field with full editing support.
//!
//! Provides cursor movement, keyboard selection, clipboard cut/copy/paste,
//! word navigation, and deletion — mirroring the behaviour of the main
//! message input but in a self-contained, reusable struct.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single-line text input field.
///
/// Tracks the raw text, a character-index cursor, and an optional keyboard-
/// selection anchor.  All editing operations are character-oriented so that
/// multi-byte UTF-8 content behaves correctly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputField {
    text: String,
    cursor: usize,
    anchor: Option<usize>,
}

impl InputField {
    /// Create an empty input field.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an input field pre-filled with text.  The cursor is placed at
    /// the end.
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor, anchor: None }
    }

    /// Current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Mutable access to the underlying buffer (use sparingly).
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// Current cursor position in characters.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set the cursor position, clamped to the text length.
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.text.chars().count());
    }

    /// Whether the field is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Character count of the text.
    pub fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    // ── Cursor helpers ─────────────────────────────────────────────────────

    fn cursor_byte_pos(&self) -> usize {
        if self.cursor == 0 {
            return 0;
        }
        self.text
            .char_indices()
            .nth(self.cursor)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    fn byte_pos_at_char(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    // ── Basic insertion / deletion ────────────────────────────────────────

    /// Insert a character at the cursor, replacing any active selection.
    pub fn insert_char(&mut self, c: char) {
        if let Some((s, e)) = self.selection_range() {
            self.remove_range(s, e);
            self.anchor = None;
        }
        let pos = self.cursor_byte_pos();
        self.text.insert(pos, c);
        self.cursor += 1;
    }

    /// Insert a string at the cursor, replacing any active selection.
    pub fn insert_str(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if let Some((start, end)) = self.selection_range() {
            self.remove_range(start, end);
            self.anchor = None;
        }
        let pos = self.cursor_byte_pos();
        let added = s.chars().count();
        self.text.insert_str(pos, s);
        self.cursor += added;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if let Some((s, e)) = self.selection_range() {
            self.remove_range(s, e);
            self.anchor = None;
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let pos = self.byte_pos_at_char(self.cursor - 1);
        self.text.remove(pos);
        self.cursor -= 1;
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        if let Some((s, e)) = self.selection_range() {
            self.remove_range(s, e);
            self.anchor = None;
            return;
        }
        if self.cursor >= self.len_chars() {
            return;
        }
        let pos = self.cursor_byte_pos();
        self.text.remove(pos);
    }

    /// Remove a range of characters (by char indices).
    fn remove_range(&mut self, start: usize, end: usize) {
        let start_byte = self.byte_pos_at_char(start);
        let end_byte = self.byte_pos_at_char(end);
        self.text.replace_range(start_byte..end_byte, "");
        self.cursor = start;
    }

    // ── Cursor movement ────────────────────────────────────────────────────

    /// Move the cursor one character to the left.
    pub fn move_left(&mut self) {
        self.anchor = None;
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move the cursor one character to the right.
    pub fn move_right(&mut self) {
        self.anchor = None;
        self.cursor = (self.cursor + 1).min(self.len_chars());
    }

    /// Move the cursor to the start of the text.
    pub fn move_home(&mut self) {
        self.anchor = None;
        self.cursor = 0;
    }

    /// Move the cursor to the end of the text.
    pub fn move_end(&mut self) {
        self.anchor = None;
        self.cursor = self.len_chars();
    }

    /// Move the cursor to the start of the previous word.
    pub fn move_word_left(&mut self) {
        self.anchor = None;
        if self.cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.text.chars().collect();
        let mut i = self.cursor.min(chars.len());
        while i > 0 && chars.get(i - 1).map_or(false, |c| c.is_whitespace()) {
            i -= 1;
        }
        while i > 0 && chars.get(i - 1).map_or(false, |c| !c.is_whitespace()) {
            i -= 1;
        }
        self.cursor = i;
    }

    /// Move the cursor to the start of the next word.
    pub fn move_word_right(&mut self) {
        self.anchor = None;
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();
        let mut i = self.cursor;
        while i < len && chars.get(i).map_or(false, |c| !c.is_whitespace()) {
            i += 1;
        }
        while i < len && chars.get(i).map_or(false, |c| c.is_whitespace()) {
            i += 1;
        }
        self.cursor = i;
    }

    // ── Selection ─────────────────────────────────────────────────────────

    /// Start or extend a selection one character to the left.
    pub fn select_left(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Start or extend a selection one character to the right.
    pub fn select_right(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = (self.cursor + 1).min(self.len_chars());
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.len_chars();
    }

    /// Clear any active selection without moving the cursor.
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Return the selected character range `(start, end)` if one is active.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        let cursor = self.cursor;
        if anchor == cursor {
            return None;
        }
        let start = anchor.min(cursor);
        let end = anchor.max(cursor);
        Some((start, end))
    }

    /// Return the selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        let (s, e) = self.selection_range()?;
        Some(self.text.chars().skip(s).take(e - s).collect())
    }

    // ── Clipboard operations ──────────────────────────────────────────────

    /// Copy the current selection to the system clipboard.
    pub fn copy_selection(&self) {
        if let Some(txt) = self.selected_text() {
            Self::set_clipboard(&txt);
        }
    }

    /// Cut the current selection to the system clipboard.
    pub fn cut_selection(&mut self) {
        if let Some((s, e)) = self.selection_range() {
            let txt: String = self.text.chars().skip(s).take(e - s).collect();
            Self::set_clipboard(&txt);
            self.remove_range(s, e);
            self.anchor = None;
        }
    }

    /// Paste text from the system clipboard at the cursor, replacing the
    /// selection if one is active.
    pub fn paste_clipboard(&mut self) {
        if let Some(text) = Self::get_clipboard() {
            let clean: String = text.chars().filter(|&c| c != '\r').collect();
            self.insert_str(&clean);
        }
    }

    // ── Clipboard helpers (same implementation as App) ────────────────────

    #[cfg(target_os = "linux")]
    fn get_clipboard() -> Option<String> {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    }

    #[cfg(not(target_os = "linux"))]
    fn get_clipboard() -> Option<String> {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
    }

    #[cfg(target_os = "linux")]
    fn set_clipboard(text: &str) {
        use arboard::SetExtLinux;
        let _ = arboard::Clipboard::new()
            .and_then(|mut cb| cb.set().wait().text(text.to_string()));
    }

    #[cfg(not(target_os = "linux"))]
    fn set_clipboard(text: &str) {
        let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text));
    }

    // ── Key event handler ───────────────────────────────────────────────

    /// Process a [`KeyEvent`] and return `true` if the event was consumed.
    ///
    /// Supports:
    /// - Typing characters (replacing selection)
    /// - Backspace / Delete
    /// - Left / Right / Home / End
    /// - Ctrl+Left/Right (word)
    /// - Shift+Left/Right (selection)
    /// - Ctrl+Shift+Left/Right (word selection)
    /// - Ctrl+A (select all)
    /// - Ctrl+C (copy)
    /// - Ctrl+X (cut)
    /// - Ctrl+V / Shift+Ctrl+V (paste)
    /// - Ctrl+W (delete previous word)
    /// - Ctrl+K (delete to end)
    /// - Ctrl+U (clear)
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        use KeyCode::{Char, Backspace, Delete, Left, Right, Home, End};

        match key.code {
            Char('a') if key.modifiers == KeyModifiers::CONTROL => {
                self.select_all();
                true
            }
            Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                self.copy_selection();
                true
            }
            Char('x') if key.modifiers == KeyModifiers::CONTROL => {
                self.cut_selection();
                true
            }
            Char('v')
                if key.modifiers == KeyModifiers::CONTROL
                    || key.modifiers
                        == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
            {
                self.paste_clipboard();
                true
            }
            Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                self.delete_prev_word();
                true
            }
            Char('k') if key.modifiers == KeyModifiers::CONTROL => {
                self.delete_to_end();
                true
            }
            Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                self.text.clear();
                self.cursor = 0;
                self.anchor = None;
                true
            }
            Char(c)
                if key.modifiers.is_empty()
                    || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.insert_char(c);
                true
            }
            Backspace => {
                self.backspace();
                true
            }
            Delete => {
                self.delete();
                true
            }
            Left if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                if self.anchor.is_none() {
                    self.anchor = Some(self.cursor);
                }
                self.move_word_left();
                true
            }
            Right if key.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                if self.anchor.is_none() {
                    self.anchor = Some(self.cursor);
                }
                self.move_word_right();
                true
            }
            Left if key.modifiers == KeyModifiers::SHIFT => {
                self.select_left();
                true
            }
            Right if key.modifiers == KeyModifiers::SHIFT => {
                self.select_right();
                true
            }
            Left if key.modifiers == KeyModifiers::CONTROL => {
                self.move_word_left();
                true
            }
            Right if key.modifiers == KeyModifiers::CONTROL => {
                self.move_word_right();
                true
            }
            Left => {
                self.move_left();
                true
            }
            Right => {
                self.move_right();
                true
            }
            Home if key.modifiers == KeyModifiers::CONTROL => {
                self.move_home();
                true
            }
            End if key.modifiers == KeyModifiers::CONTROL => {
                self.move_end();
                true
            }
            Home => {
                self.move_home();
                true
            }
            End => {
                self.move_end();
                true
            }
            _ => false,
        }
    }

    // ── Extra helpers ─────────────────────────────────────────────────────

    /// Delete the word immediately before the cursor.
    pub fn delete_prev_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = self.cursor;
        self.move_word_left();
        let start = self.cursor;
        self.remove_range(start, end);
        self.anchor = None;
    }

    /// Delete from cursor to end of text.
    pub fn delete_to_end(&mut self) {
        self.remove_range(self.cursor, self.len_chars());
        self.anchor = None;
    }
}
