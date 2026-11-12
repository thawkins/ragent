//! Context menu for one session row of the sidebar (spec SESSIONMENU).
//!
//! Opened with `m` or `Shift+F10` on a session row while the sidebar has
//! focus, or with a right-click on a session row. It lists the actions that
//! apply to that session, each with a one-letter accelerator, in groups
//! separated by a rule. `Open in` and `Move to section` open a submenu
//! (`Right`, `Enter` or a click); `Left`/`Esc` close one level. While it is
//! open it consumes every key; a click outside it closes it.
//!
//! The menu only dispatches: each action calls the same `App` method as the
//! sidebar's single-key shortcut or `/session` subcommand, so the paths
//! cannot drift apart.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;
use crate::open_in::{LaunchEnv, OpenTarget};

/// What a menu item does to its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Open the `Open in` submenu.
    OpenIn,
    /// Pin or unpin the session (same as `p`).
    TogglePin,
    /// Mark the session unread, or read again (same as `u`).
    ToggleUnread,
    /// Focus the session and prefill `/session rename` (same as `F2`).
    Rename,
    /// Copy the transcript into a new session (same as `f`).
    Fork,
    /// Open the `Move to section` submenu.
    MoveTo,
    /// Archive the session (same as `a`).
    Archive,
    /// Delete the session after a confirmation (same as `D`).
    Delete,
    /// Open the session's directory in a program (`Open in` submenu).
    Launch(OpenTarget),
    /// Move the session to a section id, or back to its project with `None`
    /// (`Move to section` submenu).
    MoveToSection(Option<String>),
    /// Focus the session and prefill `/session section new` (`Move to
    /// section` submenu).
    NewSection,
}

impl MenuAction {
    /// Whether the item opens a submenu instead of running.
    fn opens_submenu(&self) -> bool {
        matches!(self, Self::OpenIn | Self::MoveTo)
    }
}

/// One selectable menu entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Text shown for the item.
    pub label: String,
    /// Accelerator key, shown upper-case and matched case-insensitively.
    pub accel: Option<char>,
    /// The action the item runs.
    pub action: MenuAction,
    /// Group number; a rule is drawn between items of different groups.
    pub group: u8,
    /// Whether the item destroys data; drawn in red.
    pub danger: bool,
}

/// An open context menu, or an open submenu of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMenu {
    /// The session the menu acts on.
    pub session_id: String,
    /// Screen row the menu is anchored to: the sidebar row for the menu,
    /// the parent item's line for a submenu.
    pub row_y: u16,
    /// Selectable items, in display order.
    pub items: Vec<MenuItem>,
    /// Index into `items` of the highlighted item.
    pub cursor: usize,
    /// Area painted last frame; zero-sized before the first paint.
    pub area: Rect,
    /// The open submenu, if any.
    pub submenu: Option<Box<Self>>,
}

impl SessionMenu {
    /// Index of the painted line (inside the border) showing item `index`,
    /// counting the rule lines between groups.
    fn line_of(&self, index: usize) -> usize {
        let rules = self
            .items
            .windows(2)
            .take(index)
            .filter(|pair| pair[0].group != pair[1].group)
            .count();
        index + rules
    }

    /// The item painted on line `line` inside the border, if any.
    fn item_at_line(&self, line: usize) -> Option<usize> {
        (0..self.items.len()).find(|&index| self.line_of(index) == line)
    }

    /// The item painted at screen row `row`, if any.
    fn item_at_row(&self, row: u16) -> Option<usize> {
        let line = row.checked_sub(self.area.y + 1).map(usize::from)?;
        self.item_at_line(line)
    }

    /// Number of painted lines inside the border.
    fn line_count(&self) -> usize {
        self.items
            .len()
            .checked_sub(1)
            .map_or(0, |last| self.line_of(last) + 1)
    }

    /// Whether `(column, row)` is inside the painted area.
    fn contains(&self, column: u16, row: u16) -> bool {
        self.area.width > 0 && self.area.contains((column, row).into())
    }
}

