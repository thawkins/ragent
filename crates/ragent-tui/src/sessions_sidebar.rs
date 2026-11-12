//! Left-hand session sidebar (spec MULTISESSION, milestone M5).
//!
//! Lists every stored session, a `Pinned` section first and then one section
//! per project (the launch project first, the rest by most recent activity),
//! with a `+ New session` row at the top. Each session row shows an attention
//! marker, the title and a relative age; the focused session is highlighted.
//!
//! Data from storage (groups, titles, pins, missing project directories) is
//! cached in [`SidebarState`] and rebuilt only when the state is marked dirty
//! by a session event or command (NFR-03). Attention markers come from the
//! open views in memory, so they are read live each frame.
//!
//! Keys: `Alt+S` shows/focuses/hides the sidebar, `Ctrl+N` opens a session,
//! `Ctrl+Up`/`Ctrl+Down` move to the previous/next session, `Alt+R` returns
//! to the most recently focused one. While the sidebar has focus: arrows and
//! `PageUp`/`PageDown` move, `Enter` acts on the row, `Left`/`Right` collapse
//! and expand a section, `p` pins, `a` archives, `F2` renames, `/` filters,
//! `Esc` clears the filter or returns focus to the input.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ragent_storage::{ProjectSessions, Storage, project_root};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use serde::{Deserialize, Serialize};

use crate::app::{App, Attention};

/// Settings key holding the persisted sidebar layout.
const SIDEBAR_KEY: &str = "tui.sessions.sidebar";
/// Default sidebar width in columns, border included.
pub const DEFAULT_WIDTH: u16 = 32;
/// Narrowest the sidebar can be dragged to.
pub const MIN_WIDTH: u16 = 20;
/// Below this terminal width the sidebar hides itself (FR-10).
pub const MIN_TERMINAL_WIDTH: u16 = 100;
/// Section key of the `Pinned` pseudo-project; NUL keeps it apart from paths.
const PINNED_KEY: &str = "\0pinned";

/// Sidebar layout and cached session data.
#[derive(Debug)]
pub struct SidebarState {
    /// User preference: show the sidebar when the terminal is wide enough.
    pub visible: bool,
    /// Whether the sidebar has keyboard focus.
    pub focused: bool,
    /// Width in columns, border included.
    pub width: u16,
    /// Cursor row, an index into the rows from [`build_rows`].
    pub cursor: usize,
    /// First row shown.
    pub scroll: usize,
    /// Filter text; empty means no filter (FR-09).
    pub filter: String,
    /// Whether keys are currently typed into the filter.
    pub filter_editing: bool,
    /// Section keys (project roots, or the pinned key) that are collapsed.
    pub collapsed: BTreeSet<String>,
    /// Whether the cached storage data must be reloaded before the next use.
    pub dirty: bool,
    /// Cached non-archived top-level sessions grouped by project.
    pub groups: Vec<ProjectSessions>,
    /// Cached ids of pinned sessions.
    pub pinned: HashSet<String>,
    /// Cached project roots whose directory no longer exists.
    pub missing: HashSet<String>,
    /// Area painted last frame; zero-sized when hidden.
    pub area: Rect,
    /// Whether the right border is being dragged to resize.
    pub dragging: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            visible: true,
            focused: false,
            width: DEFAULT_WIDTH,
            cursor: 0,
            scroll: 0,
            filter: String::new(),
            filter_editing: false,
            collapsed: BTreeSet::new(),
            dirty: true,
            groups: Vec::new(),
            pinned: HashSet::new(),
            missing: HashSet::new(),
            area: Rect::default(),
            dragging: false,
        }
    }
}

/// The persisted part of [`SidebarState`].
#[derive(Debug, Serialize, Deserialize)]
struct Persisted {
    visible: bool,
    width: u16,
    collapsed: Vec<String>,
}

impl SidebarState {
    /// Loads the persisted layout, falling back to defaults.
    ///
    /// # Arguments
    ///
    /// * `storage` - the settings store.
    ///
    /// # Returns
    ///
    /// The sidebar state with the saved visibility, width and collapsed
    /// sections applied; defaults when nothing was saved or it is unreadable.
    pub fn load(storage: &Storage) -> Self {
        let mut state = Self::default();
        let saved = match storage.get_setting(SIDEBAR_KEY) {
            Ok(Some(json)) => json,
            Ok(None) => return state,
            Err(e) => {
                tracing::warn!(error = %e, "failed to read sidebar settings");
                return state;
            }
        };
        match serde_json::from_str::<Persisted>(&saved) {
            Ok(p) => {
                state.visible = p.visible;
                state.width = p.width.max(MIN_WIDTH);
                state.collapsed = p.collapsed.into_iter().collect();
            }
            Err(e) => tracing::warn!(error = %e, "ignoring malformed sidebar settings"),
        }
        state
    }

