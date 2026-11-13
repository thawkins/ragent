//! Context menus of the sidebar (spec SESSIONMENU): one for a session row,
//! one for a project heading, and one for the multi-selection.
//!
//! Opened with `m` or `Shift+F10` while the sidebar has focus, or with a
//! right-click: on a session row it acts on that session, on a selected
//! session (with more than one selected) on the whole selection, and on a
//! project heading on that project. It lists the actions that apply, each
//! with a one-letter accelerator, in groups separated by a rule; an action
//! that cannot run now is greyed out and does nothing. `Open in` and `Move
//! to section` open a submenu (`Right`, `Enter` or a click); `Left`/`Esc`
//! close one level. While it is open it consumes every key; a click outside
//! it closes it.
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

use std::path::PathBuf;

use crate::app::{App, Attention};
use crate::open_in::{LaunchEnv, OpenTarget};

/// What an open menu acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    /// One session, by id.
    Session(String),
    /// A project heading, by project id.
    Project(String),
    /// The sessions in the sidebar's multi-selection.
    Selection,
}

/// What a menu item does to its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    /// Open the bound pull/merge request in the browser (`Open PR` /
    /// `Open MR`, shown only when the session is bound).
    OpenRequest,
    /// Turn Auto-fix on or off for the bound request (`Auto-fix: on/off`,
    /// shown only when the session is bound).
    ToggleAutofix,
    /// Open the `PR/MRs` submenu: every pull/merge request the session has
    /// had (shown when it has had one).
    Requests,
    /// Open a web URL in the default browser (`PR/MRs` submenu).
    OpenUrl(String),
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
    /// Start a session in the project (same as `n` on its heading).
    NewSession,
    /// Prefill the command that starts a session on its own branch (same
    /// as `b` on a project heading).
    NewSessionOnBranch,
    /// Prefill `/project rename` for the project.
    RenameProject,
    /// Move the project one place up in the sidebar.
    MoveUp,
    /// Move the project one place down in the sidebar.
    MoveDown,
    /// Select every session listed under the project.
    SelectAll,
    /// Empty the selection.
    ClearSelection,
    /// Collapse every sidebar section.
    CollapseAll,
    /// Expand every sidebar section.
    ExpandAll,
}

impl MenuAction {
    /// Whether the item opens a submenu instead of running.
    fn opens_submenu(&self) -> bool {
        matches!(self, Self::Requests | Self::OpenIn | Self::MoveTo)
    }

    /// Whether the item destroys data; drawn in red.
    fn is_danger(&self) -> bool {
        matches!(self, Self::Delete)
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
    /// Whether the item can run now; a disabled one is greyed out and
    /// does nothing.
    pub enabled: bool,
}

/// An open context menu, or an open submenu of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMenu {
    /// What the menu acts on.
    pub target: MenuTarget,
    /// Screen row the menu is anchored to: the sidebar row for the menu,
    /// the parent item's line for a submenu.
    pub row_y: u16,
    /// Selectable items, in display order.
    pub items: Vec<MenuItem>,
    /// Index into `items` of the highlighted item.
    pub cursor: usize,
    /// Area painted last frame; zero-sized before the first paint.
    pub area: Rect,
    /// First painted line when the items do not all fit on screen; kept so
    /// the highlighted item is always visible.
    pub scroll: usize,
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
        if line >= usize::from(self.area.height.saturating_sub(2)) {
            return None;
        }
        self.item_at_line(line + self.scroll)
    }

    /// Number of painted lines inside the border.
    fn line_count(&self) -> usize {
        self.items
            .len()
            .checked_sub(1)
            .map_or(0, |last| self.line_of(last) + 1)
    }

    /// Moves `scroll` so the highlighted item's line is among the
    /// `visible` painted lines.
    fn keep_cursor_visible(&mut self, visible: usize) {
        let line = self.line_of(self.cursor);
        if visible == 0 {
            self.scroll = 0;
        } else if line < self.scroll {
            self.scroll = line;
        } else if line >= self.scroll + visible {
            self.scroll = line + 1 - visible;
        }
        self.scroll = self.scroll.min(self.line_count().saturating_sub(visible));
    }

    /// Whether `(column, row)` is inside the painted area.
    fn contains(&self, column: u16, row: u16) -> bool {
        self.area.width > 0 && self.area.contains((column, row).into())
    }
}