/// Builds an item; `Delete` is the only dangerous one.
fn item(label: &str, accel: Option<char>, action: MenuAction, group: u8) -> MenuItem {
    MenuItem {
        label: label.to_string(),
        accel,
        danger: action == MenuAction::Delete,
        action,
        group,
    }
}

/// The `Open in` targets this machine can start for `session_id`.
fn open_in_items(app: &App, session_id: &str) -> Vec<MenuItem> {
    let Some(dir) = app.session_directory(session_id) else {
        return Vec::new();
    };
    let env = LaunchEnv::current();
    [
        OpenTarget::Editor,
        OpenTarget::FileManager,
        OpenTarget::Terminal,
    ]
    .into_iter()
    .filter(|t| crate::open_in::launch_spec(*t, &dir, &env).is_some())
    .map(|t| item(t.label(), None, MenuAction::Launch(t), 0))
    .collect()
}

/// The `Move to section` choices; the session's current place is marked.
fn move_to_items(app: &App, session_id: &str) -> Vec<MenuItem> {
    let current = app.sidebar.session_section.get(session_id);
    let mark = |here: bool, name: &str| format!("{} {name}", if here { "*" } else { " " });
    let mut items = vec![item(
        &mark(current.is_none(), "Project (default)"),
        None,
        MenuAction::MoveToSection(None),
        0,
    )];
    items.extend(app.sidebar.sections.iter().map(|s| {
        item(
            &mark(current == Some(&s.id), &s.name),
            None,
            MenuAction::MoveToSection(Some(s.id.clone())),
            1,
        )
    }));
    items.push(item("  New section...", None, MenuAction::NewSection, 2));
    items
}

/// Builds the top-level items that apply to `session_id`.
fn build_items(app: &App, session_id: &str) -> Vec<MenuItem> {
    let pin = if app.sidebar.pinned.contains(session_id) {
        "Unpin"
    } else {
        "Pin"
    };
    let unread = if crate::sessions_sidebar::attention_of(app, session_id)
        == crate::app::Attention::Unread
    {
        "Mark as read"
    } else {
        "Mark as unread"
    };
    let mut items = Vec::new();
    if !open_in_items(app, session_id).is_empty() {
        items.push(item("Open in", None, MenuAction::OpenIn, 0));
    }
    items.extend([
        item(pin, Some('p'), MenuAction::TogglePin, 1),
        item(unread, Some('u'), MenuAction::ToggleUnread, 1),
        item("Rename", Some('r'), MenuAction::Rename, 1),
        item("Fork", Some('f'), MenuAction::Fork, 1),
        item("Move to section", None, MenuAction::MoveTo, 2),
        item("Archive", Some('a'), MenuAction::Archive, 3),
        item("Delete", Some('d'), MenuAction::Delete, 3),
    ]);
    items
}

/// Opens the menu for `session_id`, anchored to the sidebar row painted at
/// screen row `row_y`.
///
/// # Arguments
///
/// * `app` - the application; the menu is stored in `app.sidebar.menu`.
/// * `session_id` - the session the menu acts on.
/// * `row_y` - screen row of that session's sidebar row.
pub fn open(app: &mut App, session_id: &str, row_y: u16) {
    let items = build_items(app, session_id);
    app.sidebar.menu = Some(SessionMenu {
        session_id: session_id.to_string(),
        row_y,
        items,
        cursor: 0,
        area: Rect::default(),
        submenu: None,
    });
}

/// Width and height of `menu`, clamped to `frame`.
fn menu_size(menu: &SessionMenu, frame: Rect) -> (u16, u16) {
    let label_width = menu
        .items
        .iter()
        .map(|item| item.label.chars().count())
        .max()
        .unwrap_or(0);
    // Border, space, label, gap, accelerator, space, border.
    let width = u16::try_from(label_width + 7)
        .unwrap_or(u16::MAX)
        .min(frame.width);
    let height = u16::try_from(menu.line_count() + 2)
        .unwrap_or(u16::MAX)
        .min(frame.height);
    (width, height)
}