    /// Saves visibility, width and collapsed sections.
    ///
    /// # Arguments
    ///
    /// * `storage` - the settings store. A failure is logged only.
    pub fn save(&self, storage: &Storage) {
        let persisted = Persisted {
            visible: self.visible,
            width: self.width,
            collapsed: self.collapsed.iter().cloned().collect(),
        };
        match serde_json::to_string(&persisted) {
            Ok(json) => {
                if let Err(e) = storage.set_setting(SIDEBAR_KEY, &json) {
                    tracing::warn!(error = %e, "failed to save sidebar settings");
                }
            }
            Err(e) => tracing::warn!(error = %e, "failed to encode sidebar settings"),
        }
    }
}

/// One painted row of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
    /// The `+ New session` action.
    NewSession,
    /// A section heading.
    Header {
        /// Project root, or the pinned key.
        key: String,
        /// Text shown for the section.
        label: String,
        /// Sessions in the section (after filtering).
        count: usize,
        /// Whether the section's sessions are hidden.
        collapsed: bool,
        /// Whether the project directory no longer exists.
        missing: bool,
    },
    /// A session.
    Session {
        /// Full session id.
        id: String,
        /// Text shown for the session.
        title: String,
        /// RFC 3339 last-activity time.
        updated_at: String,
    },
}

/// Splits the frame into the sidebar and the chat area.
///
/// # Arguments
///
/// * `app` - for the sidebar preference and width.
/// * `area` - the whole frame.
///
/// # Returns
///
/// `(Some(sidebar), chat)` when the sidebar is shown, `(None, area)` when it
/// is turned off or the terminal is narrower than [`MIN_TERMINAL_WIDTH`].
pub fn split_area(app: &App, area: Rect) -> (Option<Rect>, Rect) {
    if !app.sidebar.visible || area.width < MIN_TERMINAL_WIDTH {
        return (None, area);
    }
    let width = app.sidebar.width.clamp(MIN_WIDTH, area.width / 2);
    let sidebar = Rect { width, ..area };
    let chat = Rect {
        x: area.x + width,
        width: area.width - width,
        ..area
    };
    (Some(sidebar), chat)
}

/// Reloads the cached storage data when it is marked dirty.
pub fn refresh_if_dirty(app: &mut App) {
    if !app.sidebar.dirty {
        return;
    }
    match (
        app.storage.list_sessions_grouped(),
        app.storage.pinned_session_ids(),
    ) {
        (Ok(groups), Ok(pinned)) => {
            app.sidebar.missing = groups
                .iter()
                .filter(|g| !Path::new(&g.project_id).exists())
                .map(|g| g.project_id.clone())
                .collect();
            app.sidebar.groups = groups;
            app.sidebar.pinned = pinned;
            app.sidebar.dirty = false;
        }
        (Err(e), _) | (_, Err(e)) => {
            // Keep the stale rows and retry on the next frame.
            tracing::warn!(error = %e, "session sidebar refresh failed");
        }
    }
}

/// Section labels: each project's directory name, or `parent/name` when two
/// projects share a name.
fn project_labels(groups: &[ProjectSessions]) -> HashMap<String, String> {
    let base = |id: &str| {
        Path::new(id)
            .file_name()
            .map_or_else(|| id.to_string(), |n| n.to_string_lossy().into_owned())
    };
    let mut counts: HashMap<String, usize> = HashMap::new();
    for g in groups {
        *counts.entry(base(&g.project_id)).or_default() += 1;
    }
    groups
        .iter()
        .map(|g| {
            let name = base(&g.project_id);
            let label = if counts.get(&name).copied().unwrap_or(0) > 1 {
                let parent = Path::new(&g.project_id)
                    .parent()
                    .and_then(Path::file_name)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{parent}/{name}")
            } else {
                name
            };
            (g.project_id.clone(), label)
        })
        .collect()
}

