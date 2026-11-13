//! Left-hand session sidebar (spec MULTISESSION, milestone M5; spec
//! `projects`).
//!
//! Lists every stored session, a `Pinned` section first and then one section
//! per project (the launch project first, the rest by most recent activity),
//! under a `+ New session` row and a `/ Search` row. A project heading shows
//! the project's name, a chevron (`v` open, `>` collapsed) and its remote
//! host kind; a project with no sessions is listed too. Each session row
//! shows the title, a relative age, the branch marker with the count of
//! pull/merge requests for the session's branch, and an attention marker at
//! the right edge; the focused session is highlighted.
//!
//! Data from storage (groups, titles, pins, branches, request counts,
//! hidden projects) is cached in [`SidebarState`] and rebuilt
//! only when the state is marked dirty by a session or project event or a
//! command (NFR-03). Attention markers come from the open views in memory,
//! so they are read live each frame.
//!
//! Keys: `Alt+S` shows/focuses/hides the sidebar, `Ctrl+N` opens a session,
//! `Ctrl+Up`/`Ctrl+Down` move to the previous/next session, `Alt+R` returns
//! to the most recently focused one. While the sidebar has focus: arrows and
//! `PageUp`/`PageDown` move, `Enter` acts on the row, `Left`/`Right` collapse
//! and expand a section, `p` pins, `a` archives, `F2` renames, `/` filters,
//! `u` marks unread/read, `f` forks, `D` deletes (after a confirmation),
//! `Esc` clears the filter or returns focus to the input. On a project
//! heading `n` starts a session in that project and `b` prefills the command
//! that starts one in its own worktree at a branch. `m`, `Shift+F10` or a
//! right-click on a session row opens its context menu, and on a project
//! heading the project's menu ([`crate::session_menu`]). `v` or
//! `Ctrl`+click adds a session to the selection (or takes it out); `m` on a
//! selected session opens a menu acting on the whole selection, and `Esc`
//! clears it. Projects keep a manual order once one is moved up or down.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ragent_storage::{ProjectSessions, SessionCheckout, SidebarSection, Storage, project_root};
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
/// Section key of the `Pinned` pseudo-project; NUL keeps it apart from ids.
const PINNED_KEY: &str = "\0pinned";
/// Background of the open session's row when the cursor is elsewhere.
const OPEN_SESSION_BG: Color = Color::Indexed(236);
/// ASCII marker shown before a session's request count; it stands for a
/// branch.
pub const BRANCH_MARK: char = 'Y';
/// Narrowest title a session row keeps before it gives up its age column.
const MIN_TITLE_WIDTH: usize = 12;

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
    /// Section keys (project ids, or the pinned key) that are collapsed.
    pub collapsed: BTreeSet<String>,
    /// Whether the cached storage data must be reloaded before the next use.
    pub dirty: bool,
    /// Cached non-archived top-level sessions grouped by project.
    pub groups: Vec<ProjectSessions>,
    /// Cached ids of pinned sessions.
    pub pinned: HashSet<String>,
    /// Cached ids of sessions marked unread in storage (SESSIONMENU FR-08).
    pub unread: HashSet<String>,
    /// Cached bound sessions, each with whether Auto-fix is paused waiting
    /// on the user (SESSIONMENU FR-22).
    pub bound: HashMap<String, bool>,
    /// Cached number of pull/merge requests each session has had (created
    /// or bound); sessions with none are absent.
    pub request_totals: HashMap<String, usize>,
    /// Cached user-defined sections, in display order (SESSIONMENU FR-11).
    pub sections: Vec<SidebarSection>,
    /// Cached section id of each session placed in a section.
    pub session_section: HashMap<String, String>,
    /// Cached ids of projects not listed: the directory no longer exists,
    /// or it is a parent folder (not a git repository, holding another
    /// project) with no open session. The launch project is listed
    /// regardless.
    pub hidden: HashSet<String>,
    /// Cached branch, worktree and request count of each session that has a
    /// branch (spec `projects`).
    pub checkouts: HashMap<String, SessionCheckout>,
    /// Area painted last frame; zero-sized when hidden.
    pub area: Rect,
    /// Whether the right border is being dragged to resize.
    pub dragging: bool,
    /// The session whose row the left button went down on; releasing it
    /// over the `Pinned` heading pins that session.
    pub drag_session: Option<String>,
    /// The open session context menu, if any (spec SESSIONMENU).
    pub menu: Option<crate::session_menu::SessionMenu>,
    /// Ids of the sessions in the multi-selection (not persisted).
    pub selected: BTreeSet<String>,
    /// Manual project order (project ids, top first), set by the project
    /// menu's `Move up`/`Move down`; empty means the default order.
    pub project_order: Vec<String>,
    /// Test hook: when set, `Open in` records what it would start here
    /// instead of starting it.
    #[doc(hidden)]
    pub launch_log: Option<Vec<crate::open_in::LaunchSpec>>,
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
            unread: HashSet::new(),
            bound: HashMap::new(),
            request_totals: HashMap::new(),
            sections: Vec::new(),
            session_section: HashMap::new(),
            hidden: HashSet::new(),
            checkouts: HashMap::new(),
            area: Rect::default(),
            dragging: false,
            drag_session: None,
            menu: None,
            selected: BTreeSet::new(),
            project_order: Vec::new(),
            launch_log: None,
        }
    }
}