/// Where the menu goes: just below its row, or just above it when it would
/// run off the bottom of `frame`; shifted left to stay on screen.
fn menu_area(menu: &SessionMenu, sidebar: Rect, frame: Rect) -> Rect {
    let (width, height) = menu_size(menu, frame);
    let right = frame.x + frame.width;
    let x = (sidebar.x + sidebar.width.saturating_sub(4)).min(right.saturating_sub(width));
    let bottom = frame.y + frame.height;
    let y = if menu.row_y + 1 + height <= bottom {
        menu.row_y + 1
    } else {
        menu.row_y.saturating_sub(height).max(frame.y)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Where a submenu goes: to the right of `parent`, its first item level
/// with the parent item, or to the left of `parent` when there is no room
/// on the right; shifted up to stay on screen.
fn submenu_area(sub: &SessionMenu, parent: Rect, frame: Rect) -> Rect {
    let (width, height) = menu_size(sub, frame);
    let right = frame.x + frame.width;
    let x = if parent.x + parent.width + width <= right {
        parent.x + parent.width
    } else {
        parent.x.saturating_sub(width)
    };
    let bottom = frame.y + frame.height;
    let y = sub
        .row_y
        .saturating_sub(1)
        .min(bottom.saturating_sub(height))
        .max(frame.y);
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Paints one menu level into `area`.
fn paint(frame: &mut Frame, menu: &SessionMenu, area: Rect) {
    let inner_width = usize::from(area.width.saturating_sub(2));
    let normal = Style::default().fg(Color::White);
    let selected = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let mut lines = Vec::with_capacity(menu.line_count());
    for (index, item) in menu.items.iter().enumerate() {
        if index > 0 && menu.items[index - 1].group != item.group {
            lines.push(Line::from(Span::styled(
                "-".repeat(inner_width),
                Style::default().fg(Color::DarkGray),
            )));
        }
        let hint = if item.action.opens_submenu() {
            '>'
        } else {
            item.accel.map_or(' ', |c| c.to_ascii_uppercase())
        };
        let gap = inner_width.saturating_sub(item.label.chars().count() + 3);
        let text = format!(" {}{}{hint} ", item.label, " ".repeat(gap));
        let style = match (index == menu.cursor, item.danger) {
            (true, _) => selected,
            (false, true) => normal.fg(Color::Red),
            (false, false) => normal,
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Paints the open menu and submenu, if any, on top of the sidebar and chat.
///
/// # Arguments
///
/// * `frame` - the frame being drawn.
/// * `app` - holds the menu; painted areas are recorded for mouse hits.
pub fn render(frame: &mut Frame, app: &mut App) {
    let sidebar = app.sidebar.area;
    let full = frame.area();
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return;
    };
    let area = menu_area(menu, sidebar, full);
    menu.area = area;
    paint(frame, menu, area);
    if let Some(sub) = menu.submenu.as_mut() {
        let sub_area = submenu_area(sub, area, full);
        sub.area = sub_area;
        paint(frame, sub, sub_area);
    }
}

/// Opens the submenu of the item at `index` of the open menu.
fn open_submenu(app: &mut App, index: usize) {
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return;
    };
    let session_id = menu.session_id.clone();
    let items = match menu.items.get(index).map(|i| &i.action) {
        Some(MenuAction::OpenIn) => open_in_items(app, &session_id),
        Some(MenuAction::MoveTo) => move_to_items(app, &session_id),
        _ => return,
    };
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return;
    };
    let row_y = menu.area.y + 1 + u16::try_from(menu.line_of(index)).unwrap_or(0);
    menu.cursor = index;
    menu.submenu = Some(Box::new(SessionMenu {
        session_id,
        row_y,
        items,
        cursor: 0,
        area: Rect::default(),
        submenu: None,
    }));
}

/// Closes the menu and runs `action` on `session_id`.
fn dispatch(app: &mut App, session_id: &str, action: MenuAction) {
    app.sidebar.menu = None;
    let result = match action {
        MenuAction::OpenIn | MenuAction::MoveTo => Ok(()),
        MenuAction::TogglePin => app.toggle_session_pin(session_id),
        MenuAction::ToggleUnread => app.toggle_session_unread(session_id),
        MenuAction::Rename => {
            crate::sessions_sidebar::begin_rename(app, session_id);
            Ok(())
        }
        MenuAction::Fork => app.fork_session_by_id(session_id),
        MenuAction::Archive => app.archive_session_by_id(session_id),
        MenuAction::Delete => app.request_delete_session(session_id),
        MenuAction::Launch(target) => app.open_session_in(session_id, target),
        MenuAction::MoveToSection(section) => {
            app.move_session_to_section(session_id, section.as_deref())
        }
        MenuAction::NewSection => {
            crate::sessions_sidebar::begin_prefill(app, session_id, "/session section new ");
            Ok(())
        }
    };
    crate::sessions_sidebar::report(app, result);
}

/// Runs the item at `index` of the open menu (`in_submenu` false) or of its
/// submenu (`true`); an item with a submenu opens it instead.
fn activate(app: &mut App, index: usize, in_submenu: bool) {
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return;
    };
    let level = if in_submenu {
        match menu.submenu.as_deref() {
            Some(sub) => sub,
            None => return,
        }
    } else {
        menu
    };
    let Some(item) = level.items.get(index) else {
        return;
    };
    if item.action.opens_submenu() {
        open_submenu(app, index);
        return;
    }
    let (session_id, action) = (menu.session_id.clone(), item.action.clone());
    dispatch(app, &session_id, action);
}

/// Handles a key while the menu is open.
///
/// # Returns
///
/// `true` when the menu is open (it consumes every key), `false` otherwise.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return false;
    };
    let in_submenu = menu.submenu.is_some();
    let level: &mut SessionMenu = match menu.submenu.as_deref_mut() {
        Some(sub) => sub,
        None => menu,
    };
    let last = level.items.len().saturating_sub(1);
    match key.code {
        KeyCode::Up => level.cursor = level.cursor.saturating_sub(1),
        KeyCode::Down => level.cursor = (level.cursor + 1).min(last),
        KeyCode::Home => level.cursor = 0,
        KeyCode::End => level.cursor = last,
        KeyCode::Enter => {
            let index = level.cursor;
            activate(app, index, in_submenu);
        }
        KeyCode::Right if !in_submenu => {
            let index = level.cursor;
            open_submenu(app, index);
        }
        KeyCode::Esc | KeyCode::Left => {
            if let Some(menu) = app.sidebar.menu.as_mut()
                && in_submenu
            {
                menu.submenu = None;
            } else {
                app.sidebar.menu = None;
            }
        }
        KeyCode::Char(c) => {
            let c = c.to_ascii_lowercase();
            if let Some(index) = level.items.iter().position(|item| item.accel == Some(c)) {
                activate(app, index, in_submenu);
            }
        }
        _ => {}
    }
    true
}

/// Handles a mouse event while the menu is open.
///
/// # Returns
///
/// `true` when the menu is open (it consumes every event), `false` otherwise.
/// A left click on an item runs it (or opens its submenu); any other button
/// press, or a click outside both levels, closes the menu.
pub fn handle_mouse(app: &mut App, event: MouseEvent) -> bool {
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return false;
    };
    let (column, row) = (event.column, event.row);
    let in_sub = menu
        .submenu
        .as_deref()
        .filter(|sub| sub.contains(column, row));
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) if in_sub.is_some() => {
            // A click on the border or a rule keeps the menu open.
            if let Some(index) = in_sub.and_then(|sub| sub.item_at_row(row)) {
                activate(app, index, true);
            }
        }
        MouseEventKind::Down(MouseButton::Left) if menu.contains(column, row) => {
            if let Some(index) = menu.item_at_row(row) {
                if let Some(menu) = app.sidebar.menu.as_mut() {
                    menu.submenu = None;
                }
                activate(app, index, false);
            }
        }
        MouseEventKind::Down(_) => app.sidebar.menu = None,
        _ => {}
    }
    true
}