/// Display title for a session: its stored title, or `untitled <id tail>`.
fn display_title(id: &str, title: &str) -> String {
    if title.is_empty() {
        let tail: String = id
            .chars()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("untitled {tail}")
    } else {
        title.to_string()
    }
}

/// Builds the rows the sidebar shows, honouring pins, collapse and filter.
///
/// # Arguments
///
/// * `app` - for the cached data, sidebar state and launch directory.
///
/// # Returns
///
/// The rows in display order, starting with [`SidebarRow::NewSession`].
pub fn build_rows(app: &App) -> Vec<SidebarRow> {
    let state = &app.sidebar;
    let filter = state.filter.to_lowercase();
    let labels = project_labels(&state.groups);
    let launch = project_root(&app.cwd_path.display().to_string());
    let mut rows = vec![SidebarRow::NewSession];

    let mut pinned: Vec<(&ragent_storage::SessionRow, &str)> = state
        .groups
        .iter()
        .flat_map(|g| {
            let label = labels.get(&g.project_id).map_or("", String::as_str);
            g.sessions.iter().map(move |s| (s, label))
        })
        .filter(|(s, _)| state.pinned.contains(&s.id))
        .collect();
    pinned.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
    push_section(
        &mut rows, app, PINNED_KEY, "Pinned", &pinned, &filter, false,
    );

    let mut ordered: Vec<&ProjectSessions> = state.groups.iter().collect();
    // Stable sort: the launch project first, the rest keep recency order.
    ordered.sort_by_key(|g| g.project_id != launch);
    for group in ordered {
        let label = labels.get(&group.project_id).map_or("", String::as_str);
        let sessions: Vec<(&ragent_storage::SessionRow, &str)> = group
            .sessions
            .iter()
            .filter(|s| !state.pinned.contains(&s.id))
            .map(|s| (s, label))
            .collect();
        let missing = state.missing.contains(&group.project_id);
        push_section(
            &mut rows,
            app,
            &group.project_id,
            label,
            &sessions,
            &filter,
            missing,
        );
    }
    rows
}

/// Appends one section (heading plus visible sessions) to `rows`.
fn push_section(
    rows: &mut Vec<SidebarRow>,
    app: &App,
    key: &str,
    label: &str,
    sessions: &[(&ragent_storage::SessionRow, &str)],
    filter: &str,
    missing: bool,
) {
    let matching: Vec<&ragent_storage::SessionRow> = sessions
        .iter()
        .filter(|(s, project)| {
            filter.is_empty()
                || s.title.to_lowercase().contains(filter)
                || project.to_lowercase().contains(filter)
        })
        .map(|(s, _)| *s)
        .collect();
    if matching.is_empty() {
        return;
    }
    // A filter shows every match, collapsed or not.
    let collapsed = filter.is_empty() && app.sidebar.collapsed.contains(key);
    rows.push(SidebarRow::Header {
        key: key.to_string(),
        label: label.to_string(),
        count: matching.len(),
        collapsed,
        missing,
    });
    if !collapsed {
        rows.extend(matching.into_iter().map(|s| SidebarRow::Session {
            id: s.id.clone(),
            title: display_title(&s.id, &s.title),
            updated_at: s.updated_at.clone(),
        }));
    }
}

/// Compact age of an RFC 3339 timestamp: `now`, `5m`, `3h`, `4d` or `2w`.
///
/// # Arguments
///
/// * `timestamp` - RFC 3339 time.
/// * `now` - the reference time.
///
/// # Returns
///
/// The age, or an empty string when `timestamp` does not parse.
pub fn relative_age(timestamp: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    let Ok(then) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return String::new();
    };
    let secs = (now - then.with_timezone(&chrono::Utc))
        .num_seconds()
        .max(0);
    match secs {
        0..60 => "now".to_string(),
        60..3_600 => format!("{}m", secs / 60),
        3_600..86_400 => format!("{}h", secs / 3_600),
        86_400..604_800 => format!("{}d", secs / 86_400),
        _ => format!("{}w", secs / 604_800),
    }
}

/// Attention of the session `id` if it is open, else `Idle`.
fn attention_of(app: &App, id: &str) -> Attention {
    if app.session.session_id.as_deref() == Some(id) {
        return app.session.attention();
    }
    app.background
        .iter()
        .find(|v| v.session_id.as_deref() == Some(id))
        .map_or(Attention::Idle, |v| v.attention())
}