/// Builds an enabled item; `Delete` is the only dangerous one.
fn item(label: &str, accel: Option<char>, action: MenuAction, group: u8) -> MenuItem {
    MenuItem {
        label: label.to_string(),
        accel,
        danger: action.is_danger(),
        action,
        group,
        enabled: true,
    }
}

/// Builds an item that is greyed out unless `enabled`.
fn item_if(
    enabled: bool,
    label: &str,
    accel: Option<char>,
    action: MenuAction,
    group: u8,
) -> MenuItem {
    MenuItem {
        enabled,
        ..item(label, accel, action, group)
    }
}

/// The directory `Open in` opens for `target`: the session's, or the
/// project's worktree.
fn target_directory(app: &App, target: &MenuTarget) -> Option<PathBuf> {
    match target {
        MenuTarget::Session(id) => app.session_directory(id),
        MenuTarget::Project(id) => app
            .sidebar
            .groups
            .iter()
            .find(|g| g.project_id == *id)
            .map(|g| PathBuf::from(&g.project.worktree)),
        MenuTarget::Selection => None,
    }
}

/// The `Open in` targets this machine can start for `target`.
fn open_in_items(app: &App, target: &MenuTarget) -> Vec<MenuItem> {
    let Some(dir) = target_directory(app, target) else {
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

/// The `PR/MRs` choices: each pull/merge request session `target` has had,
/// newest first, as `PR #12 [open] title`; choosing one opens its URL.
fn request_items(app: &App, target: &MenuTarget) -> Vec<MenuItem> {
    let MenuTarget::Session(id) = target else {
        return Vec::new();
    };
    let history = match app.storage.session_request_history(id) {
        Ok(history) => history,
        Err(e) => {
            tracing::warn!(error = %e, "could not read PR/MR history");
            Vec::new()
        }
    };
    history
        .iter()
        .map(|r| {
            let (short, sigil) =
                ragent_agent::project::RemoteKind::parse(&r.forge).map_or(("#", '#'), |kind| {
                    let noun = kind.noun();
                    (noun.short, noun.sigil)
                });
            let title: String = r.title.chars().take(REQUEST_TITLE_WIDTH).collect();
            let label = format!("{short} {sigil}{} [{}] {title}", r.number, r.state);
            item(&label, None, MenuAction::OpenUrl(r.web_url.clone()), 0)
        })
        .collect()
}

/// Longest request title shown in the `PR/MRs` submenu.
const REQUEST_TITLE_WIDTH: usize = 48;

/// The `Move to section` choices. For one session its current place is
/// marked and `New section...` is offered; for the selection nothing is
/// marked and new sections are made with `/session section new` first.
fn move_to_items(app: &App, target: &MenuTarget) -> Vec<MenuItem> {
    let current = match target {
        MenuTarget::Session(id) => Some(app.sidebar.session_section.get(id)),
        MenuTarget::Project(_) | MenuTarget::Selection => None,
    };
    let mark = |here: bool, name: &str| format!("{} {name}", if here { "*" } else { " " });
    let mut items = vec![item(
        &mark(current == Some(None), "Project (default)"),
        None,
        MenuAction::MoveToSection(None),
        0,
    )];
    items.extend(app.sidebar.sections.iter().map(|s| {
        item(
            &mark(current == Some(Some(&s.id)), &s.name),
            None,
            MenuAction::MoveToSection(Some(s.id.clone())),
            1,
        )
    }));
    if current.is_some() {
        items.push(item("  New section...", None, MenuAction::NewSection, 2));
    }
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
    if let Some(request) = app.session_request_of(session_id) {
        let short = ragent_agent::project::RemoteKind::parse(&request.forge)
            .map_or("request", |kind| kind.noun().short);
        items.push(item(
            &format!("Open {short}"),
            Some('g'),
            MenuAction::OpenRequest,
            0,
        ));
        let on = match app.storage.session_autofix(&request.session_id) {
            Ok(state) => state.is_some_and(|state| state.enabled),
            Err(e) => {
                tracing::warn!(error = %e, "could not read Auto-fix state");
                false
            }
        };
        items.push(item(
            if on { "Auto-fix: on" } else { "Auto-fix: off" },
            Some('x'),
            MenuAction::ToggleAutofix,
            0,
        ));
    }
    let total = app
        .sidebar
        .request_totals
        .get(session_id)
        .copied()
        .unwrap_or(0);
    if total > 0 {
        items.push(item(
            &format!("PR/MRs ({total})"),
            None,
            MenuAction::Requests,
            0,
        ));
    }
    if !open_in_items(app, &MenuTarget::Session(session_id.to_string())).is_empty() {
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
    show(
        app,
        MenuTarget::Session(session_id.to_string()),
        row_y,
        items,
    );
}

/// Builds the items of project `project_id`'s menu.
fn project_items(app: &App, project_id: &str) -> Vec<MenuItem> {
    let target = MenuTarget::Project(project_id.to_string());
    let listed = crate::sessions_sidebar::project_session_ids(app, project_id).len();
    let (up, down) = crate::sessions_sidebar::project_moves(app, project_id);
    let keys = crate::sessions_sidebar::section_keys(app);
    let collapsed = &app.sidebar.collapsed;
    let any_open = keys.iter().any(|k| !collapsed.contains(k));
    let any_collapsed = keys.iter().any(|k| collapsed.contains(k));
    let mut items = vec![
        item("New session", Some('n'), MenuAction::NewSession, 0),
        item(
            "New session on branch",
            Some('b'),
            MenuAction::NewSessionOnBranch,
            0,
        ),
    ];
    if !open_in_items(app, &target).is_empty() {
        items.push(item("Open in", None, MenuAction::OpenIn, 0));
    }
    items.extend([
        item("Rename project", Some('r'), MenuAction::RenameProject, 1),
        item_if(up, "Move up", Some('k'), MenuAction::MoveUp, 1),
        item_if(down, "Move down", Some('j'), MenuAction::MoveDown, 1),
        item_if(
            listed > 0,
            "Select all",
            Some('s'),
            MenuAction::SelectAll,
            2,
        ),
        item_if(
            any_open,
            "Collapse all",
            Some('c'),
            MenuAction::CollapseAll,
            2,
        ),
        item_if(
            any_collapsed,
            "Expand all",
            Some('e'),
            MenuAction::ExpandAll,
            2,
        ),
        item_if(
            listed > 0,
            &format!("Archive all ({listed})"),
            Some('a'),
            MenuAction::Archive,
            3,
        ),
        item_if(
            listed > 0,
            &format!("Delete all sessions ({listed})"),
            Some('d'),
            MenuAction::Delete,
            3,
        ),
    ]);
    items
}

/// Opens the menu for project `project_id`, anchored to its heading painted
/// at screen row `row_y`.
///
/// # Arguments
///
/// * `app` - the application; the menu is stored in `app.sidebar.menu`.
/// * `project_id` - the project the menu acts on.
/// * `row_y` - screen row of the project's heading.
pub fn open_project(app: &mut App, project_id: &str, row_y: u16) {
    let items = project_items(app, project_id);
    show(
        app,
        MenuTarget::Project(project_id.to_string()),
        row_y,
        items,
    );
}

/// Builds the items of the selection's menu. `Pin` and `Mark as unread`
/// apply to every selected session unless all already have that state, in
/// which case they read `Unpin` and `Mark as read`.
fn selection_items(app: &App) -> Vec<MenuItem> {
    let selected = &app.sidebar.selected;
    let n = selected.len();
    let all_pinned = selected.iter().all(|id| app.sidebar.pinned.contains(id));
    let all_unread = selected
        .iter()
        .all(|id| crate::sessions_sidebar::attention_of(app, id) == Attention::Unread);
    vec![
        item(
            &format!("{} ({n})", if all_pinned { "Unpin" } else { "Pin" }),
            Some('p'),
            MenuAction::TogglePin,
            0,
        ),
        item(
            if all_unread {
                "Mark as read"
            } else {
                "Mark as unread"
            },
            Some('u'),
            MenuAction::ToggleUnread,
            0,
        ),
        item("Move to section", None, MenuAction::MoveTo, 0),
        item("Clear selection", Some('c'), MenuAction::ClearSelection, 1),
        item(&format!("Archive ({n})"), Some('a'), MenuAction::Archive, 2),
        item(&format!("Delete ({n})"), Some('d'), MenuAction::Delete, 2),
    ]
}

/// Opens the menu for the sessions in the selection, anchored to the
/// sidebar row painted at screen row `row_y`.
///
/// # Arguments
///
/// * `app` - the application; the menu is stored in `app.sidebar.menu`.
/// * `row_y` - screen row of the selected session the menu was opened on.
pub fn open_selection(app: &mut App, row_y: u16) {
    let items = selection_items(app);
    show(app, MenuTarget::Selection, row_y, items);
}

/// Stores a new top-level menu.
fn show(app: &mut App, target: MenuTarget, row_y: u16, items: Vec<MenuItem>) {
    app.sidebar.menu = Some(SessionMenu {
        target,
        row_y,
        items,
        cursor: 0,
        area: Rect::default(),
        scroll: 0,
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
        let style = match (index == menu.cursor, item.enabled, item.danger) {
            (true, _, _) => selected,
            (false, false, _) => normal.fg(Color::DarkGray),
            (false, true, true) => normal.fg(Color::Red),
            (false, true, false) => normal,
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    let lines: Vec<Line> = lines.into_iter().skip(menu.scroll).collect();
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
    menu.keep_cursor_visible(usize::from(area.height.saturating_sub(2)));
    paint(frame, menu, area);
    if let Some(sub) = menu.submenu.as_mut() {
        let sub_area = submenu_area(sub, area, full);
        sub.area = sub_area;
        sub.keep_cursor_visible(usize::from(sub_area.height.saturating_sub(2)));
        paint(frame, sub, sub_area);
    }
}

/// Opens the submenu of the item at `index` of the open menu.
fn open_submenu(app: &mut App, index: usize) {
    let Some(menu) = app.sidebar.menu.as_ref() else {
        return;
    };
    let target = menu.target.clone();
    let items = match menu.items.get(index).map(|i| &i.action) {
        Some(MenuAction::Requests) => request_items(app, &target),
        Some(MenuAction::OpenIn) => open_in_items(app, &target),
        Some(MenuAction::MoveTo) => move_to_items(app, &target),
        _ => return,
    };
    let Some(menu) = app.sidebar.menu.as_mut() else {
        return;
    };
    let line = menu.line_of(index).saturating_sub(menu.scroll);
    let row_y = menu.area.y + 1 + u16::try_from(line).unwrap_or(0);
    menu.cursor = index;
    menu.submenu = Some(Box::new(SessionMenu {
        target,
        row_y,
        items,
        cursor: 0,
        area: Rect::default(),
        scroll: 0,
        submenu: None,
    }));
}

/// Closes the menu and runs `action` on `target`.
fn dispatch(app: &mut App, target: &MenuTarget, action: MenuAction) {
    app.sidebar.menu = None;
    let result = match target {
        MenuTarget::Session(id) => dispatch_session(app, id, action),
        MenuTarget::Project(id) => dispatch_project(app, id, action),
        MenuTarget::Selection => dispatch_selection(app, action),
    };
    crate::sessions_sidebar::report(app, result);
}

/// Runs a project menu `action` on project `project_id`.
fn dispatch_project(app: &mut App, project_id: &str, action: MenuAction) -> Result<(), String> {
    let project = app
        .sidebar
        .groups
        .iter()
        .find(|g| g.project_id == project_id)
        .map(|g| g.project.clone())
        .ok_or_else(|| "that project is no longer listed".to_string())?;
    let listed = || crate::sessions_sidebar::project_session_ids(app, project_id);
    match action {
        MenuAction::NewSession => {
            app.open_new_session_in_project(&project.into(), None)?;
            app.sidebar.focused = false;
        }
        MenuAction::NewSessionOnBranch => {
            let text = crate::sessions_sidebar::branch_session_command(app, &project);
            prefill(app, &text);
        }
        MenuAction::RenameProject => prefill(
            app,
            &format!("/project rename --project {project_id} {}", project.name),
        ),
        MenuAction::MoveUp | MenuAction::MoveDown => {
            crate::sessions_sidebar::move_project(app, project_id, action == MenuAction::MoveUp);
        }
        MenuAction::SelectAll => {
            let ids = listed();
            app.sidebar.selected = ids.into_iter().collect();
        }
        MenuAction::CollapseAll | MenuAction::ExpandAll => {
            crate::sessions_sidebar::set_all_collapsed(app, action == MenuAction::CollapseAll);
        }
        MenuAction::Archive => {
            let ids = listed();
            app.archive_sessions_by_id(&ids)?;
        }
        MenuAction::Delete => {
            let ids = listed();
            app.request_delete_sessions(ids, &project.name)?;
        }
        MenuAction::Launch(open) => {
            let dir = PathBuf::from(&project.worktree);
            app.open_directory_in(&dir, open)?;
        }
        MenuAction::OpenIn
        | MenuAction::MoveTo
        | MenuAction::Requests
        | MenuAction::OpenUrl(_)
        | MenuAction::OpenRequest
        | MenuAction::ToggleAutofix
        | MenuAction::TogglePin
        | MenuAction::ToggleUnread
        | MenuAction::Rename
        | MenuAction::Fork
        | MenuAction::MoveToSection(_)
        | MenuAction::NewSection
        | MenuAction::ClearSelection => {}
    }
    Ok(())
}

/// Runs a selection menu `action` on every selected session.
fn dispatch_selection(app: &mut App, action: MenuAction) -> Result<(), String> {
    let ids: Vec<String> = app.sidebar.selected.iter().cloned().collect();
    let n = ids.len();
    match action {
        MenuAction::TogglePin => {
            let pin = !ids.iter().all(|id| app.sidebar.pinned.contains(id));
            for id in &ids {
                if app.sidebar.pinned.contains(id) != pin {
                    app.toggle_session_pin(id)?;
                }
            }
            app.status = format!("{n} session(s) {}", if pin { "pinned" } else { "unpinned" });
        }
        MenuAction::ToggleUnread => {
            let is_unread = |app: &App, id: &str| {
                crate::sessions_sidebar::attention_of(app, id) == Attention::Unread
            };
            let unread = !ids.iter().all(|id| is_unread(app, id));
            for id in &ids {
                if is_unread(app, id) != unread {
                    app.toggle_session_unread(id)?;
                }
            }
            app.status = format!(
                "{n} session(s) marked {}",
                if unread { "unread" } else { "read" }
            );
        }
        MenuAction::MoveToSection(section) => {
            for id in &ids {
                app.move_session_to_section(id, section.as_deref())?;
            }
            app.status = format!("{n} session(s) moved");
        }
        MenuAction::ClearSelection => app.sidebar.selected.clear(),
        MenuAction::Archive => app.archive_sessions_by_id(&ids)?,
        MenuAction::Delete => app.request_delete_sessions(ids, "the selection")?,
        MenuAction::OpenIn
        | MenuAction::MoveTo
        | MenuAction::Requests
        | MenuAction::OpenUrl(_)
        | MenuAction::OpenRequest
        | MenuAction::ToggleAutofix
        | MenuAction::Rename
        | MenuAction::Fork
        | MenuAction::Launch(_)
        | MenuAction::NewSection
        | MenuAction::NewSession
        | MenuAction::NewSessionOnBranch
        | MenuAction::RenameProject
        | MenuAction::MoveUp
        | MenuAction::MoveDown
        | MenuAction::SelectAll
        | MenuAction::CollapseAll
        | MenuAction::ExpandAll => {}
    }
    Ok(())
}

/// Puts `text` in the input with the cursor at its end and returns focus
/// to the input.
fn prefill(app: &mut App, text: &str) {
    app.session.input_cursor = text.chars().count();
    app.session.input = text.to_string();
    app.sidebar.focused = false;
}

/// Runs a session menu `action` on `session_id`.
fn dispatch_session(app: &mut App, session_id: &str, action: MenuAction) -> Result<(), String> {
    match action {
        MenuAction::Requests | MenuAction::OpenIn | MenuAction::MoveTo => Ok(()),
        MenuAction::OpenUrl(url) => app.open_url(&url),
        MenuAction::OpenRequest => app.open_session_request(session_id),
        MenuAction::ToggleAutofix => app
            .toggle_session_autofix(session_id)
            .map(|message| app.status = format!("autofix: {message}")),
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
        MenuAction::NewSession
        | MenuAction::NewSessionOnBranch
        | MenuAction::RenameProject
        | MenuAction::MoveUp
        | MenuAction::MoveDown
        | MenuAction::SelectAll
        | MenuAction::ClearSelection
        | MenuAction::CollapseAll
        | MenuAction::ExpandAll => Ok(()),
    }
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
    if !item.enabled {
        return;
    }
    if item.action.opens_submenu() {
        open_submenu(app, index);
        return;
    }
    let (target, action) = (menu.target.clone(), item.action.clone());
    dispatch(app, &target, action);
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