/// The persisted part of [`SidebarState`].
#[derive(Debug, Serialize, Deserialize)]
struct Persisted {
    visible: bool,
    width: u16,
    collapsed: Vec<String>,
    /// Manual project order; absent in layouts saved before it existed.
    #[serde(default)]
    project_order: Vec<String>,
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
                state.project_order = p.project_order;
            }
            Err(e) => tracing::warn!(error = %e, "ignoring malformed sidebar settings"),
        }
        state
    }

    /// Saves visibility, width, collapsed sections and the project order.
    ///
    /// # Arguments
    ///
    /// * `storage` - the settings store. A failure is logged only.
    pub fn save(&self, storage: &Storage) {
        let persisted = Persisted {
            visible: self.visible,
            width: self.width,
            collapsed: self.collapsed.iter().cloned().collect(),
            project_order: self.project_order.clone(),
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
    /// A blank line above the `Pinned` heading and each project heading.
    /// It cannot be selected: the cursor skips it and a click does nothing.
    Spacer,
    /// The `/ Search` action: starts the filter.
    Search,
    /// A section heading.
    Header {
        /// Project id, or the pinned key.
        key: String,
        /// Text shown for the section.
        label: String,
        /// Sessions in the section (after filtering).
        count: usize,
        /// Whether the section's sessions are hidden.
        collapsed: bool,
        /// The project's remote host kind (`github`, `gitlab`), if any.
        remote: Option<String>,
    },
    /// A session.
    Session {
        /// Full session id.
        id: String,
        /// Text shown for the session.
        title: String,
        /// RFC 3339 last-activity time.
        updated_at: String,
        /// The session's branch, if it has one.
        branch: Option<String>,
        /// Whether the session owns its git worktree.
        owns_worktree: bool,
        /// Pull/merge requests whose source is `branch`; `None` when unknown.
        requests: Option<i64>,
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
    match app.storage.bound_sessions() {
        Ok(bound) => app.sidebar.bound = bound,
        Err(e) => tracing::warn!(error = %e, "session sidebar: bound sessions not read"),
    }
    match app.storage.session_request_totals() {
        Ok(totals) => app.sidebar.request_totals = totals,
        Err(e) => tracing::warn!(error = %e, "session sidebar: request totals not read"),
    }
    match (
        app.storage.list_sessions_grouped(),
        app.storage.pinned_session_ids(),
        app.storage.session_checkouts(),
        app.storage.unread_session_ids(),
        app.storage.list_sections(),
        app.storage.session_sections(),
    ) {
        (Ok(groups), Ok(pinned), Ok(checkouts), Ok(unread), Ok(sections), Ok(session_section)) => {
            app.sidebar.hidden = groups
                .iter()
                .filter(|g| {
                    let dir = Path::new(&g.project.worktree);
                    // A parent folder: holds another project, is no repo itself.
                    let container = || {
                        !dir.join(".git").exists()
                            && groups.iter().any(|other| {
                                let inner = Path::new(&other.project.worktree);
                                inner != dir && inner.starts_with(dir)
                            })
                    };
                    !dir.exists() || (g.sessions.is_empty() && container())
                })
                .map(|g| g.project_id.clone())
                .collect();
            // A session that is gone (archived, deleted) leaves the selection.
            app.sidebar.selected.retain(|id| {
                groups
                    .iter()
                    .any(|g| g.sessions.iter().any(|s| s.id == *id))
            });
            app.sidebar.groups = groups;
            app.sidebar.pinned = pinned;
            app.sidebar.checkouts = checkouts;
            app.sidebar.unread = unread;
            app.sidebar.sections = sections;
            app.sidebar.session_section = session_section;
            app.sidebar.dirty = false;
        }
        (Err(e), ..)
        | (_, Err(e), ..)
        | (_, _, Err(e), ..)
        | (.., Err(e), _, _)
        | (.., Err(e), _)
        | (.., Err(e)) => {
            // Keep the stale rows and retry on the next frame.
            tracing::warn!(error = %e, "session sidebar refresh failed");
        }
    }
}

/// Section labels: each project's name, or `parent/name` (the parent being
/// the directory that holds the project's worktree) when two projects share
/// a name.
fn project_labels(groups: &[ProjectSessions]) -> HashMap<String, String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for g in groups {
        *counts.entry(g.project.name.as_str()).or_default() += 1;
    }
    groups
        .iter()
        .map(|g| {
            let name = g.project.name.as_str();
            let label = if counts.get(name).copied().unwrap_or(0) > 1 {
                let parent = Path::new(&g.project.worktree)
                    .parent()
                    .and_then(Path::file_name)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{parent}/{name}")
            } else {
                name.to_string()
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
    let filter = app.sidebar.filter.to_lowercase();
    let mut rows = vec![SidebarRow::NewSession, SidebarRow::Search];
    for section in sections(app) {
        push_section(&mut rows, app, &section, &filter);
    }
    rows
}

/// Collapse key of a user-defined section; NUL keeps it apart from paths.
pub fn user_section_key(section_id: &str) -> String {
    format!("\0section:{section_id}")
}

/// One sidebar section before filtering and collapse.
struct SectionData<'a> {
    /// Collapse key: a project root, the pinned key or a user section key.
    key: String,
    /// Heading text.
    label: String,
    /// Sessions, each with its project label (matched by the filter).
    sessions: Vec<(&'a ragent_storage::SessionRow, String)>,
    /// Forge kind of the project's remote (`github`, `gitlab`), if any.
    remote: Option<String>,
    /// Whether the heading is shown when the section has no sessions.
    show_empty: bool,
    /// Whether a blank line goes above the heading (`Pinned` and projects).
    spaced: bool,
}

/// Every section in display order: `Pinned`, the user sections by name,
/// then the projects (the launch project first, the rest by recency).
/// A pinned session is listed only under `Pinned`, and a session in a user
/// section only there.
fn sections(app: &App) -> Vec<SectionData<'_>> {
    let state = &app.sidebar;
    let labels = project_labels(&state.groups);
    let launch = project_root(&app.cwd_path.display().to_string());
    let all = || {
        state.groups.iter().flat_map(|g| {
            let label = labels.get(&g.project_id).cloned().unwrap_or_default();
            g.sessions.iter().map(move |s| (s, label.clone()))
        })
    };
    let newest_first = |list: &mut Vec<(&ragent_storage::SessionRow, String)>| {
        list.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
    };
    let mut out = Vec::new();

    let mut pinned: Vec<_> = all()
        .filter(|(s, _)| state.pinned.contains(&s.id))
        .collect();
    newest_first(&mut pinned);
    out.push(SectionData {
        key: PINNED_KEY.to_string(),
        label: "Pinned".to_string(),
        sessions: pinned,
        remote: None,
        // Always listed, so a session can be dragged onto it to pin it.
        show_empty: true,
        spaced: true,
    });

    let placed = |id: &str| state.session_section.get(id);
    for section in &state.sections {
        let mut sessions: Vec<_> = all()
            .filter(|(s, _)| !state.pinned.contains(&s.id) && placed(&s.id) == Some(&section.id))
            .collect();
        newest_first(&mut sessions);
        out.push(SectionData {
            key: user_section_key(&section.id),
            label: section.name.clone(),
            sessions,
            remote: None,
            show_empty: false,
            spaced: false,
        });
    }

    // A hidden project is not listed, unless ragent was launched in it.
    let mut ordered: Vec<&ProjectSessions> = state
        .groups
        .iter()
        .filter(|g| !state.hidden.contains(&g.project_id) || g.project.worktree == launch)
        .collect();
    // Stable sort: the launch project first, the rest keep recency order.
    ordered.sort_by_key(|g| g.project.worktree != launch);
    // A manual order wins; projects not in it follow in the order above.
    if !state.project_order.is_empty() {
        let place = |g: &&ProjectSessions| {
            state
                .project_order
                .iter()
                .position(|id| *id == g.project_id)
                .unwrap_or(usize::MAX)
        };
        ordered.sort_by_key(place);
    }
    for group in ordered {
        let label = labels.get(&group.project_id).cloned().unwrap_or_default();
        let sessions = group
            .sessions
            .iter()
            .filter(|s| !state.pinned.contains(&s.id) && placed(&s.id).is_none())
            .map(|s| (s, label.clone()))
            .collect();
        out.push(SectionData {
            key: group.project_id.clone(),
            label,
            sessions,
            remote: group.project.remote.as_ref().map(|r| r.kind.clone()),
            // A project with no sessions is still listed, so a session
            // can be started in it (spec `projects`).
            show_empty: true,
            spaced: true,
        });
    }
    out
}

/// Appends one section (heading plus visible sessions) to `rows`.
fn push_section(rows: &mut Vec<SidebarRow>, app: &App, section: &SectionData<'_>, filter: &str) {
    let matching: Vec<&ragent_storage::SessionRow> = section
        .sessions
        .iter()
        .filter(|(s, project)| {
            filter.is_empty()
                || s.title.to_lowercase().contains(filter)
                || project.to_lowercase().contains(filter)
        })
        .map(|(s, _)| *s)
        .collect();
    // While filtering only sections with a match are shown.
    // `Pinned` stays even then: it is the drop target for drag-to-pin.
    let always = section.key == PINNED_KEY;
    if matching.is_empty() && !always && !(section.show_empty && filter.is_empty()) {
        return;
    }
    // A filter shows every match, collapsed or not.
    let collapsed = filter.is_empty() && app.sidebar.collapsed.contains(&section.key);
    if section.spaced {
        rows.push(SidebarRow::Spacer);
    }
    rows.push(SidebarRow::Header {
        key: section.key.clone(),
        label: section.label.clone(),
        count: matching.len(),
        collapsed,
        remote: section.remote.clone(),
    });
    if !collapsed {
        rows.extend(matching.into_iter().map(|s| {
            let checkout = app.sidebar.checkouts.get(&s.id);
            SidebarRow::Session {
                id: s.id.clone(),
                title: display_title(&s.id, &s.title),
                updated_at: s.updated_at.clone(),
                branch: checkout.and_then(|c| c.branch.clone()),
                owns_worktree: checkout.is_some_and(|c| c.worktree.is_some()),
                requests: checkout.and_then(|c| c.pr_count),
            }
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

/// Attention of the session `id`: its open view's state, or `Unread` when
/// an otherwise idle session is marked unread in storage.
pub(crate) fn attention_of(app: &App, id: &str) -> Attention {
    let live = if app.session.session_id.as_deref() == Some(id) {
        app.session.attention()
    } else {
        app.background
            .iter()
            .find(|v| v.session_id.as_deref() == Some(id))
            .map_or(Attention::Idle, |v| v.attention())
    };
    if live == Attention::Idle && app.sidebar.unread.contains(id) {
        Attention::Unread
    } else {
        live
    }
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

/// An action row: `text` in `colour`, with its shortcut `hint` dimmed at the
/// right edge when there is room for both.
fn with_hint(text: &str, colour: Color, hint: &str, width: usize) -> Line<'static> {
    let text_width = text.chars().count();
    let hint_width = hint.chars().count();
    let mut spans = vec![Span::styled(fit(text, width), Style::default().fg(colour))];
    if !hint.is_empty() && text_width + 1 + hint_width < width {
        spans.push(Span::raw(" ".repeat(width - text_width - hint_width - 1)));
        spans.push(Span::styled(
            hint.to_string(),
            Style::default().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
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
        let total: usize = app.sidebar.groups.iter().map(|g| g.sessions.len()).sum();
        format!(" SESSIONS ({total}) ")
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
    // Rebuilt rows can put a spacer under the cursor (unpinning the only
    // pinned session, say); move to the next selectable row.
    state.cursor = selectable(&rows, state.cursor.min(rows.len().saturating_sub(1)), true);
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
            let open_session = matches!(
                row,
                SidebarRow::Session { id, .. } if app.session.session_id.as_deref() == Some(id)
            );
            if focused && index == cursor {
                // Pad to the full width so the bar spans the whole row.
                let pad = width.saturating_sub(line.width());
                if pad > 0 {
                    line.spans.push(Span::raw(" ".repeat(pad)));
                }
                line = line.patch_style(Style::default().bg(Color::DarkGray));
            } else if open_session {
                // The open session keeps a soft full-width bar.
                let pad = width.saturating_sub(line.width());
                if pad > 0 {
                    line.spans.push(Span::raw(" ".repeat(pad)));
                }
                line = line.patch_style(Style::default().bg(OPEN_SESSION_BG));
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
        SidebarRow::NewSession => with_hint(" + New session", Color::Green, "Ctrl+N", width),
        SidebarRow::Spacer => Line::from(""),
        SidebarRow::Search => {
            let state = &app.sidebar;
            let text = if state.filter_editing || !state.filter.is_empty() {
                let caret = if state.filter_editing { "_" } else { "" };
                format!(" / {}{caret}", state.filter)
            } else {
                " / Search".to_string()
            };
            let hint = if state.filter_editing || !state.filter.is_empty() {
                ""
            } else {
                "/"
            };
            with_hint(&text, Color::Gray, hint, width)
        }
        SidebarRow::Header {
            key,
            label,
            count,
            collapsed,
            remote,
        } => {
            // A quiet heading so the session titles carry the list: the name
            // in grey (yellow for `Pinned`), then a dimmer chevron (`v` open,
            // `>` collapsed), the hidden-session count when collapsed, and
            // the remote host kind: `ragent v [github]`.
            let arrow = if *collapsed { '>' } else { 'v' };
            let mut suffix = format!(" {arrow}");
            if *collapsed && *count > 0 {
                suffix.push_str(&format!(" {count}"));
            }
            if let Some(kind) = remote {
                suffix.push_str(&format!(" [{kind}]"));
            }
            let name_colour = if key == PINNED_KEY {
                Color::Yellow
            } else {
                Color::Gray
            };
            let name = fit(label, width);
            let suffix = fit(&suffix, width.saturating_sub(name.chars().count()));
            Line::from(vec![
                Span::styled(name, Style::default().fg(name_colour)),
                Span::styled(suffix, Style::default().fg(Color::DarkGray)),
            ])
        }
        SidebarRow::Session {
            id,
            title,
            updated_at,
            branch,
            owns_worktree,
            requests,
        } => {
            let attention = attention_of(app, id);
            let (marker, colour) = attention_marker(attention);
            // Branch marker and request count, e.g. `Y3`; the count is left
            // out when it is zero or not known yet.
            let branch_part = branch.as_ref().map(|_| match requests.filter(|n| *n > 0) {
                Some(n) => format!("{BRANCH_MARK}{n}"),
                None => BRANCH_MARK.to_string(),
            });
            // `#N`: the session has had N pull/merge requests; blue while
            // one is bound, grey when none is any more, and `#!N` when
            // Auto-fix waits on the user (FR-22).
            let total = app.sidebar.request_totals.get(id).copied().unwrap_or(0);
            let request_part = match (app.sidebar.bound.get(id), total) {
                (Some(true), n) => Some((format!("#!{}", n.max(1)), Color::Yellow)),
                (Some(false), n) => Some((format!("#{}", n.max(1)), Color::Blue)),
                (None, 0) => None,
                (None, n) => Some((format!("#{n}"), Color::DarkGray)),
            };
            // Right edge: `[age ][#][branch ]state`. Indent of two before the
            // title and one space after it.
            let branch_width = branch_part.as_ref().map_or(0, |b| b.len() + 1);
            let request_width = request_part.as_ref().map_or(0, |(p, _)| p.len() + 1);
            let fixed = 2 + 1 + branch_width + request_width + 1;
            let mut age = relative_age(updated_at, now);
            // When space is short the age goes first, then the title is cut.
            if width.saturating_sub(fixed + age.len() + 1) < MIN_TITLE_WIDTH {
                age.clear();
            }
            let age_width = if age.is_empty() { 0 } else { age.len() + 1 };
            let title_width = width.saturating_sub(fixed + age_width);
            let title = fit(title, title_width);
            let pad = title_width.saturating_sub(title.chars().count());
            let focused = app.session.session_id.as_deref() == Some(id.as_str());
            // Focused: cyan bold. Needing a look (unread, waiting): white
            // bold. Others: white, brighter than the headings above them.
            let title_style = if focused {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if matches!(attention, Attention::Unread | Attention::Waiting) {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            // A selected session is marked with `*` in place of the indent.
            let lead = if app.sidebar.selected.contains(id) {
                Span::styled("* ", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![
                lead,
                Span::styled(title, title_style),
                Span::raw(" ".repeat(pad + 1)),
            ];
            if !age.is_empty() {
                spans.push(Span::styled(age, Style::default().fg(Color::DarkGray)));
                spans.push(Span::raw(" "));
            }
            if let Some((part, colour)) = request_part {
                spans.push(Span::styled(part, Style::default().fg(colour)));
                spans.push(Span::raw(" "));
            }
            if let Some(part) = branch_part {
                // A session with its own worktree stands out from one that
                // only records a branch.
                let branch_colour = if *owns_worktree {
                    Color::Magenta
                } else {
                    Color::DarkGray
                };
                spans.push(Span::styled(part, Style::default().fg(branch_colour)));
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                marker.to_string(),
                Style::default().fg(colour),
            ));
            Line::from(spans)
        }
    }
}

/// Session ids in sidebar order (pins first), ignoring collapse and filter.
fn ordered_session_ids(app: &App) -> Vec<String> {
    sections(app)
        .into_iter()
        .flat_map(|section| section.sessions)
        .map(|(s, _)| s.id.clone())
        .collect()
}

/// Reports a failed session action on the status line.
pub(crate) fn report(app: &mut App, result: Result<(), String>) {
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
        Some(SidebarRow::Search) => start_filter(app),
        Some(SidebarRow::Spacer) => {}
        Some(SidebarRow::Header { key, .. }) => toggle_section(app, key, None),
        Some(SidebarRow::Session { id, .. }) => {
            let result = app.switch_to_session(id);
            report(app, result);
            app.sidebar.focused = false;
        }
        None => {}
    }
}

/// Drops session `id` on the row painted at screen row `row`: on the
/// `Pinned` heading it pins the session (an already pinned one is left as
/// is); anywhere else nothing happens.
fn drop_session(app: &mut App, id: &str, area: Rect, row: u16) {
    let top = area.y + 1;
    if row < top || row >= area.y + area.height.saturating_sub(1) {
        return;
    }
    let index = app.sidebar.scroll + usize::from(row - top);
    let on_pinned = matches!(
        build_rows(app).get(index),
        Some(SidebarRow::Header { key, .. }) if key == PINNED_KEY
    );
    if on_pinned && !app.sidebar.pinned.contains(id) {
        let result = app.toggle_session_pin(id);
        report(app, result);
    }
}

/// The nearest selectable row to `index`, moving `forward` (down) or back
/// past [`SidebarRow::Spacer`] rows, and the other way when that runs off
/// the end.
fn selectable(rows: &[SidebarRow], index: usize, forward: bool) -> usize {
    let is_spacer = |i: usize| matches!(rows.get(i), Some(SidebarRow::Spacer));
    if !is_spacer(index) {
        return index;
    }
    let ahead = if forward {
        (index + 1..rows.len()).find(|&i| !is_spacer(i))
    } else {
        (0..index).rev().find(|&i| !is_spacer(i))
    };
    let behind = || {
        if forward {
            (0..index).rev().find(|&i| !is_spacer(i))
        } else {
            (index + 1..rows.len()).find(|&i| !is_spacer(i))
        }
    };
    ahead.or_else(behind).unwrap_or(index)
}

/// Starts typing a new filter (the `/` key and the `/ Search` row).
fn start_filter(app: &mut App) {
    app.sidebar.filter_editing = true;
    app.sidebar.filter.clear();
}

/// The project whose heading is under the cursor, if the cursor is on a
/// project heading (not the pinned one).
fn cursor_project(app: &App) -> Option<ragent_storage::ProjectRow> {
    match build_rows(app).get(app.sidebar.cursor) {
        Some(SidebarRow::Header { key, .. }) => app
            .sidebar
            .groups
            .iter()
            .find(|g| g.project_id == *key)
            .map(|g| g.project.clone()),
        _ => None,
    }
}

/// The `/session new` command that starts a session in `project` on a new
/// branch with a random name ([`crate::branch_names`]). The project is
/// named by its name (quoted when it has spaces) when that name picks it
/// out alone, else by its id.
pub(crate) fn branch_session_command(app: &App, project: &ragent_storage::ProjectRow) -> String {
    // The stored name, which may be newer than the cached sidebar row.
    let (name, unique) = match app.storage.list_projects() {
        Ok(all) => {
            let name = all
                .iter()
                .find(|p| p.id == project.id)
                .map_or_else(|| project.name.clone(), |p| p.name.clone());
            let count = all
                .iter()
                .filter(|p| p.name.eq_ignore_ascii_case(&name))
                .count();
            (name, count == 1)
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not list projects");
            (project.name.clone(), false)
        }
    };
    // A name that cannot be written as one `--project` value falls back to
    // the id: shared with another project, empty, flag-like, or holding
    // both quote characters.
    let key = if !unique
        || name.is_empty()
        || name.starts_with("--")
        || (name.contains('"') && name.contains('\''))
    {
        project.id.clone()
    } else if name.contains('"') {
        format!("'{name}'")
    } else if name.contains(char::is_whitespace) || name.contains('\'') {
        format!("\"{name}\"")
    } else {
        name
    };
    format!(
        "/session new --project {key} --branch {}",
        crate::branch_names::random_branch_name()
    )
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

/// Focuses session `id` and prefills `/session rename <title>` (`F2`).
pub(crate) fn begin_rename(app: &mut App, id: &str) {
    let title = match app.storage.get_session(id) {
        Ok(row) => row.map(|r| r.title),
        Err(e) => {
            tracing::warn!(error = %e, "could not read session title");
            None
        }
    };
    let text = format!("/session rename {}", title.unwrap_or_default());
    begin_prefill(app, id, &text);
}

/// Focuses session `id`, puts `text` in the input with the cursor at its
/// end, and returns focus to the input. Nothing is prefilled when the
/// session cannot be focused.
pub(crate) fn begin_prefill(app: &mut App, id: &str, text: &str) {
    let result = app.switch_to_session(id);
    let ok = result.is_ok();
    report(app, result);
    if ok {
        app.session.input_cursor = text.chars().count();
        app.session.input = text.to_string();
        app.sidebar.focused = false;
    }
}

/// Opens the context menu for the row under the cursor: a session's, the
/// selection's (on a selected session) or a project heading's.
fn open_menu_at_cursor(app: &mut App) {
    let state = &app.sidebar;
    let line = state.cursor.saturating_sub(state.scroll);
    let row_y = state.area.y + 1 + u16::try_from(line).unwrap_or(u16::MAX);
    open_menu_for_row(app, app.sidebar.cursor, row_y);
}

/// Opens the context menu for row `index`, painted at screen row `row_y`.
/// Rows other than sessions and project headings have no menu.
fn open_menu_for_row(app: &mut App, index: usize, row_y: u16) {
    match build_rows(app).get(index) {
        Some(SidebarRow::Session { id, .. }) => {
            let id = id.clone();
            if app.sidebar.selected.len() > 1 && app.sidebar.selected.contains(&id) {
                crate::session_menu::open_selection(app, row_y);
            } else {
                crate::session_menu::open(app, &id, row_y);
            }
        }
        Some(SidebarRow::Header { key, .. }) if is_project_key(app, key) => {
            let key = key.clone();
            crate::session_menu::open_project(app, &key, row_y);
        }
        _ => {}
    }
}

/// Whether section `key` is a listed project (not `Pinned`, not a user
/// section).
fn is_project_key(app: &App, key: &str) -> bool {
    app.sidebar.groups.iter().any(|g| g.project_id == key)
}

/// Adds session `id` to the selection, or takes it out.
fn toggle_selected(app: &mut App, id: &str) {
    let selected = &mut app.sidebar.selected;
    if !selected.remove(id) {
        selected.insert(id.to_string());
    }
}

/// The sessions listed under project `project_id`'s heading (not pinned,
/// not in a user section), newest first.
pub(crate) fn project_session_ids(app: &App, project_id: &str) -> Vec<String> {
    sections(app)
        .into_iter()
        .find(|section| section.key == project_id)
        .map(|section| {
            section
                .sessions
                .into_iter()
                .map(|(s, _)| s.id.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Project ids in display order.
fn project_ids_in_order(app: &App) -> Vec<String> {
    sections(app)
        .into_iter()
        .filter(|section| is_project_key(app, &section.key))
        .map(|section| section.key)
        .collect()
}

/// Where project `project_id` can move: `(can move up, can move down)`.
pub(crate) fn project_moves(app: &App, project_id: &str) -> (bool, bool) {
    let order = project_ids_in_order(app);
    match order.iter().position(|id| id == project_id) {
        Some(at) => (at > 0, at + 1 < order.len()),
        None => (false, false),
    }
}

/// Moves project `project_id` one place up or down and saves the order.
/// The cursor follows the heading.
pub(crate) fn move_project(app: &mut App, project_id: &str, up: bool) {
    let mut order = project_ids_in_order(app);
    let Some(at) = order.iter().position(|id| id == project_id) else {
        return;
    };
    let to = if up { at.checked_sub(1) } else { Some(at + 1) };
    let Some(to) = to.filter(|&to| to < order.len()) else {
        return;
    };
    order.swap(at, to);
    app.sidebar.project_order = order;
    app.sidebar.save(&app.storage);
    if let Some(index) = build_rows(app)
        .iter()
        .position(|row| matches!(row, SidebarRow::Header { key, .. } if key == project_id))
    {
        app.sidebar.cursor = index;
    }
}

/// Every section key shown in the sidebar.
pub(crate) fn section_keys(app: &App) -> Vec<String> {
    sections(app)
        .into_iter()
        .map(|section| section.key)
        .collect()
}

/// Collapses every section (`true`) or expands every section (`false`).
pub(crate) fn set_all_collapsed(app: &mut App, collapse: bool) {
    if collapse {
        let keys = section_keys(app);
        app.sidebar.collapsed.extend(keys);
    } else {
        app.sidebar.collapsed.clear();
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
    if crate::session_menu::handle_key(app, key) {
        return true;
    }
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
    let rows = build_rows(app);
    let last = rows.len().saturating_sub(1);
    let state = &mut app.sidebar;
    match key.code {
        KeyCode::Up => state.cursor = selectable(&rows, state.cursor.saturating_sub(1), false),
        KeyCode::Down => state.cursor = selectable(&rows, (state.cursor + 1).min(last), true),
        KeyCode::PageUp => {
            state.cursor = selectable(&rows, state.cursor.saturating_sub(page), false);
        }
        KeyCode::PageDown => {
            state.cursor = selectable(&rows, (state.cursor + page).min(last), true);
        }
        KeyCode::Home => state.cursor = 0,
        KeyCode::End => state.cursor = selectable(&rows, last, false),
        KeyCode::Enter | KeyCode::Char(' ') => activate_row(app, app.sidebar.cursor),
        KeyCode::Left | KeyCode::Right => {
            if let Some(SidebarRow::Header { key: section, .. }) =
                build_rows(app).get(app.sidebar.cursor)
            {
                let section = section.clone();
                toggle_section(app, &section, Some(key.code == KeyCode::Left));
            }
        }
        KeyCode::Char('/') => start_filter(app),
        // On a project heading: `n` starts a session in that project, `b`
        // prefills the command that starts one on its own branch.
        KeyCode::Char('n') => {
            if let Some(project) = cursor_project(app) {
                let result = app.open_new_session_in_project(&project.into(), None);
                let ok = result.is_ok();
                report(app, result);
                if ok {
                    app.sidebar.focused = false;
                }
            }
        }
        KeyCode::Char('b') => {
            if let Some(project) = cursor_project(app) {
                let text = branch_session_command(app, &project);
                app.session.input_cursor = text.chars().count();
                app.session.input = text;
                app.sidebar.focused = false;
            }
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
                begin_rename(app, &id);
            }
        }
        KeyCode::Char('m') => open_menu_at_cursor(app),
        KeyCode::Char('v') => {
            if let Some(id) = cursor_session(app) {
                toggle_selected(app, &id);
            }
        }
        KeyCode::Char('u') => {
            if let Some(id) = cursor_session(app) {
                let result = app.toggle_session_unread(&id);
                report(app, result);
            }
        }
        KeyCode::Char('f') => {
            if let Some(id) = cursor_session(app) {
                let result = app.fork_session_by_id(&id);
                report(app, result);
                app.sidebar.focused = false;
            }
        }
        KeyCode::Char('D') => {
            if let Some(id) = cursor_session(app) {
                let result = app.request_delete_session(&id);
                report(app, result);
            }
        }
        KeyCode::F(10) if key.modifiers.contains(KeyModifiers::SHIFT) => {
            open_menu_at_cursor(app);
        }
        KeyCode::Esc if !state.selected.is_empty() => state.selected.clear(),
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
    if crate::session_menu::handle_mouse(app, event) {
        return true;
    }
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
        match event.kind {
            MouseEventKind::Down(_) => app.sidebar.focused = false,
            // A drag that ends outside the sidebar drops nothing.
            MouseEventKind::Up(MouseButton::Left) => app.sidebar.drag_session = None,
            _ => {}
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
                let rows = build_rows(app);
                // `Ctrl`+click adds the session to the selection or takes it
                // out; a plain click clears the selection.
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(SidebarRow::Session { id, .. }) = rows.get(index) {
                        let id = id.clone();
                        app.sidebar.cursor = index;
                        toggle_selected(app, &id);
                    }
                    return true;
                }
                app.sidebar.selected.clear();
                if let Some(SidebarRow::Session { id, .. }) = rows.get(index) {
                    app.sidebar.drag_session = Some(id.clone());
                }
                if index < rows.len() && !matches!(rows.get(index), Some(SidebarRow::Spacer)) {
                    app.sidebar.cursor = index;
                    activate_row(app, index);
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if let Some(id) = app.sidebar.drag_session.take() {
                drop_session(app, &id, area, event.row);
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            app.sidebar.focused = true;
            let top = area.y + 1;
            if event.row >= top && event.row < area.y + area.height - 1 {
                let index = app.sidebar.scroll + usize::from(event.row - top);
                if matches!(
                    build_rows(app).get(index),
                    Some(SidebarRow::Session { .. } | SidebarRow::Header { .. })
                ) {
                    app.sidebar.cursor = index;
                    open_menu_for_row(app, index, event.row);
                }
            }
        }
        MouseEventKind::ScrollUp => {
            let rows = build_rows(app);
            app.sidebar.scroll = app.sidebar.scroll.saturating_sub(1);
            app.sidebar.cursor = selectable(&rows, app.sidebar.cursor.saturating_sub(1), false);
        }
        MouseEventKind::ScrollDown => {
            let rows = build_rows(app);
            let last = rows.len().saturating_sub(1);
            app.sidebar.cursor = selectable(&rows, (app.sidebar.cursor + 1).min(last), true);
        }
        _ => {}
    }
    true
}