/// Marker and colour for an attention state (FR-04, ASCII only).
fn attention_marker(attention: Attention) -> (char, Color) {
    match attention {
        Attention::Waiting => ('!', Color::Yellow),
        Attention::Running => ('*', Color::Green),
        Attention::Error => ('x', Color::Red),
        Attention::Unread => ('o', Color::Cyan),
        Attention::Idle => (' ', Color::Reset),
    }
}

/// Truncates `text` to `max` characters, marking a cut with `..`.
fn fit(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let keep = max.saturating_sub(2);
    let mut out: String = text.chars().take(keep).collect();
    out.push_str(&"..".chars().take(max - keep).collect::<String>());
    out
}

/// Paints the sidebar into `area`.
///
/// # Arguments
///
/// * `frame` - the frame being drawn.
/// * `app` - sidebar state and sessions; the painted area is recorded.
/// * `area` - the sidebar rectangle from [`split_area`].
pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    refresh_if_dirty(app);
    app.sidebar.area = area;
    let title = if app.sidebar.filter_editing || !app.sidebar.filter.is_empty() {
        let caret = if app.sidebar.filter_editing { "_" } else { "" };
        format!(" SESSIONS /{}{caret} ", app.sidebar.filter)
    } else {
        " SESSIONS ".to_string()
    };
    let border = if app.sidebar.focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = build_rows(app);
    let height = usize::from(inner.height);
    let state = &mut app.sidebar;
    state.cursor = state.cursor.min(rows.len().saturating_sub(1));
    if state.cursor < state.scroll {
        state.scroll = state.cursor;
    } else if height > 0 && state.cursor >= state.scroll + height {
        state.scroll = state.cursor + 1 - height;
    }
    state.scroll = state.scroll.min(rows.len().saturating_sub(height.max(1)));
    let (scroll, cursor, focused) = (state.scroll, state.cursor, state.focused);

    let now = chrono::Utc::now();
    let width = usize::from(inner.width);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .skip(scroll)
        .take(height)
        .map(|(index, row)| {
            let mut line = row_line(app, row, width, now);
            if focused && index == cursor {
                line = line.patch_style(Style::default().bg(Color::DarkGray));
            }
            line
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Builds the painted line for one row.
fn row_line(
    app: &App,
    row: &SidebarRow,
    width: usize,
    now: chrono::DateTime<chrono::Utc>,
) -> Line<'static> {
    match row {
        SidebarRow::NewSession => Line::from(Span::styled(
            fit(" + New session", width),
            Style::default().fg(Color::Green),
        )),
        SidebarRow::Header {
            label,
            count,
            collapsed,
            missing,
            ..
        } => {
            let arrow = if *collapsed { '>' } else { 'v' };
            let suffix = if *missing { " (missing)" } else { "" };
            Line::from(Span::styled(
                fit(&format!("{arrow} {label} ({count}){suffix}"), width),
                Style::default()
                    .fg(if *missing {
                        Color::DarkGray
                    } else {
                        Color::Gray
                    })
                    .add_modifier(Modifier::BOLD),
            ))
        }
        SidebarRow::Session {
            id,
            title,
            updated_at,
        } => {
            let (marker, colour) = attention_marker(attention_of(app, id));
            let age = relative_age(updated_at, now);
            // "  m " prefix, title, one space, age.
            let title_width = width.saturating_sub(4 + age.len() + 1);
            let title = fit(title, title_width);
            let pad = title_width.saturating_sub(title.chars().count());
            let focused = app.session.session_id.as_deref() == Some(id.as_str());
            let title_style = if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(marker.to_string(), Style::default().fg(colour)),
                Span::raw(" "),
                Span::styled(title, title_style),
                Span::raw(" ".repeat(pad + 1)),
                Span::styled(age, Style::default().fg(Color::DarkGray)),
            ])
        }
    }
}

/// Session ids in sidebar order (pins first), ignoring collapse and filter.
fn ordered_session_ids(app: &App) -> Vec<String> {
    let state = &app.sidebar;
    let launch = project_root(&app.cwd_path.display().to_string());
    let mut pinned: Vec<&ragent_storage::SessionRow> = state
        .groups
        .iter()
        .flat_map(|g| g.sessions.iter())
        .filter(|s| state.pinned.contains(&s.id))
        .collect();
    pinned.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    let mut ordered: Vec<&ProjectSessions> = state.groups.iter().collect();
    ordered.sort_by_key(|g| g.project_id != launch);
    pinned
        .into_iter()
        .chain(
            ordered
                .into_iter()
                .flat_map(|g| g.sessions.iter())
                .filter(|s| !state.pinned.contains(&s.id)),
        )
        .map(|s| s.id.clone())
        .collect()
}

/// Reports a failed session action on the status line.
fn report(app: &mut App, result: Result<(), String>) {
    if let Err(message) = result {
        app.status = format!("session: {message}");
    }
    app.sidebar.dirty = true;
}

/// Focuses the session `step` places away in sidebar order (FR-12),
/// skipping sessions that cannot be opened from this directory.
fn cycle_session(app: &mut App, forward: bool) {
    refresh_if_dirty(app);
    let ids = ordered_session_ids(app);
    if ids.is_empty() {
        return;
    }
    let current = app
        .session
        .session_id
        .as_deref()
        .and_then(|id| ids.iter().position(|s| s == id));
    let len = ids.len();
    let start = current.unwrap_or(if forward { len - 1 } else { 0 });
    let mut last_error = None;
    for step in 1..=len {
        let index = if forward {
            (start + step) % len
        } else {
            (start + len - step % len) % len
        };
        match app.switch_to_session(&ids[index]) {
            Ok(()) => {
                app.sidebar.dirty = true;
                return;
            }
            Err(e) => last_error = Some(e),
        }
    }
    if let Some(e) = last_error {
        app.status = format!("session: {e}");
    }
}

/// Shows and focuses, focuses, or hides the sidebar (`Alt+S`, FR-10).
pub fn toggle(app: &mut App) {
    let state = &mut app.sidebar;
    if !state.visible {
        state.visible = true;
        state.focused = true;
    } else if !state.focused {
        state.focused = true;
    } else {
        state.visible = false;
        state.focused = false;
    }
    app.sidebar.save(&app.storage);
}

/// Acts on the row at `index`: open a session, toggle a section, or switch.
fn activate_row(app: &mut App, index: usize) {
    let rows = build_rows(app);
    match rows.get(index) {
        Some(SidebarRow::NewSession) => {
            let result = app.open_new_session();
            report(app, result);
            app.sidebar.focused = false;
        }
        Some(SidebarRow::Header { key, .. }) => toggle_section(app, key, None),
        Some(SidebarRow::Session { id, .. }) => {
            let result = app.switch_to_session(id);
            report(app, result);
            app.sidebar.focused = false;
        }
        None => {}
    }
}

/// Collapses, expands (`Some`) or toggles (`None`) a section and saves it.
fn toggle_section(app: &mut App, key: &str, collapse: Option<bool>) {
    let collapsed = &mut app.sidebar.collapsed;
    let want = collapse.unwrap_or(!collapsed.contains(key));
    if want {
        collapsed.insert(key.to_string());
    } else {
        collapsed.remove(key);
    }
    app.sidebar.save(&app.storage);
}

/// The session id under the cursor, if the cursor is on a session row.
fn cursor_session(app: &App) -> Option<String> {
    match build_rows(app).get(app.sidebar.cursor) {
        Some(SidebarRow::Session { id, .. }) => Some(id.clone()),
        _ => None,
    }
}

/// Handles a key in chat mode.
///
/// # Arguments
///
/// * `app` - the application.
/// * `key` - the key event (after every modal dialog had its chance).
///
/// # Returns
///
/// `true` when the sidebar consumed the key.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('s') if alt => {
            toggle(app);
            return true;
        }
        KeyCode::Char('n') if ctrl => {
            let result = app.open_new_session();
            report(app, result);
            return true;
        }
        KeyCode::Up if ctrl => {
            cycle_session(app, false);
            return true;
        }
        KeyCode::Down if ctrl => {
            cycle_session(app, true);
            return true;
        }
        KeyCode::Char('r') if alt => {
            let recent = app
                .background
                .iter()
                .next()
                .and_then(|v| v.session_id.clone());
            match recent {
                Some(id) => {
                    let result = app.switch_to_session(&id);
                    report(app, result);
                }
                None => app.status = "session: no other open session".to_string(),
            }
            return true;
        }
        _ => {}
    }
    if !app.sidebar.focused || !app.sidebar.visible {
        return false;
    }
    if app.sidebar.filter_editing {
        match key.code {
            KeyCode::Char(c) if !ctrl && !alt => app.sidebar.filter.push(c),
            KeyCode::Backspace => {
                app.sidebar.filter.pop();
            }
            KeyCode::Enter | KeyCode::Esc => app.sidebar.filter_editing = false,
            _ => {}
        }
        app.sidebar.cursor = 0;
        return true;
    }
    let page = usize::from(app.sidebar.area.height.saturating_sub(2)).max(1);
    let last = build_rows(app).len().saturating_sub(1);
    let state = &mut app.sidebar;
    match key.code {
        KeyCode::Up => state.cursor = state.cursor.saturating_sub(1),
        KeyCode::Down => state.cursor = (state.cursor + 1).min(last),
        KeyCode::PageUp => state.cursor = state.cursor.saturating_sub(page),
        KeyCode::PageDown => state.cursor = (state.cursor + page).min(last),
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = last,
        KeyCode::Enter | KeyCode::Char(' ') => activate_row(app, app.sidebar.cursor),
        KeyCode::Left | KeyCode::Right => {
            if let Some(SidebarRow::Header { key: section, .. }) =
                build_rows(app).get(app.sidebar.cursor)
            {
                let section = section.clone();
                toggle_section(app, &section, Some(key.code == KeyCode::Left));
            }
        }
        KeyCode::Char('/') => {
            state.filter_editing = true;
            state.filter.clear();
        }
        KeyCode::Char('p') => {
            if let Some(id) = cursor_session(app) {
                let result = app.toggle_session_pin(&id);
                report(app, result);
            }
        }
        KeyCode::Char('a') => {
            if let Some(id) = cursor_session(app) {
                let result = app.archive_session_by_id(&id);
                report(app, result);
            }
        }
        KeyCode::F(2) => {
            if let Some(id) = cursor_session(app) {
                let result = app.switch_to_session(&id);
                let ok = result.is_ok();
                report(app, result);
                if ok {
                    let title = match app.storage.get_session(&id) {
                        Ok(row) => row.map(|r| r.title),
                        Err(e) => {
                            tracing::warn!(error = %e, "could not read session title");
                            None
                        }
                    };
                    let text = format!("/session rename {}", title.unwrap_or_default());
                    app.session.input_cursor = text.chars().count();
                    app.session.input = text;
                    app.sidebar.focused = false;
                }
            }
        }
        KeyCode::Esc if !state.filter.is_empty() => {
            state.filter.clear();
            state.cursor = 0;
        }
        KeyCode::Esc | KeyCode::Tab => state.focused = false,
        _ => {}
    }
    true
}

/// Handles a mouse event.
///
/// # Arguments
///
/// * `app` - the application.
/// * `event` - the mouse event (after modal overlays had their chance).
///
/// # Returns
///
/// `true` when the sidebar consumed the event.
pub fn handle_mouse(app: &mut App, event: MouseEvent) -> bool {
    let area = app.sidebar.area;
    if app.sidebar.dragging {
        match event.kind {
            MouseEventKind::Drag(MouseButton::Left) => {
                app.sidebar.width = (event.column.saturating_sub(area.x) + 1).max(MIN_WIDTH);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                app.sidebar.dragging = false;
                app.sidebar.save(&app.storage);
            }
            _ => {}
        }
        return true;
    }
    let inside = area.width > 0 && area.contains((event.column, event.row).into());
    if !inside {
        if matches!(event.kind, MouseEventKind::Down(_)) {
            app.sidebar.focused = false;
        }
        return false;
    }
    let border_x = area.x + area.width - 1;
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) if event.column == border_x => {
            app.sidebar.dragging = true;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.sidebar.focused = true;
            let top = area.y + 1;
            if event.row >= top && event.row < area.y + area.height - 1 {
                let index = app.sidebar.scroll + usize::from(event.row - top);
                if index < build_rows(app).len() {
                    app.sidebar.cursor = index;
                    activate_row(app, index);
                }
            }
        }
        MouseEventKind::ScrollUp => {
            app.sidebar.scroll = app.sidebar.scroll.saturating_sub(1);
            app.sidebar.cursor = app.sidebar.cursor.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            let last = build_rows(app).len().saturating_sub(1);
            app.sidebar.cursor = (app.sidebar.cursor + 1).min(last);
        }
        _ => {}
    }
    true
}
