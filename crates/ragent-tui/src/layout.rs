//! TUI layout and rendering.
//!
//! Builds the main layout with a 2-line status bar at the top, messages in the
//! middle, and an input area at the bottom.
//!
//! The status bar is organized into 2 lines for better readability:
//! - Line 1: Session, agent, working directory, git branch, and status message
//! - Line 2: Provider, token usage, active tasks, code index, and log indicator

use pulldown_cmark::{Event as MdEvent, Options, Parser, Tag, TagEnd};
use std::path::Path;

use crate::app::{image_dimensions_or_placeholder, sanitize_for_display, short_id};

use crate::widgets::message_widget::make_relative_path;
use ragent_types::ThinkingLevel;
use ragent_types::sanitize_terminal::sanitize_terminal;
use ragent_types::strutil::{truncate_bytes, truncate_bytes_no_ellipsis};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use crate::layout_active_agents::render_active_agents_subpanel;

use crate::theme;
use crate::utils::{
    ResponsiveBreakpoint, centered_rect, centered_rect_fixed, centered_rect_max,
    is_below_minimum_size, shorten_middle,
};
use ragent_agent::team::MemberStatus;

use ragent_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use ragent_storage::storage::MemoryRow;

use crate::app::{
    App, ContextAction, LogLevel, ModelPickerEntry, OutputViewTarget, PROVIDER_LIST,
    ProviderSetupStep, SelectionPane, StorePane,
};
use crate::widgets::message_widget::{
    INDENT_SPACES, canonical_tool_name, capitalize_tool_name, is_agent_notice, read_line_range,
    render_agent_notice_lines, scrub_narrated_toolcalls, tool_inline_diff, tool_input_summary,
    tool_result_summary,
};
use ragent_connectors::CataloguePane;

/// Padding applied to each side of a content-sized table column.
const COLUMN_PADDING_CHARS: usize = 1;

/// Column header labels shared by the model-picker dialogs.
const MODEL_PICKER_HEADERS: [&str; 5] = ["Model", "Context", "Cost", "Thinking", "Features"];

/// Default spacing between table columns (matches `Table::column_spacing`).
const MODEL_PICKER_COLUMN_SPACING: usize = 1;

/// The one-column lane at the left of every plugin-store result row that marks
/// an installed entry (spec `pluginstores` FR-005).
///
/// The marker sits between the cursor gutter and the id column, so it never
/// competes with the description for horizontal room. Rows whose plugin is not
/// installed paint a single blank in the lane, keeping every column aligned.
const STORE_INSTALLED_MARKER: &str = "*";

/// Fixed terminal columns reserved for a plugin-store row's plugin-id column
/// (spec `pluginstores` FR-004). Wider than the original 22 so the id column
/// keeps a comfortable width now the panel is a third wider.
const PLUGIN_STORE_ID_MAX: usize = 24;

/// Fixed terminal columns reserved for a plugin-store row's version column
/// (spec `pluginstores` FR-004).
const PLUGIN_STORE_VERSION_MAX: usize = 8;

/// Ceiling on the terminal columns reserved for a plugin-store row's marketplace
/// column before it is truncated (spec `pluginstores` FR-048). Half again the
/// previous 14 so common marketplace labels render in full; the description still
/// keeps its share of the flexible remainder.
const PLUGIN_STORE_MARKETPLACE_MAX: usize = 21;

/// The pinned column-heading labels of the plugin-store result list (FR-004).
/// Each label renders directly above its column; a label longer than its column
/// is clipped by the row template, so the headings never push a value off the
/// row. The two leading spaces mirror the `> ` row prefix so the labels align
/// with the values beneath.
const PLUGIN_STORE_HEADING_ID: &str = "Id";
const PLUGIN_STORE_HEADING_VERSION: &str = "Version";
const PLUGIN_STORE_HEADING_MARKETPLACE: &str = "Marketplace";
const PLUGIN_STORE_HEADING_DESCRIPTION: &str = "Description";

/// Narrowest terminal columns the plugin-store category navigator is drawn in
/// (spec `catnav` FR-020).
const STORE_NAV_MIN: u16 = 12;

/// Widest terminal columns the plugin-store category navigator is drawn in
/// (spec `catnav` FR-020).
const STORE_NAV_MAX: u16 = 26;

/// Terminal columns always reserved for the plugin-store result column, so the
/// navigator can never squeeze the result list below a readable width
/// (spec `catnav` FR-020).
const STORE_RESULT_MIN: u16 = 27;

/// Blank separator columns between the plugin-store category navigator and the
/// result column (spec `catnav` FR-001).
const STORE_NAV_GAP: u16 = 1;

/// The title painted on the row directly above the plugin-store category
/// navigator's `ALL` entry, naming the navigation type (spec `pluginstores`
/// FR-004). Rendered in reversed video (black on white).
const STORE_CATEGORY_TITLE: &str = "Categories";

/// The title painted on the row directly above the plugin-store marketplace
/// navigator's `ALL` entry, naming the navigation type (spec `pluginstores`
/// FR-004). Rendered in reversed video (black on white).
const STORE_MARKETPLACE_TITLE: &str = "Marketplace";

/// The label painted at the left edge of the plugin-store bottom search box
/// (spec `pluginstores` FR-004).
const STORE_SEARCH_LABEL: &str = " search: ";

/// The clear button painted at the right edge of the plugin-store bottom search
/// box (spec `pluginstores` FR-004). A left-click inside the button clears the
/// live search filter; the text is also the hit affordance, so click-to-clear is
/// discoverable.
const STORE_CLEAR_LABEL: &str = "[x] clear";

/// Narrowest terminal columns the plugin-store search box's editable field is
/// given, so the label and the clear button never squeeze it below a usable
/// width on a small terminal (spec `pluginstores` FR-004).
const STORE_SEARCH_MIN: u16 = 6;

/// Largest number of terminal rows the plugin-store browser overlay occupies
/// before it stops growing and stays centred on a tall terminal (spec
/// `pluginstores` FR-018). Raised from the previous 24-row ceiling so the panel
/// shows roughly 70% more result rows on a full-height screen.
const PLUGIN_STORE_MAX_HEIGHT: u16 = 41;

/// The plugin-store accent colour: a slate blue used for the panel border, the
/// store badge in the title, the marketplace column, and the search-box chrome.
///
/// Deliberately calm: the panel is a reading surface, so the chrome recedes and
/// the cursor colours (cyan for a navigator, magenta for a result row) stay the
/// brightest thing on screen (spec `pluginstores` FR-004, FR-005).
const STORE_ACCENT: Color = Color::Rgb(112, 148, 204);

/// The background painted across the plugin-store chrome bands (the footer hint
/// row and the search box), so the panel's operating furniture reads as a
/// distinct strip beneath the result table (spec `pluginstores` FR-004).
const STORE_CHROME_BG: Color = Color::Rgb(30, 32, 44);

/// A near-black stripe painted on alternating plugin-store result rows, so the
/// eye can follow one row across the id, version, marketplace and description
/// columns on a wide panel.
const STORE_BAND_BG: Color = Color::Rgb(22, 22, 30);

/// Fixed terminal columns reserved for a connector-catalogue row's connector-id
/// column (spec `connectors` FR-045).
///
/// Matches [`PLUGIN_STORE_ID_MAX`] so the two catalogue browsers lay their rows out
/// identically on the same terminal.
const CONNECTOR_CATALOGUE_ID_MAX: usize = 24;

/// Fixed terminal columns reserved for a connector-catalogue row's category column
/// (spec `connectors` FR-045): wide enough for the common category names, and the
/// description takes every remaining column.
const CONNECTOR_CATALOGUE_CATEGORY_MAX: u16 = 18;

/// Ceiling on the terminal columns reserved for a connector-catalogue row's
/// marketplace column before it is truncated (spec `connectors` FR-043, FR-045).
///
/// Mirrors [`PLUGIN_STORE_MARKETPLACE_MAX`]: wide enough for common marketplace
/// labels, and any space the description does not use moves to the marketplace.
const CONNECTOR_CATALOGUE_MARKETPLACE_MAX: u16 = 21;

/// The pinned column-heading labels of the connector-catalogue result list
/// (FR-045).
///
/// Each label renders directly above its column; a label longer than its column is
/// clipped by the row template, so the headings never push a value off the row. The
/// three leading spaces mirror the `> ` cursor gutter plus the one-column installed
/// lane, so the labels align with the values beneath (mirrors
/// [`PLUGIN_STORE_HEADING_ID`]).
const CONNECTOR_CATALOGUE_HEADING_ID: &str = "Id";
const CONNECTOR_CATALOGUE_HEADING_CATEGORY: &str = "Category";
const CONNECTOR_CATALOGUE_HEADING_MARKETPLACE: &str = "Marketplace";
const CONNECTOR_CATALOGUE_HEADING_DESCRIPTION: &str = "Description";

/// The accent colour of the provider/model setup dialogs (`/provider`, `/model`,
/// `/provider router`).
///
/// Shares [`STORE_ACCENT`] so every list-bearing overlay in the TUI wears one calm
/// slate chrome: the frame, the cursor bar, and the secondary facts in the heading
/// band. The cursor bar stays the brightest thing on the dialog.
const SETUP_ACCENT: Color = STORE_ACCENT;

/// Foreground of the reversed-video heading band pinned above a setup dialog's
/// list (the same treatment the plugin and connector catalogues give their column
/// headings, so all three overlays head their tables alike).
const SETUP_BAND_FG: Color = Color::Black;

/// Background of that heading band.
const SETUP_BAND_BG: Color = Color::Gray;

/// The near-black stripe painted on alternating setup-dialog rows, so the eye can
/// follow one entry across the name, description and status lanes.
const SETUP_ZEBRA_BG: Color = STORE_BAND_BG;

/// Terminal columns reserved at the right of a provider-setup row for the
/// `[local]` badge lane, so the badges of adjacent rows line up.
const PROVIDER_BADGE_WIDTH: usize = 7;

/// Terminal columns reserved for a provider-setup row's provider-id column
/// (`generic_openai`, `azure_resource` and friends all fit).
const PROVIDER_ID_MAX: usize = 14;

/// Terminal columns reserved at the right of a provider-setup row for the
/// credential-status lane (`[ok]` when a key is stored, `*` for the active
/// provider in the reset dialog).
const PROVIDER_STATUS_WIDTH: usize = 5;

/// PERF-042: minimum interval between re-renders of a message whose cache
/// group is already populated while it is being streamed.
///
/// `Message::edit_seq` is bumped on every streamed token, so re-parsing and
/// re-wrapping the growing reply on every token is O(n^2) over the reply.  A
/// group that has already been rendered is refreshed at most once per this
/// interval; the dirty watermark keeps it pending, and the main loop schedules
/// a wake at the end of the window so the tail is never left stale.
pub const MESSAGE_STREAM_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);

/// Format a model-picker entry into its five display-cell strings.
///
/// The first cell carries the selection indicator prefix (a filled-triangle
/// glyph plus space when `selected`, two spaces otherwise) so column
/// measurement sees exactly what the table will render.
fn model_picker_entry_cells(entry: &ModelPickerEntry, selected: bool) -> Vec<String> {
    // Format context window.
    let ctx_str = if entry.context_window >= 1_000_000 {
        format!("{}M", entry.context_window / 1_000_000)
    } else if entry.context_window >= 1_000 {
        format!("{}K", entry.context_window / 1_000)
    } else {
        entry.context_window.to_string()
    };

    // Format cost: display tier (Free, Low, Medium, etc.) and multiplier.
    let cost_str = if entry.cost_multiplier.is_empty() {
        entry.cost_tier.clone()
    } else {
        format!("{} {}", entry.cost_tier, entry.cost_multiplier)
    };
    let thinking_str = App::format_thinking_levels(&entry.thinking_levels);

    // Format features.
    let mut features = Vec::new();
    if entry.reasoning {
        features.push("R");
    }
    if entry.vision {
        features.push("V");
    }
    if entry.tool_use {
        features.push("T");
    }
    let features_str = if features.is_empty() {
        "-".to_string()
    } else {
        features.join(",")
    };

    // Selection indicator lives inside the name cell.
    let model_name = if selected {
        format!("> {}", entry.name)
    } else {
        format!("  {}", entry.name)
    };

    vec![model_name, ctx_str, cost_str, thinking_str, features_str]
}

/// Inner (table-area) width in characters needed to render every model-picker
/// cell without truncation.
///
/// This mirrors the measurement in [`content_sized_columns`]: widest cell or
/// header per column, plus padding on each side, plus one column of spacing
/// between adjacent columns.
fn model_picker_inner_width(cells: &[Vec<String>]) -> usize {
    let col_count = MODEL_PICKER_HEADERS.len();
    let mut total = 0;
    for col in 0..col_count {
        let widest = cells
            .iter()
            .filter_map(|row| row.get(col))
            .map(|cell| cell.chars().count())
            .chain(std::iter::once(MODEL_PICKER_HEADERS[col].chars().count()))
            .max()
            .unwrap_or(0)
            + COLUMN_PADDING_CHARS * 2;
        total += widest;
    }
    // Spacing between adjacent columns.
    total + MODEL_PICKER_COLUMN_SPACING * col_count.saturating_sub(1)
}

/// Build the shared cell grid for a model-picker dialog.
fn model_picker_cells(models: &[ModelPickerEntry], selected: usize) -> Vec<Vec<String>> {
    models
        .iter()
        .enumerate()
        .map(|(i, entry)| model_picker_entry_cells(entry, i == selected))
        .collect()
}

/// Build table column constraints sized to hold the widest content.
///
/// Each column is measured against its header label and every row's cell
/// string, then widened by [`COLUMN_PADDING_CHARS`] spaces on each side. The
/// first column uses `Min` so it absorbs any surplus dialog width (giving
/// model names room to grow); the remaining columns are exact `Length`s so
/// they never truncate their widest cell.
fn content_sized_columns(cells: &[Vec<String>]) -> Vec<Constraint> {
    MODEL_PICKER_HEADERS
        .iter()
        .enumerate()
        .map(|(col, label)| {
            let content_width = cells
                .iter()
                .filter_map(|row| row.get(col))
                .map(|cell| cell.chars().count())
                .chain(std::iter::once(label.chars().count()))
                .max()
                .unwrap_or(0)
                + COLUMN_PADDING_CHARS * 2;
            if col == 0 {
                Constraint::Min(content_width as u16)
            } else {
                Constraint::Length(content_width as u16)
            }
        })
        .collect()
}

/// The pinned column-heading band of a model-picker dialog.
///
/// Each label is padded to the width measured for its column (see
/// [`content_sized_columns`]) and joined with the same [`MODEL_PICKER_COLUMN_SPACING`]
/// the rows use, so every label sits directly over the values beneath it. The two
/// leading spaces mirror the `> ` cursor gutter carried inside the model-name cell,
/// so `Model` starts over the name rather than over the cursor.
fn model_picker_heading_band(widths: &[usize]) -> String {
    let mut band = String::from("  ");
    for (col, label) in MODEL_PICKER_HEADERS.iter().enumerate() {
        let width = widths
            .get(col)
            .copied()
            .unwrap_or(label.chars().count() + 2);
        // The first column already carries the gutter inside its measured width.
        let padded = match col {
            0 => format!("{label:<pad$}", pad = width.saturating_sub(2)),
            // Context and cost are numeric columns: their heading right-aligns over
            // the right-aligned digits beneath it.
            1 | 2 => format!("{label:>width$}"),
            _ => format!("{label:<width$}"),
        };
        band.push_str(&padded);
        if col + 1 < MODEL_PICKER_HEADERS.len() {
            band.push_str(&" ".repeat(MODEL_PICKER_COLUMN_SPACING));
        }
    }
    band
}

/// Render the full TUI chat screen.
///
/// # Examples
///
/// ```rust,no_run
/// # use ratatui::Frame;
/// # use ragent_tui::App;
/// # use ragent_tui::layout::render;
/// # fn example(frame: &mut Frame, app: &mut App) {
/// render(frame, app);
/// # }
/// ```
pub fn render(frame: &mut Frame, app: &mut App) {
    render_chat(frame, app);
    // Config-save picker overlay - rendered on top of chat, before history
    // picker so that whichever picker is open gets drawn last (i.e. on top).
    if app.config_save_picker.is_some() {
        render_config_save_picker(frame, app);
    }
    // History picker overlay - rendered on top of everything.
    if app.history_picker.is_some() {
        render_history_picker(frame, app);
    }
    // Run-cost summary banner (FR-012): a transient one-line overlay rendered
    // last so it sits above all other UI. Dismissed on the next keypress.
    if app.run_cost_banner.is_some() {
        render_run_cost_banner(frame, app);
    }
    // Queue-control menu overlay (ALT-Q) - a modal drawn above everything so the
    // actions stay reachable while the queue drains underneath (FR-021). The
    // queue-entry panel (the menu's `Show` row) is drawn above it when open.
    if app.queue_menu_open {
        render_queue_menu(frame, app);
    } else {
        app.queue_menu_area = Rect::default();
    }
    if app.queue_show_open {
        render_queue_show_panel(frame, app);
    } else {
        app.queue_show_area = Rect::default();
    }
    // `Clear the input queue?` confirmation dialog (ALT-Q `Clear` row) - a modal
    // drawn above the menu/chat that reuses the shared overlay machinery
    // (spec `inputqueue` FR-033, NFR-010).
    if app.queue_clear_confirm_open {
        render_queue_clear_confirm(frame, app);
    } else {
        app.queue_clear_confirm_area = Rect::default();
    }
    // `Clear this project's memory?` confirmation dialog (`/memory clear`) - a
    // modal drawn above the chat that reuses the shared overlay machinery.
    if app.memory_clear_confirm_open {
        render_memory_clear_confirm(frame, app);
    }
    // Destructive row-action confirmation dialog (spec `agentui` T-009) - a
    // modal drawn above the chat (and therefore above the Agents and Teams
    // windows, which render inside the chat pass) so the answer applies to the
    // topmost surface (FR-036, FR-037, FR-040).
    if app.row_action_confirm.is_some() {
        render_row_action_confirm(frame, app);
    } else {
        app.row_action_confirm_area = Rect::default();
        app.row_action_confirm_yes_area = Rect::default();
        app.row_action_confirm_no_area = Rect::default();
    }
    // Plugin-store browse panel (spec `pluginstores` T-006) - a full-screen-ish
    // modal drawn last so it sits above every other overlay while it owns the
    // keyboard (FR-004, FR-015).
    if app.plugin_store.is_some() {
        render_plugin_store_panel(frame, app);
    } else {
        app.plugin_store_area = Rect::default();
        app.plugin_store_nav_area = Rect::default();
        app.plugin_store_marketplace_area = Rect::default();
        app.plugin_store_result_area = Rect::default();
        app.plugin_store_search_area = Rect::default();
        app.plugin_store_clear_area = Rect::default();
    }
    // Connector-catalogue browse panel (`/connectors claude`) - drawn last so it
    // sits above every other overlay while it owns the keyboard.
    if app.connector_store.is_some() {
        render_connector_catalogue_panel(frame, app);
    } else {
        app.connector_store_area = Rect::default();
        app.connector_store_nav_area = Rect::default();
        app.connector_store_marketplace_area = Rect::default();
        app.connector_store_result_area = Rect::default();
        app.connector_store_search_area = Rect::default();
        app.connector_store_clear_area = Rect::default();
    }
    // `/backend` switcher panel (spec `openhands` FR-008) - drawn last so it sits
    // above every other overlay while it owns the keyboard.
    if app.backend_panel.is_some() {
        render_backend_panel(frame, app);
    } else {
        app.backend_panel_area = Rect::default();
    }
}

/// Render the `/backend` execution-backend switcher panel (spec `openhands`
/// T-005; FR-008).
///
/// A bordered, titled modal listing every registered backend with its id, kind,
/// health, and secret-free connection summary. The active backend carries an
/// `[active]` marker and the highlighted row carries the block cursor; an
/// `unavailable` row is dimmed because it cannot be selected. The panel area is
/// recomputed from `Frame::area` every frame and stored back in
/// [`App::backend_panel_area`] for tests.
fn render_backend_panel(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let Some(panel) = app.backend_panel.as_ref() else {
        return;
    };
    let active_kind = app.active_backend.clone();

    let rows = &panel.rows;
    let id_w = rows
        .iter()
        .map(|r| r.id.len())
        .max()
        .unwrap_or(4)
        .clamp(4, 16);
    let kind_w = rows
        .iter()
        .map(|r| r.kind.len())
        .max()
        .unwrap_or(4)
        .clamp(4, 8);
    let health_w = rows
        .iter()
        .map(|r| r.health.len())
        .max()
        .unwrap_or(7)
        .clamp(7, 12);
    let widest = rows
        .iter()
        .map(|row| {
            id_w + kind_w
                + health_w
                + row.connection.len()
                + row.detail.as_deref().map_or(0, str::len)
                + 24
        })
        .max()
        .unwrap_or(0);
    let screen = frame.area();
    let title = format!(" Execution backend - active: {active_kind} ");
    let width = (widest as u16)
        .saturating_add(4)
        .max(title.chars().count() as u16 + 2)
        .clamp(30, 120)
        .min(screen.width);
    let height = (rows.len() as u16)
        .saturating_add(4)
        .clamp(5, 16)
        .min(screen.height);
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);
    app.backend_panel_area = area;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let list_area = chunks[0];
    let footer_area = chunks[1];

    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let active_style = Style::default().fg(Color::Green);
    let normal_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let marker = if row.active { " [active]" } else { "" };
            let detail = row
                .detail
                .as_deref()
                .map(|d| format!(" - {d}"))
                .unwrap_or_default();
            let style = if !row.health_ok {
                dim_style
            } else if row.active {
                active_style
            } else {
                normal_style
            };
            let summary = format!(
                "{}{marker}{detail}",
                truncate_bytes_no_ellipsis(&row.connection, 40),
            );
            let line = format!(
                "> {:<id_w$} {:<kind_w$} {:<health_w$} {summary}",
                truncate_bytes_no_ellipsis(&row.id, id_w),
                truncate_bytes_no_ellipsis(row.kind, kind_w),
                truncate_bytes_no_ellipsis(row.health, health_w),
            );
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let selected = panel.cursor.min(rows.len().saturating_sub(1));
    let list = List::new(items).highlight_style(cursor_style);
    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, list_area, &mut state);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Up/Down move  Enter switch  Esc close",
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center),
        footer_area,
    );
}

/// Render the connector-catalogue browse panel (`/connectors claude`).
///
/// A bordered, titled modal overlay carrying the same design language as the
/// plugin-store panel ([`render_plugin_store_panel`]), so the two catalogue
/// browsers read as one family: a slate accent frame, a structured title line,
/// a left column split into the category navigator (top) and the marketplace
/// navigator (bottom) - each under a reversed-video title band - a result column
/// with a pinned dark-on-light column-heading row above a zebra-banded list, a
/// centred footer of key hints, and a bottom-edge search box with a click-to-clear
/// button.
///
/// The highlighted result row carries the block cursor (a full-row background
/// applied through [`ratatui::widgets::List::highlight_style`]). A row whose
/// connector id is in the derived installed set paints a `*` in the one-column
/// installed lane between the cursor gutter and the id column, in the installed
/// colour, independent of the cursor.
///
/// The body renders an explicit state line rather than a blank list when there is
/// nothing to show: a loading row while the fetch is in flight, an inline error
/// row naming the cause when it failed, and a `no matching connectors` line when
/// the catalogue, the query, or a filter yields no results.
///
/// The panel area is recomputed from `Frame::area` every frame, so a terminal
/// resize re-derives a centred, fully visible modal; every hit-test area is stored
/// back on [`App`] for the mouse router.
fn render_connector_catalogue_panel(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    if app.connector_store.is_none() {
        return;
    }

    let screen = frame.area();
    // Size the panel to the screen with a margin, so it stays centred and fully
    // visible as the terminal changes size. The 140-column ceiling and the row
    // budget match the plugin-store panel exactly, so the two browsers are the
    // same shape on the same terminal.
    let width = screen
        .width
        .saturating_sub(4)
        .clamp(30, 140)
        .min(screen.width);
    let height = screen
        .height
        .saturating_sub(2)
        .clamp(8, PLUGIN_STORE_MAX_HEIGHT)
        .min(screen.height);
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);
    app.connector_store_area = area;

    // The title names the catalogue, the search field, the counts, and the active
    // filters; it needs only an immutable read of the browser.
    let title = match app.connector_store.as_ref() {
        Some(browser) => connector_catalogue_title(browser),
        // The early return above guarantees `Some`, but read defensively so no
        // path can panic if that guard ever drifts.
        None => Line::default(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(STORE_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Reserve the last inner row for the footer key hints and the row above it for
    // the bottom search box; the body takes what remains.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];
    let search_area = chunks[2];
    app.connector_store_search_area = search_area;

    // Split the body into the left navigator column and the result column. The
    // navigator width is recomputed from the current frame every draw, so a
    // terminal resize re-derives it and the column stays fully visible.
    let nav_width = store_nav_width(body_area.width);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(nav_width),
            Constraint::Length(STORE_NAV_GAP),
            Constraint::Min(0),
        ])
        .split(body_area);
    let nav_area = columns[0];
    let result_area = columns[2];
    // The left navigator column is split vertically into the category navigator
    // (top half) and the marketplace navigator (bottom half); each pane owns a full
    // viewport so both stay usable side by side in one column.
    let nav_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(nav_area);
    let category_area = nav_panes[0];
    let marketplace_area = nav_panes[1];

    // Each navigator carries a one-row title above its `ALL` entry naming the
    // navigation type, painted in reversed video. Reserve that row out of each pane
    // so the rows beneath still scroll inside what remains; the cached hit-test
    // areas are the row areas below the title, so a click or wheel on a title row is
    // inert and never mis-mapped onto the `ALL` row.
    let category_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(category_area);
    let category_title_area = category_chunks[0];
    let category_rows_area = category_chunks[1];
    let marketplace_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(marketplace_area);
    let marketplace_title_area = marketplace_chunks[0];
    let marketplace_rows_area = marketplace_chunks[1];
    app.connector_store_nav_area = category_rows_area;
    app.connector_store_marketplace_area = marketplace_rows_area;

    // The result column splits vertically into a fixed column-heading row (always
    // painted, never scrolled) and the scrolling result list beneath it. The
    // headings are drawn outside the `List`, so the list's own scroll keeps rows
    // under the cursor visible while the headings stay pinned.
    let result_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(result_area);
    let heading_area = result_chunks[0];
    let list_area = result_chunks[1];
    // Mouse hit-testing and the cursor-to-row mapping both reason about result
    // rows; anchor the cached area on the list (below the heading) so a click or
    // wheel on the heading row is inert.
    app.connector_store_result_area = list_area;

    // Row text colours. The id and description carry the reading colour; the
    // category and marketplace columns step back - the category to a muted tone,
    // the marketplace to the panel accent - so a horizontally busy row still leads
    // the eye with its name and its purpose.
    let normal_style = Style::default().fg(Color::White);
    let active_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let installed_style = Style::default().fg(Color::Green);
    let muted_style = store_muted_style();
    let tag_style = Style::default().fg(STORE_ACCENT);
    let dim_style = Style::default().fg(theme::colors::HINT);

    let Some(browser) = app.connector_store.as_mut() else {
        return;
    };
    // Focus distinction: the pane that holds keyboard focus paints its cursor in
    // its filled colour; the other panes' cursors are dimmed, so which pane owns
    // `Up`/`Down` is visible at a glance. The two navigators share one focused
    // colour and the result cursor keeps its own colour, independent of them.
    let focused_result_cursor = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let focused_nav_cursor = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let unfocused_cursor = Style::default().fg(Color::White).bg(Color::DarkGray);
    let nav_cursor_style = if browser.focus == CataloguePane::Categories {
        focused_nav_cursor
    } else {
        unfocused_cursor
    };
    let marketplace_cursor_style = if browser.focus == CataloguePane::Marketplaces {
        focused_nav_cursor
    } else {
        unfocused_cursor
    };
    let cursor_style = if browser.focus == CataloguePane::Results {
        focused_result_cursor
    } else {
        unfocused_cursor
    };
    // The header band above each navigator's `ALL` entry names the navigation type.
    // It is painted reversed video - black on white - so it reads as a distinct
    // header, and is padded to the column width so the band runs edge to edge
    // instead of stopping dead after the label.
    let title_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let nav_band = |label: &'static str, width: u16| {
        let text = format!(" {label}");
        let pad = (width as usize).saturating_sub(text.chars().count());
        Line::from(Span::styled(
            format!("{text}{}", " ".repeat(pad)),
            title_style,
        ))
    };
    frame.render_widget(
        Paragraph::new(nav_band(STORE_CATEGORY_TITLE, category_title_area.width)),
        category_title_area,
    );
    frame.render_widget(
        Paragraph::new(nav_band(
            STORE_MARKETPLACE_TITLE,
            marketplace_title_area.width,
        )),
        marketplace_title_area,
    );
    // The category navigator: `ALL`, the `Installed` pseudo-category, then the
    // derived categories, each with its entry count.
    render_connector_catalogue_navigator(
        frame,
        browser,
        category_rows_area,
        nav_cursor_style,
        active_style,
        normal_style,
    );
    // The marketplace navigator: `ALL` first, then the derived marketplaces.
    render_connector_marketplace_navigator(
        frame,
        browser,
        marketplace_rows_area,
        marketplace_cursor_style,
        active_style,
        normal_style,
    );

    // The body: result rows, or one explicit state line. The navigators stay
    // visible above either case. The id and category columns are fixed-width; the
    // marketplace keeps its ceiling and the description takes every remaining
    // column, so as the row's last column it renders straight up to the panel's
    // inner right edge and a longer description is clipped there by the row area.
    let id_width = CONNECTOR_CATALOGUE_ID_MAX;
    let category_width = CONNECTOR_CATALOGUE_CATEGORY_MAX as usize;
    let (marketplace_width, desc_width) =
        connector_flex_widths(result_area.width as usize, id_width, category_width);

    match connector_catalogue_body(browser) {
        StoreBody::Rows => {
            let visible = browser.filtered.len();
            // The pinned column-heading row: the same column template as a result
            // row so each label sits exactly above its column. The dark-on-light
            // style distinguishes it from both the normal and installed row colours,
            // and the band is padded to the full result width so it reads as a header
            // strip rather than a label floating over the rows.
            let heading_style = Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD);
            let heading_text = format!(
                "   {CONNECTOR_CATALOGUE_HEADING_ID:<id_width$} {CONNECTOR_CATALOGUE_HEADING_CATEGORY:<category_width$} {CONNECTOR_CATALOGUE_HEADING_MARKETPLACE:<marketplace_width$} {CONNECTOR_CATALOGUE_HEADING_DESCRIPTION:<desc_width$}",
            );
            let heading_pad =
                (result_area.width as usize).saturating_sub(heading_text.chars().count());
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("{heading_text}{}", " ".repeat(heading_pad)),
                    heading_style,
                ))),
                heading_area,
            );
            // The block cursor's position, clamped into the filtered set; the list's
            // own scroll keeps it visible beneath the pinned heading row. It is
            // resolved before the rows are built so each row can paint the cursor
            // gutter marker itself.
            let selected = browser.cursor.min(visible.saturating_sub(1));
            let items: Vec<ListItem> = browser
                .filtered
                .iter()
                .enumerate()
                .map(|(row_index, &index)| {
                    let entry = &browser.all[index];
                    let installed = browser.is_installed(entry.id.as_str());
                    let base_style = if installed {
                        installed_style
                    } else {
                        normal_style
                    };
                    let id_field = truncate_bytes_no_ellipsis(entry.id.as_str(), id_width);
                    let category_field =
                        truncate_bytes_no_ellipsis(&entry.category, category_width);
                    let marketplace_field = if entry.marketplace.trim().is_empty() {
                        "-".to_string()
                    } else {
                        truncate_bytes_no_ellipsis(&entry.marketplace, marketplace_width)
                    };
                    let desc_field = truncate_bytes_no_ellipsis(&entry.description, desc_width);
                    // The two-column gutter mirrors the heading's leading indent and
                    // carries a `>` only on the row under the cursor, so the cursor is
                    // visible even where a background-only highlight washes out. It is
                    // followed by the one-column installed lane, which every row paints
                    // - the marker or a blank - so the columns stay aligned whether or
                    // not the row is installed.
                    let gutter = if row_index == selected { "> " } else { "  " };
                    let installed_span = if installed {
                        Span::styled(
                            STORE_INSTALLED_MARKER.to_string(),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(" ".to_string(), base_style)
                    };
                    let mut spans = vec![
                        Span::styled(gutter.to_string(), base_style),
                        installed_span,
                        Span::styled(format!("{id_field:<id_width$} "), base_style),
                        Span::styled(format!("{category_field:<category_width$} "), muted_style),
                        Span::styled(
                            format!("{marketplace_field:<marketplace_width$} "),
                            tag_style,
                        ),
                        Span::styled(desc_field, base_style),
                    ];
                    // Pad the row to the full result width: the trailing run carries
                    // the row's own style, so a zebra band (and the cursor background
                    // the list paints over it) reaches the column's right edge instead
                    // of stopping after the last glyph.
                    let painted: usize =
                        spans.iter().map(|span| span.content.chars().count()).sum();
                    let pad = (result_area.width as usize).saturating_sub(painted);
                    if pad > 0 {
                        spans.push(Span::styled(" ".repeat(pad), base_style));
                    }
                    // Alternate rows carry a near-black stripe so the eye can follow
                    // one row across the id, category, marketplace and description
                    // columns on a wide panel.
                    let item = ListItem::new(Line::from(spans));
                    if row_index % 2 == 1 {
                        item.style(Style::default().bg(STORE_BAND_BG))
                    } else {
                        item
                    }
                })
                .collect();
            let list = List::new(items)
                .highlight_style(cursor_style)
                .highlight_symbol("");
            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(list, list_area, &mut state);
        }
        StoreBody::Line(text, style) => {
            // Wrap rather than clip: with the navigators sharing the body, the
            // result column is narrower, and a long state line (a failed fetch
            // naming its cause) must stay fully readable instead of being silently
            // truncated at the column edge.
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(text, style)))
                    .block(Block::default())
                    .style(Style::default())
                    .wrap(Wrap { trim: false }),
                list_area,
            );
        }
    }

    let footer = connector_catalogue_footer(browser, dim_style, installed_style);
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::default().bg(STORE_CHROME_BG))
            .wrap(Wrap { trim: true }),
        footer_area,
    );

    // The bottom-edge search box: the live filter field on the left and the
    // click-to-clear button on the right. The box reflects the browser's query
    // exactly, so typing and the button stay in step without any extra state; the
    // button is inert while the query is already empty.
    app.connector_store_clear_area =
        render_store_search_box(frame, &browser.query, search_area, normal_style, dim_style);
}

/// Render the connector-catalogue category navigator into `area`.
///
/// The rows are the `ALL` sentinel, the `Installed` pseudo-category, then the
/// derived, sorted distinct categories, each labelled `Name (N)` with its entry
/// count in the panel accent. The row under the category cursor carries a block
/// cursor through [`ratatui::widgets::List::highlight_style`], independent of the
/// result-list cursor; the active category is painted in the active-category
/// colour. A still-loading catalogue yields an `ALL`-only navigator with counts.
/// The navigator viewport height is recorded on the browser so the handlers clamp
/// a wheel scroll; the scroll offset is otherwise left exactly as the wheel set
/// it, so scrolling away from the cursor sticks.
fn render_connector_catalogue_navigator(
    frame: &mut Frame,
    browser: &mut ragent_connectors::CatalogueBrowser,
    area: Rect,
    cursor_style: Style,
    active_style: Style,
    normal_style: Style,
) {
    use ratatui::widgets::{List, ListItem, ListState};

    if area.width == 0 || area.height == 0 {
        return;
    }

    // Record the navigator viewport height so the key and mouse handlers clamp the
    // wheel scroll and keep the cursor visible. The per-frame visibility sync is
    // *not* done here: a wheel scroll deliberately moves the viewport away from the
    // cursor, and a render-time re-sync would undo it.
    browser.category_viewport = area.height as usize;

    let rows = browser.category_row_count();
    let active = browser.active_category().to_string();
    let selected = browser.category_cursor_row();
    let count_style = Style::default().fg(STORE_ACCENT);
    let items: Vec<ListItem> = (0..rows)
        .map(|row| {
            let label = browser.category_row_label(row);
            let style = if label.eq_ignore_ascii_case(&active) {
                active_style
            } else {
                normal_style
            };
            // Each row shows the category name with its entry count, so the size of
            // each category is visible before selecting it. The count is painted in
            // the panel accent so the name stays the reading element.
            let display = browser.category_row_display(row);
            // The label is a prefix of the display text, so its byte length is a
            // valid split point and the count suffix keeps its own accent style.
            let split = label.len().min(display.len());
            let (name, count) = display.split_at(split);
            ListItem::new(Line::from(vec![
                Span::styled(" ".to_string(), style),
                Span::styled(name.to_string(), style),
                Span::styled(count.to_string(), count_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(cursor_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(selected));
    *state.offset_mut() = browser.category_scroll.min(rows.saturating_sub(1));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Render the connector-catalogue marketplace navigator into `area`.
///
/// Mirrors [`render_connector_catalogue_navigator`]: the rows are the `ALL`
/// sentinel first and then the derived, sorted distinct marketplaces, each
/// labelled `Name (N)`. The row under the marketplace cursor carries a block
/// cursor, independent of the category and result cursors; the active marketplace
/// is painted in the active-category colour. The viewport height is recorded on the
/// browser so the handlers clamp a wheel scroll.
fn render_connector_marketplace_navigator(
    frame: &mut Frame,
    browser: &mut ragent_connectors::CatalogueBrowser,
    area: Rect,
    cursor_style: Style,
    active_style: Style,
    normal_style: Style,
) {
    use ratatui::widgets::{List, ListItem, ListState};

    if area.width == 0 || area.height == 0 {
        return;
    }

    browser.marketplace_viewport = area.height as usize;

    let rows = browser.marketplace_row_count();
    let active = browser.active_marketplace().to_string();
    let selected = browser.marketplace_cursor_row();
    let count_style = Style::default().fg(STORE_ACCENT);
    let items: Vec<ListItem> = (0..rows)
        .map(|row| {
            let label = browser.marketplace_row_label(row);
            let style = if label.eq_ignore_ascii_case(&active) {
                active_style
            } else {
                normal_style
            };
            let display = browser.marketplace_row_display(row);
            let split = label.len().min(display.len());
            let (name, count) = display.split_at(split);
            ListItem::new(Line::from(vec![
                Span::styled(" ".to_string(), style),
                Span::styled(name.to_string(), style),
                Span::styled(count.to_string(), count_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(cursor_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(selected));
    *state.offset_mut() = browser.marketplace_scroll.min(rows.saturating_sub(1));
    frame.render_stateful_widget(list, area, &mut state);
}

/// The marketplace and description column widths for one connector-catalogue
/// result row of `row_width` columns.
///
/// The id and category columns are fixed at `id_width` and `category_width`; the
/// cursor gutter, the one-column installed lane ([`STORE_INSTALLED_MARKER`]), and
/// the three single-space separators are fixed overhead. The marketplace is capped
/// by [`CONNECTOR_CATALOGUE_MARKETPLACE_MAX`] and takes at most half of what
/// remains, and the description is given **every other column**: it is the row's
/// last column, so it renders right up to the panel's inner right edge, and a long
/// description is clipped there rather than pushing the id, category or marketplace
/// columns off the row. Mirrors [`store_flex_widths`].
fn connector_flex_widths(
    row_width: usize,
    id_width: usize,
    category_width: usize,
) -> (usize, usize) {
    // `> ` (2) + installed lane (1) + id + sep (1) + category + sep (1) +
    // marketplace + sep (1) + description.
    let fixed = 2 + STORE_INSTALLED_MARKER.len() + id_width + 1 + category_width + 1 + 1;
    let available = row_width.saturating_sub(fixed);
    let marketplace = (available / 2).min(CONNECTOR_CATALOGUE_MARKETPLACE_MAX as usize);
    (marketplace, available.saturating_sub(marketplace))
}

/// The panel title line for the connector-catalogue browser.
///
/// A single line of styled spans rather than one flat string, exactly as
/// [`plugin_store_title`], so the catalogue name reads as a badge, the counts as
/// the primary fact, the active filters as labelled values, and any parser skip or
/// in-flight enrichment as a trailing note that is visible at a glance rather than
/// silently short.
fn connector_catalogue_title(browser: &ragent_connectors::CatalogueBrowser) -> Line<'static> {
    let muted = store_muted_style();
    let accent = Style::default()
        .fg(STORE_ACCENT)
        .add_modifier(Modifier::BOLD);
    let badge = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let value = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![
        Span::styled(" Claude Connectors ", badge),
        Span::styled(
            format!(" {} of {} ", browser.filtered.len(), browser.all.len()),
            accent,
        ),
        Span::styled(" search: ", muted),
        Span::styled(
            if browser.query.is_empty() {
                "-".to_string()
            } else {
                browser.query.clone()
            },
            if browser.query.is_empty() {
                muted
            } else {
                value
            },
        ),
        Span::styled(" category ", muted),
        Span::styled(browser.selected_category_label().to_string(), value),
        Span::styled(" marketplace ", muted),
        Span::styled(browser.active_marketplace().to_string(), value),
    ];
    if browser.skipped > 0 {
        spans.push(Span::styled(
            format!("  {} skipped ", browser.skipped),
            Style::default().fg(Color::Yellow),
        ));
    }
    if browser.enriching {
        spans.push(Span::styled(" loading details... ", muted));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Decide what the connector-catalogue panel body should show.
fn connector_catalogue_body(browser: &ragent_connectors::CatalogueBrowser) -> StoreBody {
    use crate::app::ConnectorBrowseStatus;

    match &browser.status {
        ConnectorBrowseStatus::Loading => StoreBody::Line(
            "loading connector catalogue...".to_string(),
            Style::default().fg(Color::Cyan),
        ),
        ConnectorBrowseStatus::Failed(reason) => StoreBody::Line(
            format!("catalogue failed: {reason}"),
            Style::default().fg(Color::Red),
        ),
        ConnectorBrowseStatus::Empty | ConnectorBrowseStatus::Ready => {
            if browser.has_results() {
                StoreBody::Rows
            } else {
                StoreBody::Line(
                    connector_catalogue_empty_line(browser),
                    Style::default().fg(Color::DarkGray),
                )
            }
        }
    }
}

/// The panel's empty-state line: the no-entries or no-match text, plus the
/// malformed-entry count when the parser skipped any.
fn connector_catalogue_empty_line(browser: &ragent_connectors::CatalogueBrowser) -> String {
    let skipped = browser.skipped;
    if browser.all.is_empty() {
        if skipped == 0 {
            "no connectors in this catalogue".to_string()
        } else {
            format!("no connectors in this catalogue ({skipped} malformed entries skipped)")
        }
    } else if skipped == 0 {
        "no matching connectors".to_string()
    } else {
        format!("no matching connectors ({skipped} malformed entries skipped)")
    }
}

/// The footer line: the most recent install notice when one is present,
/// otherwise the key-hint line.
fn connector_catalogue_footer(
    browser: &ragent_connectors::CatalogueBrowser,
    dim_style: Style,
    installed_style: Style,
) -> Line<'static> {
    if let Some(notice) = browser.last_install.as_deref() {
        return Line::from(Span::styled(notice.to_string(), installed_style));
    }
    Line::from(Span::styled(
        ragent_config::i18n::t(ragent_config::i18n::MessageKey::CatalogueFooter).into_owned(),
        dim_style,
    ))
}

/// Render the transient run-complete banner as a centered one-line popup
/// near the top of the screen (FR-012).
///
/// The banner text is produced by the `Event::RunCostSummary` handler and
/// takes the form `* run complete * {in}+{out} tokens * ${cost} * {dur}s`.
/// It is drawn on top of all other UI and cleared on the next keypress.
fn render_run_cost_banner(frame: &mut Frame, app: &mut App) {
    let Some(text) = app.run_cost_banner.as_ref() else {
        return;
    };
    let area = frame.area();
    // One content line + top/bottom borders = 3 rows tall.
    let height = 3u16;
    // Width fits the text plus padding, capped to the screen width.
    let text_width = text.chars().count() as u16;
    let width = text_width
        .saturating_add(4)
        .min(area.width.saturating_sub(2))
        .max(text_width.saturating_add(2));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    // Anchor near the top of the screen, just below the status bar.
    let y = area.y + 1;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);
    let style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .style(Style::default().bg(Color::Black));
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text.clone(), style)))
            .alignment(Alignment::Center)
            .block(block),
        popup,
    );
}

/// Render the queue-control menu overlay opened by ALT-Q (spec `inputqueue`
/// FR-021, FR-023).
///
/// The menu is a small centred modal that reuses the shared overlay drawing
/// path (`Clear` + a bordered `List`) and the standard key-dispatch machinery
/// (NFR-007, NFR-009). It presents exactly four rows in a fixed order:
/// `Next`, `Stop`/`Resume`, `Clear`, and `Show`.
///
/// While the input queue is empty the `Next` row is drawn dimmed and a leading
/// space (instead of `>`) marks it non-selectable; while entries are pending it
/// is drawn like the other rows and carries the `>` selection marker (FR-023).
/// The `Stop`/`Resume` label is resolved by
/// [`App::queue_menu_halt_label`], which reflects whether a turn is executing
/// (FR-026), and all four labels are produced by [`App::queue_menu_labels`] so
/// the render and the action dispatch share one source of truth.
fn render_queue_menu(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let labels = app.queue_menu_labels();
    let title = format!(" Queue control - {} pending ", app.input_queue_len());

    // Width fits the widest label (with its selection marker and padding) and
    // the title, whichever is longer; height is the rows plus the
    // title/border chrome.
    let max_label = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let width = (max_label as u16)
        .saturating_add(6)
        .max(title.chars().count() as u16 + 2)
        .max(22);
    let height = (labels.len() as u16).saturating_add(5);
    let screen = frame.area();
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);
    app.queue_menu_area = area;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::White);
    let disabled_style = Style::default().fg(Color::DarkGray);

    let items: Vec<ListItem> = labels
        .iter()
        .enumerate()
        .map(|(i, &label)| {
            // Only the empty-queue `Next` row is non-selectable (FR-023); every
            // other row (including `Show`) is always actionable.
            let selectable = app.queue_menu_row_selectable(i);
            // The `>` marker is the on-screen expression of selectability: a
            // non-selectable row (empty-queue `Next`) must never carry it, even
            // when it is the current selection index (FR-023).
            let selected = selectable && i == app.queue_menu_selected;
            let marker = if selected { "> " } else { "  " };
            let style = if !selectable {
                disabled_style
            } else if selected {
                selected_style
            } else {
                normal_style
            };
            ListItem::new(Line::from(Span::styled(format!("{marker}{label}"), style)))
        })
        .collect();

    // Footer hints for navigation/selection of the queue-control menu.
    let footer = if inner.height > labels.len() as u16 {
        Line::from(Span::styled(
            "^/v move * Enter select * Esc close",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::default()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(
        app.queue_menu_selected.min(labels.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(footer).alignment(Alignment::Center),
        chunks[1],
    );
}

/// Render the scrollable queue-entry panel opened by the queue-control menu's
/// `Show` row (spec `inputqueue` `Show` row).
///
/// The panel is a modal that reuses the shared overlay drawing path (`Clear` +
/// a bordered `List`, NFR-007). It lists every queued entry oldest-first with a
/// running position, keeping the highlighted entry visible via the list's own
/// scroll offset; the highlighted row carries the block-cursor background. Only
/// `Esc` dismisses the panel; `Up`/`Down` move the highlight, `Enter` moves the
/// highlighted entry one step toward the front, and `Del` removes it.
fn render_queue_show_panel(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let len = app.input_queue.len();
    // Keep the highlight within range while the queue length can change under
    // the panel (e.g. a turn-boundary drain) so the block cursor is always valid.
    let selected = app.queue_show_selected.min(len.saturating_sub(1));
    let title = format!(" Queued messages - {len} ");

    // Size the panel to the entries and the widest entry text, clamped to the
    // screen so a long queue scrolls instead of overflowing.
    let screen = frame.area();
    let max_entry = app
        .input_queue
        .iter()
        .map(|e| e.text.chars().count())
        .max()
        .unwrap_or(0)
        .min(60);
    let width = (max_entry as u16)
        .saturating_add(10)
        .max(title.chars().count() as u16 + 2)
        .max(30);
    let height = (len as u16).saturating_add(5).min(16).min(screen.height);
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);
    app.queue_show_area = area;

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The highlighted row carries the block cursor: a full-row background that
    // marks the entry `Enter`/`Del` will act on.
    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::White);
    let index_style = Style::default().fg(Color::DarkGray);

    let items: Vec<ListItem> = app
        .input_queue
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let text = entry.text.lines().next().unwrap_or("").to_string();
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>3}", i + 1), index_style),
                Span::raw(" "),
                Span::styled(text, normal_style),
            ]))
        })
        .collect();

    let footer = if inner.height > len as u16 {
        Line::from(Span::styled(
            "^/v move * Enter move up * Del remove * Esc close",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::default()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    // The highlighted row carries the block cursor: `highlight_style` patches the
    // whole row after the items are drawn, so the background spans every cell
    // (the index prefix included) rather than only the text span.
    let list = List::new(items).highlight_style(cursor_style);
    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(footer).alignment(Alignment::Center),
        chunks[1],
    );
}

/// Render the plugin-store browse panel opened by `/plugins codex` and
/// `/plugins claude` (spec `pluginstores` T-006; FR-004, FR-005, FR-011,
/// FR-017, FR-018).
///
/// The panel is a bordered, titled modal overlay, modelled on
/// [`render_queue_show_panel`], presenting a title line with the store name, the
/// current search query, and the visible/total result count; a scrollable result
/// list; and a footer of key hints.
///
/// The highlighted result row carries the block cursor: a full-row background
/// applied through [`ratatui::widgets::List::highlight_style`], which patches the
/// whole row after the item spans are drawn, so the background spans every cell
/// (the leading `> ` marker included) rather than only the text span. A row whose
/// plugin id is in the derived installed set ([`PluginStoreBrowser::is_installed`])
/// is painted in the installed colour and carries the installed marker
/// ([`STORE_INSTALLED_MARKER`]) in the one-column lane between the cursor
/// gutter and the id column, independent of the cursor (FR-005, A7).
///
/// The body renders an explicit state line rather than a blank list when there is
/// nothing to show (FR-017): a loading row while the fetch is in flight (FR-016),
/// an inline error row naming the cause when it failed (FR-013), and a
/// `no matching plugins` line when the index or the query yields no results.
///
/// The panel area is recomputed from `Frame::area` every frame, so a terminal
/// resize re-derives a centred, fully visible modal (FR-018); it is stored back in
/// [`App::plugin_store_area`] for tests and hit-testing.
///
/// The body is split into the left navigator column and the result column
/// (spec `catnav` FR-001, FR-020; `pluginstores` FR-052). The left column is split
/// vertically into the category navigator on top and the marketplace navigator
/// below; each lists `ALL` first, then the distinct values the index declares, with
/// the active value in the active-category colour and a block cursor on the row
/// under that navigator's cursor. Both navigators stay visible and operable while
/// the fetch is loading or the result set is empty (FR-016, FR-018, FR-019). All
/// three column areas are recomputed from the current frame and stored back in
/// [`App::plugin_store_nav_area`], [`App::plugin_store_marketplace_area`], and
/// [`App::plugin_store_result_area`]. The bottom inner row carries the panel's
/// search box (FR-051), whose field and clear-button areas are recomputed here and
/// stored back in [`App::plugin_store_search_area`] and
/// [`App::plugin_store_clear_area`] for click hit-testing.
fn render_plugin_store_panel(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    if app.plugin_store.is_none() {
        return;
    }

    let screen = frame.area();
    // Size the panel to the screen with a margin, so it stays centred and fully
    // visible as the terminal changes size (FR-018).
    let width = screen
        .width
        .saturating_sub(4)
        .clamp(30, 140)
        .min(screen.width);
    let height = screen
        .height
        .saturating_sub(2)
        .clamp(8, PLUGIN_STORE_MAX_HEIGHT)
        .min(screen.height);
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);
    app.plugin_store_area = area;

    // The title names the store, the search field, the counts, and the active
    // category (FR-005); it needs only an immutable read of the browser.
    let title = match app.plugin_store.as_ref() {
        Some(browser) => plugin_store_title(browser),
        // The early return above guarantees `Some`, but read defensively so no
        // path can panic if that guard ever drifts.
        None => Line::default(),
    };
    // A calm slate-blue frame rather than the saturated magenta of the older
    // build: the border is chrome, and the cyan/magenta cursor colours must stay
    // the brightest thing on the panel so the eye lands on the cursor (FR-004,
    // FR-005).
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(STORE_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Reserve the last inner row for the footer key hints and the row above it
    // for the bottom search box; the body takes what remains.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];
    let search_area = chunks[2];
    app.plugin_store_search_area = search_area;

    // Split the body into the left category navigator and the result column. The
    // navigator width is recomputed from the current frame every draw, so a
    // terminal resize re-derives it and the column stays fully visible
    // (spec `catnav` FR-020).
    let nav_width = store_nav_width(body_area.width);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(nav_width),
            Constraint::Length(STORE_NAV_GAP),
            Constraint::Min(0),
        ])
        .split(body_area);
    let nav_area = columns[0];
    let result_area = columns[2];
    app.plugin_store_nav_area = nav_area;
    // The left navigator column is split vertically into the category navigator
    // (top half) and the marketplace navigator (bottom half); each pane owns a full
    // viewport so both stay usable side by side in one column (FR-052).
    let nav_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(nav_area);
    let category_area = nav_panes[0];
    let marketplace_area = nav_panes[1];

    // Each navigator carries a one-row title above its `ALL` entry naming the
    // navigation type, painted in reversed video (FR-004). Reserve that row out of
    // each pane so the rows beneath still scroll inside what remains. The cached
    // hit-test areas are the row areas below the title, so a click or wheel on a
    // title row is inert and never mis-mapped onto the `ALL` row.
    let category_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(category_area);
    let category_title_area = category_chunks[0];
    let category_rows_area = category_chunks[1];
    let marketplace_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(marketplace_area);
    let marketplace_title_area = marketplace_chunks[0];
    let marketplace_rows_area = marketplace_chunks[1];
    app.plugin_store_nav_area = category_rows_area;
    app.plugin_store_marketplace_area = marketplace_rows_area;
    // The result column splits vertically into a fixed column-heading row (always
    // painted, never scrolled) and the scrolling result list beneath it. The
    // headings are drawn outside the `List`, so the list's own scroll keeps rows
    // under the cursor visible while the headings stay pinned (FR-004).
    let result_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(result_area);
    let heading_area = result_chunks[0];
    let list_area = result_chunks[1];
    // Mouse hit-testing and the cursor-to-row mapping both reason about result
    // rows; anchor the cached area on the list (below the heading) so a click or
    // wheel on the heading row is inert.
    app.plugin_store_result_area = list_area;

    // Row text colours. The id and description carry the reading colour;
    // the version and marketplace columns step back into a muted tone so a
    // horizontally busy row still leads the eye with its name and its purpose.
    let normal_style = Style::default().fg(Color::White);
    let active_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let installed_style = Style::default().fg(Color::Green);
    let muted_style = store_muted_style();
    let tag_style = Style::default().fg(STORE_ACCENT);
    let dim_style = Style::default().fg(theme::colors::HINT);

    let Some(browser) = app.plugin_store.as_mut() else {
        return;
    };
    // Focus distinction (FR-009, FR-052): the pane that holds keyboard focus paints
    // its cursor in its filled colour; the other panes' cursors are dimmed, so which
    // pane owns `Up`/`Down` is visible at a glance. The two navigators share one
    // focused colour and the result cursor keeps its own colour, independent of them
    // (FR-002).
    let focused_result_cursor = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let focused_nav_cursor = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let unfocused_cursor = Style::default().fg(Color::White).bg(Color::DarkGray);
    let nav_cursor_style = if browser.focus == StorePane::Categories {
        focused_nav_cursor
    } else {
        unfocused_cursor
    };
    let marketplace_cursor_style = if browser.focus == StorePane::Marketplaces {
        focused_nav_cursor
    } else {
        unfocused_cursor
    };
    let cursor_style = if browser.focus == StorePane::Results {
        focused_result_cursor
    } else {
        unfocused_cursor
    };
    // The header band above each navigator's `ALL` entry names the navigation
    // type (FR-004). It is painted reversed video - black on white - so it reads
    // as a distinct header, and is padded to the column width so the band runs
    // edge to edge instead of stopping dead after the label.
    let title_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let nav_band = |label: &'static str, width: u16| {
        let text = format!(" {label}");
        let pad = (width as usize).saturating_sub(text.chars().count());
        Line::from(Span::styled(
            format!("{text}{}", " ".repeat(pad)),
            title_style,
        ))
    };
    frame.render_widget(
        Paragraph::new(nav_band(STORE_CATEGORY_TITLE, category_title_area.width)),
        category_title_area,
    );
    frame.render_widget(
        Paragraph::new(nav_band(
            STORE_MARKETPLACE_TITLE,
            marketplace_title_area.width,
        )),
        marketplace_title_area,
    );
    // The category navigator: `ALL` first, then the derived categories (FR-001).
    render_plugin_store_navigator(
        frame,
        browser,
        category_rows_area,
        nav_cursor_style,
        active_style,
        normal_style,
    );
    // The marketplace navigator: `ALL` first, then the derived marketplaces (FR-052).
    render_plugin_store_marketplace_navigator(
        frame,
        browser,
        marketplace_rows_area,
        marketplace_cursor_style,
        active_style,
        normal_style,
    );

    // The body: result rows, or one explicit state line (FR-013, FR-016, FR-017,
    // FR-019). The navigator stays visible above either case.
    let body = plugin_store_body(browser);
    match body {
        StoreBody::Rows => {
            let visible = browser.filtered.len();
            // The id and version columns are fixed-width; the marketplace column
            // keeps its ceiling and the description takes every remaining column. As
            // the row's last column it renders straight up to the panel's inner right
            // edge, and a description longer than that room is clipped there by the
            // row area rather than pushing another column off the row (FR-018,
            // FR-048).
            let id_width = PLUGIN_STORE_ID_MAX;
            let version_width = PLUGIN_STORE_VERSION_MAX;
            let (marketplace_width, desc_width) =
                store_flex_widths(result_area.width as usize, id_width, version_width);
            // The fixed column-heading row: the same column template as a result
            // row so each label sits exactly above its column, painted once above
            // the list so it never scrolls. The dark-on-light style distinguishes
            // it from both the normal and installed row colours (FR-004), and the
            // band is padded to the full result width so it reads as a header strip
            // rather than a label floating over the rows. A description heading
            // wider than the room left in the row is clipped by the row area,
            // exactly as its values are.
            let heading_style = Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD);
            let heading_text = format!(
                "   {PLUGIN_STORE_HEADING_ID:<id_width$} {PLUGIN_STORE_HEADING_VERSION:<version_width$} {PLUGIN_STORE_HEADING_MARKETPLACE:<marketplace_width$} {PLUGIN_STORE_HEADING_DESCRIPTION:<desc_width$}",
            );
            let heading_pad =
                (result_area.width as usize).saturating_sub(heading_text.chars().count());
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("{heading_text}{}", " ".repeat(heading_pad)),
                    heading_style,
                ))),
                heading_area,
            );
            // The block cursor's position, clamped into the filtered set; the list's
            // own scroll keeps it visible (FR-010) beneath the pinned heading row.
            // It is resolved before the rows are built so each row can paint the
            // cursor gutter marker itself.
            let selected = browser.cursor.min(visible.saturating_sub(1));
            let items: Vec<ListItem> = browser
                .filtered
                .iter()
                .enumerate()
                .map(|(row_index, &index)| {
                    let entry = &browser.all[index];
                    let installed = browser.is_installed(&entry.id);
                    // The id and description carry the reading colour; the version
                    // and marketplace step back into a muted tone so a wide row still
                    // leads with the plugin's name and what it does, and the
                    // installed marker is the one saturated green accent.
                    let base_style = if installed {
                        installed_style
                    } else {
                        normal_style
                    };
                    let id_field = truncate_bytes_no_ellipsis(&entry.id, id_width);
                    let version_field =
                        truncate_bytes_no_ellipsis(version_display(&entry.version), version_width);
                    let marketplace_field =
                        truncate_bytes_no_ellipsis(&entry.marketplace, marketplace_width);
                    let desc_field = truncate_bytes_no_ellipsis(&entry.description, desc_width);
                    // The two-column gutter mirrors the heading's leading indent and
                    // carries a `>` only on the row under the cursor, so the cursor is
                    // visible even where a background-only highlight washes out. It is
                    // followed by the one-column installed lane, which every row paints
                    // - the marker or a blank - so the columns stay aligned whether or
                    // not the row is installed.
                    let gutter = if row_index == selected { "> " } else { "  " };
                    let installed_span = if installed {
                        Span::styled(
                            STORE_INSTALLED_MARKER.to_string(),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(" ".to_string(), base_style)
                    };
                    let mut spans = vec![
                        Span::styled(gutter.to_string(), base_style),
                        installed_span,
                        Span::styled(format!("{id_field:<id_width$} "), base_style),
                        Span::styled(format!("{version_field:<version_width$} "), muted_style),
                        Span::styled(
                            format!("{marketplace_field:<marketplace_width$} "),
                            tag_style,
                        ),
                        Span::styled(desc_field, base_style),
                    ];
                    // Pad the row to the full result width: the trailing run carries
                    // the row's own style, so a zebra band (and the cursor background
                    // the list paints over it) reaches the column's right edge instead
                    // of stopping after the last glyph. A description wider than that
                    // edge paints nothing past it - the row area clips the surplus.
                    let painted: usize =
                        spans.iter().map(|span| span.content.chars().count()).sum();
                    let pad = (result_area.width as usize).saturating_sub(painted);
                    if pad > 0 {
                        spans.push(Span::styled(" ".repeat(pad), base_style));
                    }
                    // Alternate rows carry a near-black stripe so the eye can follow
                    // one row across the id, version, marketplace and description
                    // columns on a wide panel.
                    let item = ListItem::new(Line::from(spans));
                    if row_index % 2 == 1 {
                        item.style(Style::default().bg(STORE_BAND_BG))
                    } else {
                        item
                    }
                })
                .collect();
            let list = List::new(items)
                .highlight_style(cursor_style)
                .highlight_symbol("");
            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(list, list_area, &mut state);
        }
        StoreBody::Line(text, style) => {
            // Wrap rather than clip: with the navigator sharing the body, the
            // result column is narrower, and a long state line (a failed fetch
            // naming its cause, FR-013) must stay fully readable instead of being
            // silently truncated at the column edge. The line starts at the column's
            // first cell, so the whole width is available to the wrap.
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(text, style)))
                    .block(Block::default())
                    .style(Style::default())
                    .wrap(Wrap { trim: false }),
                result_area,
            );
        }
    }

    let footer = plugin_store_footer(browser, dim_style, installed_style);
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::default().bg(STORE_CHROME_BG))
            .wrap(Wrap { trim: true }),
        footer_area,
    );

    // The bottom-edge search box (FR-004): the live filter field on the left and
    // the click-to-clear button on the right. The box reflects the browser's
    // query exactly, so typing and the button stay in step without any extra
    // state; the button is inert while the query is already empty. The painted
    // button area is cached for click hit-testing.
    app.plugin_store_clear_area =
        render_plugin_store_search_box(frame, browser, search_area, normal_style, dim_style);
}

/// Render the plugin-store search box on the panel's bottom edge (spec
/// `pluginstores` FR-004).
///
/// The row is split into a leading `search:` label, the editable filter field
/// (the browser's live query, with a caret glyph standing in for the cursor), and
/// a right-aligned `[x] clear` button. The field mirrors [`PluginStoreBrowser::query`]
/// byte-for-byte so the box and the result filter cannot drift; a query wider than
/// the field is clipped from the left, so the most recently typed characters stay
/// visible. The painted clear-button area is returned so the caller can cache it
/// for click hit-testing; [`Rect::default`] is returned when the row is too narrow
/// to hold the button.
fn render_plugin_store_search_box(
    frame: &mut Frame,
    browser: &crate::app::PluginStoreBrowser,
    area: Rect,
    field_style: Style,
    dim_style: Style,
) -> Rect {
    render_store_search_box(frame, &browser.query, area, field_style, dim_style)
}

/// Render a bottom-edge catalogue search box for `query` into `area`.
///
/// The one implementation shared by the plugin-store and connector-catalogue
/// panels, so their bottom strips cannot drift apart. The row is split into a
/// leading `search:` label, the editable filter field (the live query, with a
/// caret glyph standing in for the cursor), and a right-aligned `[x] clear`
/// button. The field mirrors `query` byte-for-byte so the box and the result
/// filter cannot drift; a query wider than the field is clipped from the left, so
/// the most recently typed characters stay visible. The painted clear-button area
/// is returned so the caller can cache it for click hit-testing;
/// [`Rect::default`] is returned when the row is too narrow to hold the button.
fn render_store_search_box(
    frame: &mut Frame,
    query: &str,
    area: Rect,
    field_style: Style,
    dim_style: Style,
) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    // Reserve the clear button's columns at the right edge (when the row is wide
    // enough to hold it), then the label, leaving the field what remains. The
    // button is dropped entirely on an extremely narrow row rather than squeezing
    // the field away, so the field always keeps at least its minimum width.
    let clear_len = STORE_CLEAR_LABEL.chars().count() as u16;
    let has_clear = area.width >= clear_len + STORE_SEARCH_MIN;
    let button_width = if has_clear { clear_len } else { 0 };
    let field_width = area
        .width
        .saturating_sub(STORE_SEARCH_LABEL.len() as u16)
        .saturating_sub(button_width)
        .max(STORE_SEARCH_MIN.min(area.width));

    // Paint the label, the editable field, and the clear button as one row of
    // spans. The field's text is the query clipped to the field width from the
    // left; the caret follows it so the typing position is visible.
    let label_span = Span::styled(
        STORE_SEARCH_LABEL,
        Style::default()
            .fg(STORE_ACCENT)
            .add_modifier(Modifier::BOLD),
    );
    let mut field = query.to_string();
    let field_cols = field_width as usize;
    if field.chars().count() >= field_cols {
        // Keep the tail, so the newest characters are on screen.
        let skip = field.chars().count() + 1 - field_cols;
        field = field.chars().skip(skip).collect();
    }
    let used = field.chars().count();
    let mut field_text = field;
    // A caret marker fills the next cell so the live editing position reads
    // clearly even with an empty query.
    let caret = if used < field_cols { "|" } else { "" };
    field_text.push_str(caret);
    let pad = field_cols.saturating_sub(field_text.chars().count());
    let field_style = if query.is_empty() {
        // An empty field reads as a prompt rather than as typed content.
        store_muted_style()
    } else {
        field_style
    };
    let field_span = Span::styled(format!("{field_text}{}", " ".repeat(pad)), field_style);

    let mut spans = vec![label_span, field_span];
    let clear_area = if has_clear {
        let button_style = if query.is_empty() {
            dim_style
        } else {
            // The button lights up in the accent colour as soon as there is
            // something to clear, so its affordance tracks the filter state.
            Style::default()
                .fg(STORE_ACCENT)
                .add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(STORE_CLEAR_LABEL, button_style));
        // The button is right-aligned: its painted columns are the last
        // `clear_len` of the row.
        Rect {
            x: area.x + area.width - clear_len,
            y: area.y,
            width: clear_len,
            height: 1,
        }
    } else {
        Rect::default()
    };

    // The row carries the chrome background so the search box reads as a distinct
    // strip along the panel's bottom edge rather than loose text on the body.
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(STORE_CHROME_BG)),
        area,
    );
    clear_area
}

/// The navigator column width for a body of `body_width` columns (spec `catnav`
/// FR-020).
///
/// A third of the body, clamped to a readable range, and always narrow enough
/// that the result column keeps at least [`STORE_RESULT_MIN`] columns
/// plus the separator. A narrow terminal therefore shrinks the navigator rather
/// than clipping the result list.
fn store_nav_width(body_width: u16) -> u16 {
    let max_nav = body_width.saturating_sub(STORE_NAV_GAP + STORE_RESULT_MIN);
    (body_width / 3)
        .clamp(STORE_NAV_MIN, STORE_NAV_MAX)
        .min(max_nav)
}

/// The version cell text: the entry's version, or `-` when it is blank.
///
/// A vendor index frequently omits the version (the Codex marketplace never
/// declares one), so a blank field would otherwise be an invisible cell; `-`
/// makes the unknown state explicit (FR-004).
fn version_display(version: &str) -> &str {
    if version.trim().is_empty() {
        "-"
    } else {
        version
    }
}

/// The marketplace and description column widths for one plugin-store result row
/// of `row_width` columns (spec `pluginstores` FR-004, FR-048).
///
/// The id and version columns are fixed at `id_width` and `version_width`; the
/// cursor gutter, the one-column installed lane
/// ([`STORE_INSTALLED_MARKER`]), and the three single-space separators are
/// fixed overhead. The marketplace is capped by
/// [`PLUGIN_STORE_MARKETPLACE_MAX`] and takes at most half of what remains, and the
/// description is given **every other column**: it is the row's last column, so it
/// renders right up to the panel's inner right edge, and a long description is
/// clipped there rather than pushing the id, version or marketplace columns off the
/// row (FR-018).
fn store_flex_widths(row_width: usize, id_width: usize, version_width: usize) -> (usize, usize) {
    // `> ` (2) + installed lane (1) + id + sep (1) + version + sep (1) +
    // marketplace + sep (1) + description.
    let fixed = 2 + STORE_INSTALLED_MARKER.len() + id_width + 1 + version_width + 1 + 1;
    let available = row_width.saturating_sub(fixed);
    // The marketplace is a short label: it takes at most half the flexible space
    // (and at most its ceiling), and the description takes the rest.
    let marketplace = (available / 2).min(PLUGIN_STORE_MARKETPLACE_MAX);
    (marketplace, available.saturating_sub(marketplace))
}

/// Render the plugin-store category navigator into `area` (spec `catnav` FR-001,
/// FR-002, FR-016, FR-018).
///
/// The rows are the `ALL` sentinel first and then the derived, sorted distinct
/// categories. The row under the category cursor carries a block cursor through
/// [`ratatui::widgets::List::highlight_style`], independent of the result-list
/// cursor; the active category is painted in the active-category colour. A
/// still-loading or category-less index yields an `ALL`-only navigator. The
/// navigator viewport height is recorded on the browser so the handlers clamp a
/// wheel scroll; the scroll offset is otherwise left exactly as the wheel set it,
/// so scrolling away from the cursor sticks.
fn render_plugin_store_navigator(
    frame: &mut Frame,
    browser: &mut crate::app::PluginStoreBrowser,
    area: Rect,
    cursor_style: Style,
    active_style: Style,
    normal_style: Style,
) {
    use ratatui::widgets::{List, ListItem, ListState};

    if area.width == 0 || area.height == 0 {
        return;
    }

    // Record the navigator viewport height so the key and mouse handlers clamp the
    // wheel scroll and keep the category cursor visible (FR-007, FR-008, FR-012).
    // The per-frame visibility sync is *not* done here: a wheel scroll deliberately
    // moves the viewport away from the cursor, and a render-time re-sync would undo
    // it. The move and clear handlers keep the cursor visible instead.
    browser.category_viewport = area.height as usize;

    let rows = browser.category_row_count();
    let active = browser.active_category().to_string();
    let selected = browser.category_cursor_row();
    let count_style = Style::default().fg(STORE_ACCENT);
    let items: Vec<ListItem> = (0..rows)
        .map(|row| {
            let label = browser.category_row_label(row);
            let style = if label.eq_ignore_ascii_case(&active) {
                active_style
            } else {
                normal_style
            };
            // Each row shows the category name with its entry count (FR-053), so the
            // size of each category is visible before selecting it. The count is
            // painted in the panel accent so the name stays the reading element.
            let display = browser.category_row_display(row);
            // The label is a prefix of the display text, so its byte length is a
            // valid split point and the count suffix keeps its own accent style.
            let split = label.len().min(display.len());
            let (name, count) = display.split_at(split);
            ListItem::new(Line::from(vec![
                Span::styled(" ".to_string(), style),
                Span::styled(name.to_string(), style),
                Span::styled(count.to_string(), count_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(cursor_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(selected));
    *state.offset_mut() = browser.category_scroll.min(rows.saturating_sub(1));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Render the plugin-store marketplace navigator into `area` (spec `pluginstores`
/// FR-052).
///
/// The rows are the `ALL` sentinel first and then the derived, sorted distinct
/// marketplaces, mirroring [`render_plugin_store_navigator`]. The row under the
/// marketplace cursor carries a block cursor, independent of the category and
/// result cursors; the active marketplace is painted in the active-category colour.
/// A still-loading or marketplace-less index yields an `ALL`-only navigator. The
/// viewport height is recorded on the browser so the handlers clamp a wheel scroll,
/// which is otherwise left exactly as the wheel set it.
fn render_plugin_store_marketplace_navigator(
    frame: &mut Frame,
    browser: &mut crate::app::PluginStoreBrowser,
    area: Rect,
    cursor_style: Style,
    active_style: Style,
    normal_style: Style,
) {
    use ratatui::widgets::{List, ListItem, ListState};

    if area.width == 0 || area.height == 0 {
        return;
    }

    // Record the viewport height so the key and mouse handlers clamp the wheel
    // scroll and keep the marketplace cursor visible (FR-052).
    browser.marketplace_viewport = area.height as usize;

    let rows = browser.marketplace_row_count();
    let active = browser.active_marketplace().to_string();
    let selected = browser.marketplace_cursor_row();
    let count_style = Style::default().fg(STORE_ACCENT);
    let items: Vec<ListItem> = (0..rows)
        .map(|row| {
            let label = browser.marketplace_row_label(row);
            let style = if label.eq_ignore_ascii_case(&active) {
                active_style
            } else {
                normal_style
            };
            // Each row shows the marketplace name with its entry count (FR-053),
            // mirroring the category navigator: the count in the panel accent.
            let display = browser.marketplace_row_display(row);
            let split = label.len().min(display.len());
            let (name, count) = display.split_at(split);
            ListItem::new(Line::from(vec![
                Span::styled(" ".to_string(), style),
                Span::styled(name.to_string(), style),
                Span::styled(count.to_string(), count_style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(cursor_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    state.select(Some(selected));
    *state.offset_mut() = browser.marketplace_scroll.min(rows.saturating_sub(1));
    frame.render_stateful_widget(list, area, &mut state);
}

/// The body content of the plugin-store panel: either the result rows or a
/// single explicit state line (FR-013, FR-016, FR-017).
enum StoreBody {
    /// The filtered set is non-empty; draw the result rows.
    Rows,
    /// Render `text` in `style` as the body (loading / error / empty states).
    Line(String, Style),
}

/// The muted style used for the plugin-store's secondary text: a row's version
/// and marketplace columns, the title's separators, and an empty search field.
///
/// A cool light grey rather than `Color::DarkGray`, which is too dim to read on
/// the chrome bands the panel now paints (theme `colors::HINT` is the same
/// accessible tone; this variant carries a slight blue cast so it belongs to the
/// panel's slate accent family).
fn store_muted_style() -> Style {
    Style::default().fg(Color::Rgb(150, 155, 170))
}

/// The panel title for `browser` (FR-004, FR-005, FR-013, FR-016, FR-025).
///
/// A single line of styled spans rather than one flat string, so the store name
/// reads as a badge, the counts as the primary fact, the active filters in the
/// panel accent, and the secondary hints in the muted tone:
///
/// ```text
///  Codex Plugin Store  3 of 3  search:    category ALL   marketplace ALL
/// ```
///
/// It names the store, the live search field, the visible/total counts, the active
/// category (spec `catnav` FR-005), the active marketplace (spec `pluginstores`
/// FR-052), and - when the parser skipped any malformed entries - the skipped
/// count, so a partial result set is visible at a glance rather than silently
/// short (FR-025). While the off-loop manifest enrichment is still running a
/// `loading details...` hint is appended, so a still-blank version/description
/// reads as pending rather than missing (FR-048). The rendered text stays the same
/// words the flat version used, so the title remains greppable in tests and logs.
fn plugin_store_title(browser: &crate::app::PluginStoreBrowser) -> Line<'static> {
    let muted = store_muted_style();
    let accent = Style::default()
        .fg(STORE_ACCENT)
        .add_modifier(Modifier::BOLD);
    let value = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let mut spans = vec![
        Span::styled(
            format!(" {} Plugin Store ", browser.kind.label()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} of {} ", browser.filtered.len(), browser.all.len()),
            accent,
        ),
        Span::styled(" search: ", muted),
        Span::styled(
            if browser.query.is_empty() {
                "-".to_string()
            } else {
                browser.query.clone()
            },
            if browser.query.is_empty() {
                muted
            } else {
                value
            },
        ),
        Span::styled(" category ", muted),
        Span::styled(browser.active_category().to_string(), value),
        Span::styled(" marketplace ", muted),
        Span::styled(browser.active_marketplace().to_string(), value),
    ];
    if browser.skipped > 0 {
        spans.push(Span::styled(
            format!("  {} skipped ", browser.skipped),
            Style::default().fg(Color::Yellow),
        ));
    }
    if browser.enriching {
        spans.push(Span::styled(" loading details... ", muted));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

/// Decide what the plugin-store panel body should show for `browser`
/// (FR-013, FR-016, FR-017, FR-025).
///
/// A still-loading index, a failed fetch, and an empty/unsatisfied result set
/// each produce an explicit line so the body is never a blank list. An empty or
/// unsatisfied list also names the skipped-malformed-entry count when the parser
/// dropped any entries, so a partial result is reported rather than hidden
/// (FR-025).
fn plugin_store_body(browser: &crate::app::PluginStoreBrowser) -> StoreBody {
    use crate::app::PluginStoreStatus;

    match &browser.status {
        PluginStoreStatus::Loading => StoreBody::Line(
            "loading store index...".to_string(),
            Style::default().fg(Color::Cyan),
        ),
        PluginStoreStatus::Failed(reason) => StoreBody::Line(
            format!("store index failed: {reason}"),
            Style::default().fg(Color::Red),
        ),
        PluginStoreStatus::Empty | PluginStoreStatus::Ready => {
            if browser.has_results() {
                StoreBody::Rows
            } else {
                StoreBody::Line(
                    plugin_store_empty_line(browser),
                    Style::default().fg(Color::DarkGray),
                )
            }
        }
    }
}

/// The panel's empty-state line: the no-entries or no-match text, plus the
/// malformed-entry count when the parser skipped any (FR-013, FR-017, FR-025).
///
/// Reported through the status rather than a result row, so the line is the only
/// place a partial parse surfaces in the body.
fn plugin_store_empty_line(browser: &crate::app::PluginStoreBrowser) -> String {
    let skipped = browser.skipped;
    if browser.all.is_empty() {
        if skipped == 0 {
            "no plugins in this store".to_string()
        } else {
            format!("no plugins in this store ({skipped} malformed entries skipped)")
        }
    } else if skipped == 0 {
        "no matching plugins".to_string()
    } else {
        format!("no matching plugins ({skipped} malformed entries skipped)")
    }
}

/// The footer line: the most recent install notice when one is present
/// (FR-006, FR-014), otherwise the key-hint line (FR-004).
///
/// The hint line names the navigation keys (spec `catnav` FR-005; `pluginstores`
/// FR-052): `Tab` cycles focus through the result list, the category navigator, and
/// the marketplace navigator, `Up`/`Down` move the focused pane's cursor, and `c`
/// clears the focused navigator's filter. `ENTER` is context-sensitive - it applies
/// a filter over a navigator and installs the highlighted row over the result list -
/// so the hint names both (`select/install`). A failed fetch already renders its
/// cause in the body (FR-013), so the footer stays the ordinary hint line in every
/// non-notice case.
fn plugin_store_footer(
    browser: &crate::app::PluginStoreBrowser,
    dim_style: Style,
    installed_style: Style,
) -> Line<'static> {
    if let Some(notice) = browser.last_install.as_deref() {
        return Line::from(Span::styled(notice.to_string(), installed_style));
    }
    Line::from(Span::styled(
        "Tab focus  Up/Down move  Enter select/install  c clear filter  Esc close".to_string(),
        dim_style,
    ))
}

/// Render a two-option `Yes`/`No` confirmation dialog and return its hit area.
///
/// `title` is the box title, `selected` the currently highlighted option (0 =
/// `Yes`, 1 = `No`). The body paints state only; the caller's key handling
/// interprets the selection. Shared by every `Yes`/`No` confirm so their layout
/// cannot drift.
fn render_yes_no_confirm(frame: &mut Frame, title: &str, selected: usize) -> Rect {
    use ratatui::widgets::{List, ListItem, ListState};

    // Option order matches the index constants: `Yes` at 0, `No` at 1.
    let options = ["Yes", "No"];

    let width = (title.chars().count() as u16 + 2).max(30);
    let height = (options.len() as u16).saturating_add(5);
    let screen = frame.area();
    let area = centered_rect_fixed(width, height, screen);
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::White);

    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_selected = i == selected;
            let marker = if is_selected { "> " } else { "  " };
            let style = if is_selected {
                selected_style
            } else {
                normal_style
            };
            ListItem::new(Line::from(Span::styled(format!("{marker}{label}"), style)))
        })
        .collect();

    let footer = if inner.height > options.len() as u16 {
        Line::from(Span::styled(
            "<-/-> move * Enter select * Esc close",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::default()
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let list = List::new(items);
    let mut state = ListState::default();
    state.select(Some(selected.min(options.len().saturating_sub(1))));
    frame.render_stateful_widget(list, chunks[0], &mut state);
    frame.render_widget(
        Paragraph::new(footer).alignment(Alignment::Center),
        chunks[1],
    );

    area
}

/// Render the `Clear the input queue?` confirmation dialog (spec `inputqueue`
/// T-021, FR-033, FR-034, NFR-010).
///
/// Opened by the queue-control menu's `Clear` row; it reuses the shared overlay
/// drawing path and the standard modal key-dispatch machinery in
/// [`crate::input::handle_key`] rather than adding a parallel input path
/// (NFR-010). It presents exactly two options, `Yes` and `No`, with `No`
/// selected by default so a stray `Enter` cannot empty the queue (FR-034).
///
/// The dialog only paints state here - it removes no entry. The queue is emptied
/// by the `Yes` gating step (FR-036); `No`/`Esc` leave it unchanged (FR-035).
fn render_queue_clear_confirm(frame: &mut Frame, app: &mut App) {
    app.queue_clear_confirm_area = render_yes_no_confirm(
        frame,
        " Clear the input queue? ",
        app.queue_clear_confirm_selected,
    );
}

/// Render the `Clear this project's memory?` confirmation dialog (opened by
/// `/memory clear`).
///
/// The dialog offers two options, `Yes` and `No`, with `No` selected by default
/// so a stray `Enter` cannot clear memory.
///
/// The dialog only paints state here - it removes no memory. The store is
/// emptied by the `Yes` gating step; `No`/`Esc` leave it unchanged.
fn render_memory_clear_confirm(frame: &mut Frame, app: &mut App) {
    render_yes_no_confirm(
        frame,
        " Clear this project's memory? ",
        app.memory_clear_confirm_selected,
    );
}

/// Render the destructive row-action confirmation dialog (spec `agentui`
/// T-009, FR-036, FR-037, FR-040).
///
/// A bordered `Yes`/`No` modal opened above the Agents and Teams windows; its
/// title names the action (`Kill sub-agent?`, `Cancel background task?`,
/// `Suspend teammate?`, `Shut down teammate?`) and its body names the exact
/// target the action would hit (`RowAction::body`), so the user answers about
/// a specific row rather than a generic warning. `No` is the default selection
/// (state.rs `ROW_ACTION_CONFIRM_NO`), so a stray `Enter` cannot terminate a
/// task or teammate (FR-069).
///
/// The dialog only paints state here - it dispatches nothing. The suspension,
/// termination, cancellation or shutdown runs in
/// `App::row_action_confirm_accept` on an explicit `Yes`; `No`/`Esc` (or a
/// click on `No`) dismiss it and leave the row untouched (FR-069).
///
/// Layout (spec `agentui` PLAN "Confirmation modal shape"): the body line, a
/// blank row, then the centred `[ Yes ]` / `[ No ]` button spans, a blank row
/// and the key hint. The two painted button rectangles are cached on
/// [`App::row_action_confirm_yes_area`] / [`App::row_action_confirm_no_area`]
/// and the dialog itself on [`App::row_action_confirm_area`], so the mouse
/// router can answer the dialog by clicking exactly the button it painted
/// (NFR-010).
fn render_row_action_confirm(frame: &mut Frame, app: &mut App) {
    let Some(ref pending) = app.row_action_confirm else {
        return;
    };
    let selected = pending.selected;

    let title = pending.action.title();
    let body = pending.action.body();

    // Widths: the buttons are fixed spans; the dialog must be wide enough for
    // the widest of the title, the body and the centred button pair (four
    // padding columns so nothing touches a border).
    const YES_LABEL: &str = "[ Yes ]";
    const NO_LABEL: &str = "[ No ]";
    const BUTTON_GAP: u16 = 8;
    const INNER_ROWS: u16 = 5;
    let yes_label_w = YES_LABEL.chars().count() as u16;
    let no_label_w = NO_LABEL.chars().count() as u16;
    let buttons_w = yes_label_w + BUTTON_GAP + no_label_w;
    let body_w = body.chars().count() as u16;
    let width = (title.chars().count() as u16)
        .max(body_w.saturating_add(4))
        .max(buttons_w.saturating_add(4))
        .max(34);
    let height = INNER_ROWS + 2; // plus the top and bottom border rows

    let screen = frame.area();
    let area = centered_rect_fixed(width, height, screen);
    app.row_action_confirm_area = area;
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Body line naming the target, centred on the row under the title.
    let body_x = inner.x + inner.width.saturating_sub(body_w) / 2;
    let body_line = Line::from(Span::styled(
        body,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(
        Paragraph::new(body_line),
        Rect::new(body_x, inner.y, body_w, 1),
    );

    // The centred button pair, painted exactly where the cached hit rects
    // below will say it is (NFR-010): a click answers the button it painted.
    let pair_x = inner.x + inner.width.saturating_sub(buttons_w) / 2;
    let buttons_y = inner.y + 2;
    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(Color::White);
    frame.render_widget(
        Paragraph::new(Span::styled(
            YES_LABEL,
            if selected == crate::app::ROW_ACTION_CONFIRM_YES {
                selected_style
            } else {
                normal_style
            },
        )),
        Rect::new(pair_x, buttons_y, yes_label_w, 1),
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            NO_LABEL,
            if selected == crate::app::ROW_ACTION_CONFIRM_NO {
                selected_style
            } else {
                normal_style
            },
        )),
        Rect::new(pair_x + yes_label_w + BUTTON_GAP, buttons_y, no_label_w, 1),
    );
    app.row_action_confirm_yes_area = Rect::new(pair_x, buttons_y, yes_label_w, 1);
    app.row_action_confirm_no_area =
        Rect::new(pair_x + yes_label_w + BUTTON_GAP, buttons_y, no_label_w, 1);

    // The key hint sits on the inner row under the buttons; omitted when the
    // dialog is too small to carry it.
    let hint = if inner.height > 4 {
        Line::from(Span::styled(
            "<-/-> move * Enter select * Esc close",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        Line::default()
    };
    frame.render_widget(
        Paragraph::new(hint).alignment(Alignment::Center),
        Rect::new(inner.x, inner.y.saturating_add(4), inner.width, 1),
    );
}

/// Terminal columns reserved as the gap between the Agents and Teams buttons.
const SIDE_BUTTON_GAP: u16 = 1;

/// Narrowest terminal width a side button is drawn at when the column has room
/// for it, borders included (spec `agentui` FR-001).
const SIDE_BUTTON_MIN_WIDTH: u16 = 7;

/// Compute the widths for the Agents/Teams side buttons from pre-built labels.
///
/// Each button width is its label width plus 2 for borders, with a minimum of
/// [`SIDE_BUTTON_MIN_WIDTH`]. While the pair plus the gap fits in
/// `button_col_w` both labels are drawn in full, so neither is truncated
/// (spec `agentui` FR-001). When the column is too narrow the available width
/// is split in proportion to the two natural widths, keeping each button at
/// least one column wide, so the pair always matches the column exactly and
/// neither button can be painted over the input field (NFR-010, NFR-004).
///
/// `draw_input_side_buttons` builds the labels once and passes them here so
/// the per-frame label allocations are not repeated for the width pass and the
/// render pass (PERF-048).
///
/// Returns `(agents_width, teams_width)`.
fn side_button_widths(agents_label: &str, teams_label: &str, button_col_w: u16) -> (u16, u16) {
    let agents_w = (agents_label.chars().count() as u16 + 2).max(SIDE_BUTTON_MIN_WIDTH);
    let teams_w = (teams_label.chars().count() as u16 + 2).max(SIDE_BUTTON_MIN_WIDTH);
    let total = agents_w
        .saturating_add(SIDE_BUTTON_GAP)
        .saturating_add(teams_w);
    if total <= button_col_w {
        return (agents_w, teams_w);
    }
    // Not enough room for both labels in full: share the column between the
    // two buttons in proportion to their natural widths.
    let budget = button_col_w.saturating_sub(SIDE_BUTTON_GAP);
    if budget <= 1 {
        return (budget, 0);
    }
    let natural = u32::from(agents_w) + u32::from(teams_w);
    let scaled = (u32::from(agents_w) * u32::from(budget) / natural) as u16;
    let agents_fit = scaled.clamp(1, budget - 1);
    (agents_fit, budget - agents_fit)
}

pub(crate) fn input_side_button_widths(app: &App, button_col_w: u16) -> (u16, u16) {
    side_button_widths(
        &agents_button_label(app),
        &teams_button_label(app),
        button_col_w,
    )
}

/// Total width the button column needs so the Agents/Teams labels are not
/// truncated. Never narrower than the responsive breakpoint width.
pub(crate) fn input_side_buttons_total_width(app: &App) -> u16 {
    let (agents_w, teams_w) = input_side_button_widths(app, u16::MAX);
    agents_w
        .saturating_add(SIDE_BUTTON_GAP)
        .saturating_add(teams_w)
        .max(12)
}

/// Label for the Agents button (spec `agentui` FR-002).
///
/// The count is the sum of the live sub-agent tasks and the live background
/// shell tasks. The bare label `" Agents "` is used when that sum is 0.
fn agents_button_label(app: &App) -> String {
    let count = app.active_tasks.len() + app.bg_tasks.len();
    if count == 0 {
        " Agents ".to_string()
    } else {
        format!(" Agents ({count}) ")
    }
}

/// Label for the Teams button (spec `agentui` FR-003).
///
/// The count is the number of team members whose status is not
/// `ShuttingDown`, `Stopped` or `Failed`, so a teammate that is winding down is
/// no longer advertised as live. The bare label `" Teams "` is used when that
/// count is 0.
fn teams_button_label(app: &App) -> String {
    let count = app
        .team_members
        .iter()
        .filter(|m| {
            !matches!(
                m.status,
                MemberStatus::ShuttingDown | MemberStatus::Stopped | MemberStatus::Failed
            )
        })
        .count();
    if count == 0 {
        " Teams ".to_string()
    } else {
        format!(" Teams ({count}) ")
    }
}

/// Styles for one side button, returned as `(text, border)`.
///
/// Three states, in precedence order: **active** paints white on blue with a
/// bold modifier (spec `agentui` FR-005), **enabled** paints white text inside a
/// dark grey border, and **disabled** dims both so the button reads as
/// unavailable (FR-004).
fn side_button_styles(active: bool, enabled: bool) -> (Style, Style) {
    if active {
        (
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Blue).bg(Color::Blue),
        )
    } else if enabled {
        (
            Style::default().fg(Color::White),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        let dimmed = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        (dimmed, dimmed)
    }
}

/// Truncate a side-button label to `cols` characters without splitting a
/// multi-byte character (spec `agentui` NFR-004).
///
/// Only reached when the button column is narrower than the two labels need;
/// the labels themselves are pure ASCII, so this is a defensive guard rather
/// than a routine path.
fn fit_button_label(label: &str, cols: u16) -> String {
    let budget = usize::from(cols);
    if label.chars().count() <= budget {
        return label.to_string();
    }
    label.chars().take(budget).collect()
}

/// Render the bordered Agents/Teams button column immediately left of the
/// message input (spec `agentui` FR-001..FR-005).
///
/// Both buttons are laid out inside `button_col_area` and their rects are
/// recorded in [`App::agents_button_area`] / [`App::teams_button_area`] so the
/// mouse hit-tests in the input handler resolve to the painted cells. Each
/// button's text and border style come from [`side_button_styles`]: active
/// (white on blue, bold) when its window is open, enabled (white) when its
/// subject exists, and dimmed when it does not.
fn draw_input_side_buttons(frame: &mut Frame, app: &mut App, button_col_area: Rect) {
    // Build the labels once and reuse them for both the width pass and the
    // render pass instead of allocating them again per call (PERF-048).
    let agents_label = agents_button_label(app);
    let teams_label = teams_button_label(app);
    let (agents_w, teams_w) =
        side_button_widths(&agents_label, &teams_label, button_col_area.width);
    let agents_x = button_col_area.x;
    let teams_x = agents_x
        .saturating_add(agents_w)
        .saturating_add(SIDE_BUTTON_GAP);
    let y = button_col_area.y;
    let h = button_col_area.height.max(3);

    app.agents_button_area = Rect::new(agents_x, y, agents_w, h);
    app.teams_button_area = Rect::new(teams_x, y, teams_w, h);

    let agents_enabled = app.agents_window_available();
    let teams_enabled = app.teams_window_available();
    let agents_active = agents_enabled && app.show_agents_window;
    let teams_active = teams_enabled && app.show_teams_window;

    let (agents_text_style, agents_border_style) =
        side_button_styles(agents_active, agents_enabled);
    let (teams_text_style, teams_border_style) = side_button_styles(teams_active, teams_enabled);

    // The label sits inside the two border columns; fit it to the inner width
    // so a squeezed column truncates the text instead of letting it bleed over
    // the input field.
    if agents_w > 0 {
        let agents_text = fit_button_label(&agents_label, agents_w.saturating_sub(2));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(agents_text, agents_text_style)))
                .style(agents_text_style)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(agents_border_style),
                ),
            app.agents_button_area,
        );
    }

    if teams_w > 0 {
        let teams_text = fit_button_label(&teams_label, teams_w.saturating_sub(2));
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(teams_text, teams_text_style)))
                .style(teams_text_style)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(teams_border_style),
                ),
            app.teams_button_area,
        );
    }
}

/// Apply a visual highlight to cells within the active text selection.
fn apply_selection_highlight(frame: &mut Frame, app: &App, pane: SelectionPane, area: Rect) {
    let sel = match &app.text_selection {
        Some(s) if s.pane == pane => s,
        _ => return,
    };
    let ((start_col, start_row), (end_col, end_row)) = sel.normalized();
    let highlight = Style::default().bg(Color::LightBlue).fg(Color::Black);
    let buf = frame.buffer_mut();
    for row in start_row..=end_row {
        if row < area.y || row >= area.bottom() {
            continue;
        }
        let col_start = if row == start_row {
            start_col.max(area.x)
        } else {
            area.x
        };
        let col_end = if row == end_row {
            (end_col + 1).min(area.right())
        } else {
            area.right()
        };
        for col in col_start..col_end {
            if let Some(cell) = buf.cell_mut(Position::new(col, row)) {
                cell.set_style(highlight);
            }
        }
    }
}

/// The accent style of a provider/model setup dialog: its frame, its titles and
/// its cursor.
///
/// Deliberately calm slate, not saturated cyan, so the dialog reads as chrome and
/// the filled cursor bar is the brightest thing on the panel (mirrors the plugin
/// and connector catalogue chrome).
fn setup_accent_style() -> Style {
    Style::default()
        .fg(SETUP_ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Muted style for a setup dialog's secondary text: captions, key hints, and the
/// columns that carry supporting facts rather than the entry's name.
fn setup_muted_style() -> Style {
    Style::default().fg(theme::colors::HINT)
}

/// The reversed-video section band pinned above a setup dialog's list (black on
/// grey), padded with `width` so it runs edge to edge like the plugin and
/// connector catalogue column headings.
///
/// `label` is painted exactly as given, so the caller controls the column alignment
/// and the band sits directly over the values beneath it.
fn setup_band(label: &str, width: u16) -> Line<'static> {
    let pad = (width as usize).saturating_sub(label.chars().count());
    Line::from(Span::styled(
        format!("{label}{}", " ".repeat(pad)),
        Style::default()
            .fg(SETUP_BAND_FG)
            .bg(SETUP_BAND_BG)
            .add_modifier(Modifier::BOLD),
    ))
}

/// The framed block every setup dialog shares: a slate accent border carrying
/// `title`, with the accent applied to the title text as well.
fn setup_block(title: Line<'static>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(SETUP_ACCENT))
}

/// The one-line key-hint footer shared by the setup dialogs, centred inside
/// `area` and painted in the muted tone.
fn render_setup_hint(frame: &mut Frame, area: Rect, hint: &str) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint.to_string(),
            setup_muted_style(),
        )))
        .alignment(Alignment::Center),
        area,
    );
}

/// Compose one setup-dialog heading strip: `left` spans at the column's left edge,
/// `right` in the muted tone flush to its right edge, with the gap padded between.
///
/// `width` is the dialog's inner width, so the strip always spans the full column.
fn setup_heading(left: Vec<Span<'static>>, right: &str, width: u16) -> Line<'static> {
    let painted: usize = left
        .iter()
        .map(|span| span.content.chars().count())
        .sum::<usize>()
        + right.chars().count();
    let mut spans = left;
    spans.push(Span::raw(
        " ".repeat((width as usize).saturating_sub(painted).max(1)),
    ));
    if !right.is_empty() {
        spans.push(Span::styled(right.to_string(), setup_muted_style()));
    }
    Line::from(spans)
}

/// The filled cursor bar every setup-dialog list paints on its selected row.
fn setup_cursor_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(SETUP_ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Pad `line` with trailing spaces to `width` so a zebra stripe (or the cursor bar)
/// reaches the dialog's inner right edge instead of stopping after the last glyph.
fn setup_pad_line(mut line: Line<'static>, width: u16) -> Line<'static> {
    let painted = line.width();
    let pad = (width as usize).saturating_sub(painted);
    if pad > 0 {
        line.spans.push(Span::raw(" ".repeat(pad)));
    }
    line
}

/// First row of the window that keeps `selected` visible in `visible` rows of a
/// `total`-row list.
///
/// Every setup selector uses this one rule, so the painted window, the scroll
/// legend and the cursor can never disagree: the window only starts moving once
/// the cursor reaches its bottom row, and it then follows the cursor one row at
/// a time.
fn setup_window_start(selected: usize, visible: usize, total: usize) -> usize {
    let visible = visible.max(1).min(total.max(1));
    if selected >= visible {
        (selected + 1).saturating_sub(visible)
    } else {
        0
    }
}

/// First row of a stateless scroll window that shows the row span `start..end`
/// inside a viewport of `height` rows.
///
/// The two-pane router setup keeps no scroll state (the cursor is the only
/// state), so its panes recompute the offset every frame from the selected
/// entry's own line span: an entry taller than the viewport shows from its first
/// line rather than being pushed off the top.
fn setup_scroll_to_show(start: usize, end: usize, height: usize) -> usize {
    if height == 0 {
        return 0;
    }
    let end = end.max(start.saturating_add(1));
    if end <= height {
        0
    } else {
        (end - height).min(start)
    }
}

/// Render the shared setup-dialog list shape: a framed block, a heading strip, an
/// optional pinned column band, the scrolling cursor list, and a key-hint footer.
///
/// `rows` are the pre-built entry lines (already padded to the dialog width by the
/// caller via [`setup_pad_line`]); `selected` is the cursor index. Every list-bearing
/// setup step shares this composition so the pickers cannot drift apart visually.
/// When `scroll_hint` is set the legend replaces the key hint on the last inner row
/// (the legend also states the key bindings, so nothing is lost).
#[allow(clippy::too_many_arguments)]
fn render_setup_list(
    frame: &mut Frame,
    area: Rect,
    title: Line<'static>,
    heading: Line<'static>,
    hint: &str,
    band: Option<Line<'static>>,
    rows: Vec<Line<'static>>,
    selected: usize,
    scroll_hint: Option<String>,
) {
    let block = setup_block(title);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let band_rows = u16::from(band.is_some());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(band_rows),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(Paragraph::new(heading), chunks[0]);
    if let Some(band) = band {
        frame.render_widget(Paragraph::new(band), chunks[1]);
    }

    let items: Vec<ListItem> = rows
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let item = ListItem::new(line);
            if index % 2 == 1 {
                item.style(Style::default().bg(SETUP_ZEBRA_BG))
            } else {
                item
            }
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(selected.min(items.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(setup_cursor_style())
        .highlight_symbol("");
    frame.render_stateful_widget(list, chunks[2], &mut state);

    match scroll_hint {
        Some(legend) => frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                legend,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            chunks[3],
        ),
        None => render_setup_hint(frame, chunks[3], hint),
    }
}

fn render_provider_setup_dialog(frame: &mut Frame, app: &mut App) {
    // Use a taller, capped dialog so longer provider/model lists fit without
    // clipping on typical terminal sizes.  80% height up to 30 rows gives the
    // provider picker enough room to show most entries.
    //
    // The rect is recorded before the step is borrowed so the mouse router knows
    // where the dialog is painted: a wheel notch inside it scrolls the dialog's
    // own list and a notch outside it is swallowed (FR-011).
    app.provider_setup_area = centered_rect_max(60, 80, 80, 30, frame.area());
    let area = app.provider_setup_area;
    frame.render_widget(Clear, area);

    let Some(step) = app.provider_setup.as_ref() else {
        return;
    };
    match step {
        ProviderSetupStep::LoadingModels {
            provider_id,
            provider_name,
        } => {
            let elapsed = app
                .model_loading_state
                .as_ref()
                .map_or(0, |s| s.started_at.elapsed().as_secs());
            let spinner =
                ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(elapsed as usize) % 10];
            let lines: Vec<Line<'_>> = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("Loading models for {}", provider_name),
                    setup_accent_style(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        format!("{} Fetching model list...", spinner),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(format!("  {}s", elapsed), setup_muted_style()),
                ]),
                Line::from(""),
                Line::from(Span::styled("Esc cancel", setup_muted_style())),
            ];
            let block = setup_block(Line::from(vec![
                Span::styled(" Loading Models ", setup_accent_style()),
                Span::styled(format!(" {} ", provider_id), setup_muted_style()),
            ]));
            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectProvider { selected, .. } => {
            // One calm column: a heading strip naming the step, a pinned
            // reversed-video band naming the row columns, the scrolling provider
            // list, and a key-hint footer. The provider with stored credentials
            // carries `[ok]` in the status lane; a local provider carries `[local]`
            // in the badge lane.
            let block = setup_block(Line::from(Span::styled(
                " Provider Setup ",
                setup_accent_style(),
            )));
            let inner = block.inner(area);
            // Which providers already have saved credentials, so the status lane
            // can show `[ok]` rather than a blank.
            let configured_ids: std::collections::HashSet<String> =
                App::get_configured_providers(&app.storage)
                    .into_iter()
                    .map(|p| p.id)
                    .collect();
            let configured_count = PROVIDER_LIST
                .iter()
                .filter(|(pid, _)| configured_ids.contains(*pid))
                .count();

            // Column geometry: the name column takes every column the fixed right
            // lanes leave, so a long provider name is the thing that gets the room.
            let right_lanes = PROVIDER_BADGE_WIDTH + PROVIDER_STATUS_WIDTH + PROVIDER_ID_MAX + 3;
            let name_width = (inner.width as usize)
                .saturating_sub(2 + right_lanes)
                .max(8);

            let name_style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
            let id_style = setup_muted_style();
            let badge_style = Style::default().fg(Color::Yellow);
            let ok_style = Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD);

            let rows: Vec<Line<'static>> = PROVIDER_LIST
                .iter()
                .map(|(pid, pname)| {
                    let name_field = truncate_bytes_no_ellipsis(pname, name_width);
                    let id_field = truncate_bytes_no_ellipsis(pid, PROVIDER_ID_MAX);
                    let badge = if *pid == "ollama" { "[local]" } else { "" };
                    let status = if configured_ids.contains(*pid) {
                        "[ok]"
                    } else {
                        ""
                    };
                    let line = Line::from(vec![
                        Span::styled(format!("{name_field:<name_width$} "), name_style),
                        Span::styled(format!("{id_field:<PROVIDER_ID_MAX$} "), id_style),
                        Span::styled(format!("{badge:<PROVIDER_BADGE_WIDTH$} "), badge_style),
                        Span::styled(format!("{status:<PROVIDER_STATUS_WIDTH$}"), ok_style),
                    ]);
                    setup_pad_line(line, inner.width)
                })
                .collect();

            // The scroll legend keeps the `more above` / `more below` counts explicit
            // (with the key bindings folded in) so a clipped list never looks whole. The
            // window is computed with the same bottom-anchored rule `List` uses, so the
            // legend describes exactly the rows that are painted.
            let band_rows = 1u16;
            let list_rows = inner.height.saturating_sub(1 + band_rows + 1) as usize;
            let total = rows.len();
            let scroll_hint = if total > list_rows && list_rows > 0 {
                let offset = if *selected >= list_rows {
                    (*selected + 1).saturating_sub(list_rows)
                } else {
                    0
                };
                let visible_end = (offset + list_rows).min(total);
                Some(format!(
                    "^ {} more above  |  {}-{} of {}  |  v {} more below  |  ^/v navigate  Enter select  Esc cancel",
                    offset,
                    offset + 1,
                    visible_end,
                    total,
                    total.saturating_sub(visible_end)
                ))
            } else {
                None
            };

            let heading = setup_heading(
                vec![Span::styled("Select a Provider", setup_accent_style())],
                &format!("{} of {} configured", configured_count, PROVIDER_LIST.len()),
                inner.width,
            );
            let band_text = format!(
                "{:<name_width$} {:<PROVIDER_ID_MAX$} {:<PROVIDER_BADGE_WIDTH$} {}",
                "Provider", "Id", "Kind", "Status"
            );
            render_setup_list(
                frame,
                area,
                Line::from(Span::styled(" Provider Setup ", setup_accent_style())),
                heading,
                "^/v navigate  Enter select  Esc cancel",
                Some(setup_band(&band_text, inner.width)),
                rows,
                *selected,
                scroll_hint,
            );
        }
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            key_field,
            endpoint_field,
            active_field,
            error,
            ..
        } => {
            // Wider dialog so the full API key (>= 48 chars) is visible.
            let area = centered_rect_max(80, 80, 100, 30, frame.area());
            app.provider_setup_area = area;
            frame.render_widget(Clear, area);

            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    format!("Configure {}", provider_name),
                    setup_accent_style(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter your API key:",
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
            ];

            // SEC-ragent-tui-002 (SECTASKS T-045): mask the key. The field was
            // rendered verbatim, so a live provider credential was readable
            // during a screen share, a terminal recording, or a shoulder-surf.
            let key_text = mask_secret(key_field.text());
            let key_cursor_display = if *active_field == 0 {
                key_field.cursor()
            } else {
                key_text.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 0 { "> " } else { "  " },
                    setup_accent_style(),
                ),
                Span::styled(
                    with_cursor_marker(&key_text, key_cursor_display),
                    Style::default().fg(Color::White),
                ),
            ]));

            if provider_id == "generic_openai" || provider_id == "azure_foundry" {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Endpoint URL (optional, e.g. http://localhost:11434/v1):",
                    Style::default().fg(Color::White),
                )));
                let ep_text = endpoint_field.text();
                let endpoint_cursor_display = if *active_field == 1 {
                    endpoint_field.cursor()
                } else {
                    ep_text.chars().count()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        if *active_field == 1 { "> " } else { "  " },
                        setup_accent_style(),
                    ),
                    Span::styled(
                        if ep_text.is_empty() {
                            "(use default/env)".to_string()
                        } else {
                            with_cursor_marker(ep_text, endpoint_cursor_display)
                        },
                        Style::default().fg(Color::White),
                    ),
                ]));
                lines.push(Line::from(Span::styled(
                    "Tab switches between API key and endpoint fields",
                    setup_muted_style(),
                )));
            }
            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    err.as_str(),
                    Style::default().fg(Color::Red),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Enter confirm  Esc cancel",
                setup_muted_style(),
            )));

            let block = setup_block(Line::from(vec![
                Span::styled(" Enter API Key ", setup_accent_style()),
                Span::styled(format!(" {} ", provider_name), setup_muted_style()),
            ]));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::DeviceFlowPending {
            flow,
            user_code,
            verification_uri,
        } => {
            let (title, header) = match flow {
                crate::app::DeviceFlowKind::Copilot => {
                    (" Copilot Sign In ", "GitHub Copilot Authorisation")
                }
                crate::app::DeviceFlowKind::GitHub => (" GitHub Sign In ", "GitHub Authorisation"),
            };

            let lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(header, setup_accent_style())),
                Line::from(""),
                Line::from("Visit the URL below and enter the code:"),
                Line::from(""),
                Line::from(Span::styled(
                    verification_uri.as_str(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::UNDERLINED),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Code: ", setup_muted_style()),
                    Span::styled(
                        user_code.as_str(),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Waiting for authorisation...",
                    setup_muted_style(),
                )),
                Line::from(""),
                Line::from(Span::styled("c copy code  Esc cancel", setup_muted_style())),
            ];

            let block = setup_block(Line::from(Span::styled(title, setup_accent_style())));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectModel {
            provider_id: _,
            provider_name,
            models,
            selected,
        } => {
            // Dialog width adapts to its content: columns are sized to the
            // widest cell in each column (see `content_sized_columns`), and
            // the dialog is just wide enough to hold the full row. The cap
            // (`centered_rect_max`) also clamps to the terminal, so small
            // terminals still fit. An empty model list has no columns to
            // measure, so it keeps a readable fixed width for its notice.
            let cells = model_picker_cells(models, *selected);
            let area = if models.is_empty() {
                centered_rect_max(80, 80, 68, 30, frame.area())
            } else {
                let inner_w = model_picker_inner_width(&cells) as u16;
                centered_rect_max(100, 80, inner_w.saturating_add(2), 30, frame.area())
            };
            app.provider_setup_area = area;
            frame.render_widget(Clear, area);

            let title = setup_block(Line::from(vec![
                Span::styled(" Select Model ", setup_accent_style()),
                Span::styled(format!(" {} ", provider_name), setup_muted_style()),
            ]));

            if models.is_empty() {
                let paragraph = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No models are currently available for this provider.",
                        Style::default().fg(Color::Yellow),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Check provider setup, authentication, or model discovery.",
                        setup_muted_style(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled("Esc cancel", setup_muted_style())),
                ])
                .block(title)
                .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
                return;
            }

            // Three inner bands: the pinned column heading, the scrolling model
            // list, and one footer row carrying the key hints and the window
            // position. The heading lives outside the table so it never scrolls.
            let inner = title.inner(area);

            // Measure the columns exactly as `content_sized_columns` will, so the
            // heading labels sit over their own column.
            let columns = content_sized_columns(&cells);
            let widths: Vec<usize> = columns
                .iter()
                .map(|c| match c {
                    Constraint::Length(w) | Constraint::Min(w) => *w as usize,
                    _ => 0,
                })
                .collect();

            // The pinned heading band: dark-on-light like the catalogue column
            // headings, each label over its column and the trailing space padded so
            // the band runs to the dialog's inner right edge.
            let band_text = model_picker_heading_band(&widths);

            // Window the list so the cursor stays visible.
            let list_rows = inner.height.saturating_sub(3) as usize;
            let visible = list_rows.max(1).min(models.len());
            let start = setup_window_start(*selected, visible, models.len());
            let end = (start + visible).min(models.len());

            // Row palette: the name leads in white, the supporting facts step back
            // into the muted tone so a wide row still leads with the model name.
            // Every cell is padded to its measured column width, so the columns stay
            // aligned down the list and under the heading band.
            let rows: Vec<Line<'static>> = cells
                .iter()
                .enumerate()
                .skip(start)
                .take(end - start)
                .map(|(_, row_cells)| {
                    let mut spans: Vec<Span<'static>> = Vec::with_capacity(row_cells.len());
                    for (col, value) in row_cells.iter().enumerate() {
                        let width = widths
                            .get(col)
                            .copied()
                            .unwrap_or_else(|| value.chars().count());
                        // The context-window and cost columns are numeric: right
                        // align them so their digits line up down the table.
                        let field = if col == 1 || col == 2 {
                            format!("{value:>width$}")
                        } else {
                            format!("{value:<width$}")
                        };
                        let style = if col == 0 {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            setup_muted_style()
                        };
                        spans.push(Span::styled(field, style));
                        if col + 1 < row_cells.len() {
                            spans.push(Span::raw(" ".repeat(MODEL_PICKER_COLUMN_SPACING)));
                        }
                    }
                    setup_pad_line(Line::from(spans), inner.width)
                })
                .collect();

            // The footer carries the window position (when the list is clipped), the
            // key hints, and the feature legend. `Showing` leads so a clipped window
            // says so even when a narrow dialog truncates the tail.
            let mut legend = String::new();
            if models.len() > visible {
                legend.push_str(&format!(
                    "Showing {}-{} of {}  |  ",
                    start + 1,
                    end,
                    models.len()
                ));
            }
            legend.push_str("^/v navigate  Enter select  Esc cancel");
            legend.push_str("  |  R reasoning  V vision  T tools");

            render_setup_list(
                frame,
                area,
                Line::from(vec![
                    Span::styled(" Select Model ", setup_accent_style()),
                    Span::styled(format!(" {} ", provider_name), setup_muted_style()),
                ]),
                setup_heading(
                    vec![Span::styled(
                        format!("{} models", models.len()),
                        setup_accent_style(),
                    )],
                    "Enter selects",
                    inner.width,
                ),
                &legend,
                Some(setup_band(&band_text, inner.width)),
                rows,
                (*selected).saturating_sub(start),
                None,
            );
        }
        ProviderSetupStep::SelectAzureResource {
            entries,
            selected,
            error,
        } => {
            let block = setup_block(Line::from(Span::styled(
                " Select Azure Resource ",
                setup_accent_style(),
            )));
            let inner = block.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            if inner.width == 0 || inner.height == 0 {
                return;
            }

            let mut header: Vec<Line<'static>> = vec![
                Line::from(Span::styled("Select Azure Resource", setup_accent_style())),
                Line::from(""),
            ];
            let mut rows: Vec<Line<'static>> = Vec::new();

            if let Some(err) = error {
                header.push(Line::from(Span::styled(
                    err.clone(),
                    Style::default().fg(Color::Yellow),
                )));
                header.push(Line::from(""));
            } else if entries.is_empty() {
                header.push(Line::from(Span::styled(
                    "No resources found.",
                    Style::default().fg(Color::Yellow),
                )));
                header.push(Line::from(""));
                header.push(Line::from(Span::styled(
                    "Place an azureresources.json file in ~/.config/ragent/ or .ragent/",
                    setup_muted_style(),
                )));
            } else {
                for (i, entry) in entries.iter().enumerate() {
                    let (indicator, style) = if i == *selected {
                        (
                            "> ",
                            Style::default()
                                .fg(Color::Black)
                                .bg(SETUP_ACCENT)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        ("  ", Style::default().fg(Color::White))
                    };
                    rows.push(Line::from(vec![
                        Span::styled(indicator, setup_accent_style()),
                        Span::styled(entry.name.clone(), style),
                    ]));
                }
            }

            // One blank row plus the key-hint footer are pinned below the
            // scrolling window, so a long resource list scrolls under a fixed
            // legend instead of clipping the cursor off the bottom edge.
            let reserved = 2usize;
            let body = (inner.height as usize).saturating_sub(reserved);
            let visible = body.saturating_sub(header.len()).max(1);
            let start = setup_window_start(*selected, visible, rows.len());
            let end = (start + visible).min(rows.len());

            let mut lines = header;
            lines.extend(rows[start..end].iter().cloned());
            lines.push(Line::from(""));

            let mut legend = String::new();
            if rows.len() > visible {
                legend.push_str(&format!(
                    "Showing {}-{} of {}  |  ",
                    start + 1,
                    end,
                    rows.len()
                ));
            }
            legend.push_str("^/v to move, Enter to select, Esc to cancel");
            lines.push(Line::from(Span::styled(legend, setup_muted_style())));

            let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
            frame.render_widget(paragraph, inner);
        }
        ProviderSetupStep::SelectThinkingLevel {
            model, selected, ..
        } => {
            // Each level is a cursor row carrying its name plus a muted one-line
            // description, so the tradeoff between depths is readable without
            // opening a help page.
            let level_lines: Vec<Vec<Line<'static>>> = model
                .thinking_levels
                .iter()
                .enumerate()
                .map(|(i, level)| {
                    let selected_row = i == *selected;
                    let cursor_style = Style::default()
                        .fg(Color::Black)
                        .bg(SETUP_ACCENT)
                        .add_modifier(Modifier::BOLD);
                    let name_style = if selected_row {
                        cursor_style
                    } else {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    };
                    let label = match level {
                        ThinkingLevel::Auto => "Auto",
                        ThinkingLevel::Off => "Off",
                        ThinkingLevel::Low => "Low",
                        ThinkingLevel::Medium => "Medium",
                        ThinkingLevel::High => "High",
                    };
                    let desc = match level {
                        ThinkingLevel::Auto => "Use the model default reasoning depth",
                        ThinkingLevel::Off => "Disable reasoning / thinking",
                        ThinkingLevel::Low => "Use a light reasoning budget",
                        ThinkingLevel::Medium => "Use a balanced reasoning budget",
                        ThinkingLevel::High => "Use the deepest reasoning budget",
                    };
                    vec![
                        Line::from(Span::styled(format!(" {label}"), name_style)),
                        Line::from(Span::styled(format!("   {}", desc), setup_muted_style())),
                    ]
                })
                .collect();

            let block = setup_block(Line::from(vec![
                Span::styled(" Select Thinking Level ", setup_accent_style()),
                Span::styled(format!(" {} ", model.name), setup_muted_style()),
            ]));
            let inner = block.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            if inner.width == 0 || inner.height == 0 {
                return;
            }

            // The heading, the blank spacer and the key-hint footer are pinned;
            // only the level rows scroll, each two lines tall, so a custom level
            // set longer than the dialog keeps its cursor on screen.
            let reserved = 4usize;
            let body = (inner.height as usize).saturating_sub(reserved);
            let visible_rows = (body / 2).max(1);
            let start = setup_window_start(*selected, visible_rows, level_lines.len());
            let end = (start + visible_rows).min(level_lines.len());

            let mut content: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    format!("Select thinking for {}", model.name),
                    setup_accent_style(),
                )),
                Line::from(""),
            ];
            for level in &level_lines[start..end] {
                content.extend(level.iter().cloned());
            }
            content.push(Line::from(""));

            let mut legend = String::new();
            if level_lines.len() > visible_rows {
                legend.push_str(&format!(
                    "Showing {}-{} of {}  |  ",
                    start + 1,
                    end,
                    level_lines.len()
                ));
            }
            legend.push_str("^/v navigate  Enter select  Esc cancel");
            content.push(Line::from(Span::styled(legend, setup_muted_style())));

            frame.render_widget(Paragraph::new(content).alignment(Alignment::Left), inner);
        }
        ProviderSetupStep::Done {
            provider_name,
            model_name,
        } => {
            // Success reads as a single confirmation card: a green `[ok]` headline,
            // the provider and its model named plainly, and one muted prompt.
            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "[ok] Provider Configured",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(provider_name.clone(), setup_accent_style()),
                    Span::styled(" is now ready to use.", Style::default().fg(Color::White)),
                ]),
            ];

            if let Some(model) = model_name {
                lines.push(Line::from(vec![
                    Span::styled("Model: ", setup_muted_style()),
                    Span::styled(model.clone(), Style::default().fg(Color::White)),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press any key to continue",
                setup_muted_style(),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " Success ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Green));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectAgent { agents, selected } => {
            // Agent rows: the name leads in white with the cursor bar on the selected
            // row, `[custom]` marks a project/user-defined agent, `*` marks the agent
            // currently in use, and the description trails in the muted tone.
            let inner_width = area.width.saturating_sub(2);
            let rows: Vec<Line<'static>> = agents
                .iter()
                .enumerate()
                .map(|(i, (name, desc, is_custom))| {
                    let mut spans = vec![Span::styled(
                        name.clone(),
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )];
                    if *is_custom {
                        spans.push(Span::styled(
                            " [custom]",
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if i == app.current_agent_index {
                        spans.push(Span::styled(
                            " *",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    spans.push(Span::styled(format!("  {desc}"), setup_muted_style()));
                    setup_pad_line(Line::from(spans), inner_width)
                })
                .collect();

            render_setup_list(
                frame,
                area,
                Line::from(Span::styled(" Select Agent ", setup_accent_style())),
                setup_heading(
                    vec![Span::styled("Select an Agent", setup_accent_style())],
                    &format!("{} available  * = in use", agents.len()),
                    inner_width,
                ),
                "^/v navigate  Enter select  Esc cancel",
                None,
                rows,
                *selected,
                None,
            );
        }
        // Renderer for the configured-provider picker (used by `/model`).
        ProviderSetupStep::SelectConfiguredProvider {
            providers,
            selected,
        } => {
            // Only configured providers are listed, so every row carries `[ok]`; the
            // key source is named in the muted trailing lane so it is clear where the
            // credential came from.
            let inner_width = area.width.saturating_sub(2);
            let rows: Vec<Line<'static>> = providers
                .iter()
                .map(|prov| {
                    let source = match prov.source {
                        crate::app::ProviderSource::EnvVar => "env",
                        crate::app::ProviderSource::Database => "stored",
                        crate::app::ProviderSource::AutoDiscovered => "auto",
                    };
                    let line = Line::from(vec![
                        Span::styled(
                            prov.name.clone(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            " [ok]",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {source}"), setup_muted_style()),
                    ]);
                    setup_pad_line(line, inner_width)
                })
                .collect();

            render_setup_list(
                frame,
                area,
                Line::from(Span::styled(" Switch Provider ", setup_accent_style())),
                setup_heading(
                    vec![Span::styled("Switch Provider", setup_accent_style())],
                    &format!("{} configured", providers.len()),
                    inner_width,
                ),
                "^/v navigate  Enter select  Esc cancel",
                None,
                rows,
                *selected,
                None,
            );
        }
        ProviderSetupStep::ShowProviderConfig {
            providers,
            selected,
        } => {
            // The config view lists the same providers without the credential lanes: it
            // is a browse-only step, so the muted id names the provider behind the label.
            let inner_width = area.width.saturating_sub(2);
            let rows: Vec<Line<'static>> = providers
                .iter()
                .map(|prov| {
                    let line = Line::from(vec![
                        Span::styled(
                            prov.name.clone(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(format!("  {}", prov.id), setup_muted_style()),
                    ]);
                    setup_pad_line(line, inner_width)
                })
                .collect();

            render_setup_list(
                frame,
                area,
                Line::from(Span::styled(" Provider Config ", setup_accent_style())),
                setup_heading(
                    vec![Span::styled(
                        "Show Provider Configuration",
                        setup_accent_style(),
                    )],
                    &format!("{} configured", providers.len()),
                    inner_width,
                ),
                "^/v navigate  Enter show config  Esc cancel",
                None,
                rows,
                *selected,
                None,
            );
        }
        ProviderSetupStep::ResetProvider { selected } => {
            // Destructive step: the frame, the cursor and the heading stay yellow so the
            // dialog reads as a warning rather than a neutral picker, while the row
            // layout (name / kind / active lanes, zebra, cursor bar) matches the other
            // setup lists.
            let active_id = app.configured_provider.as_ref().map(|p| p.id.as_str());
            let danger = Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD);
            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Provider Reset ", danger))
                .border_style(Style::default().fg(Color::Yellow));
            let inner = block.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            if inner.width == 0 || inner.height == 0 {
                return;
            }

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .split(inner);
            frame.render_widget(
                Paragraph::new(setup_heading(
                    vec![Span::styled("Reset Provider Credentials", danger)],
                    "* = active provider",
                    inner.width,
                )),
                chunks[0],
            );
            let items: Vec<ListItem> = PROVIDER_LIST
                .iter()
                .enumerate()
                .map(|(index, (pid, pname))| {
                    let active_marker = if active_id == Some(*pid) { "*" } else { "" };
                    let badge = if *pid == "ollama" { "[local]" } else { "" };
                    let line = Line::from(vec![
                        Span::styled(
                            pname.to_string(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {:<PROVIDER_BADGE_WIDTH$} ", badge),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            format!("{active_marker:<PROVIDER_STATUS_WIDTH$}"),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]);
                    let line = setup_pad_line(line, inner.width);
                    let item = ListItem::new(line);
                    if index % 2 == 1 {
                        item.style(Style::default().bg(SETUP_ZEBRA_BG))
                    } else {
                        item
                    }
                })
                .collect();
            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some((*selected).min(items.len() - 1)));
            }
            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("");
            frame.render_stateful_widget(list, chunks[1], &mut state);
            render_setup_hint(frame, chunks[2], "^/v navigate  Enter reset  Esc cancel");
        }

        // -- GitLab setup form --------------------------------------------
        ProviderSetupStep::GitLabSetup {
            url_input,
            url_cursor,
            token_input,
            token_cursor,
            active_field,
            error,
        } => {
            // Wider dialog so the full token (>= 48 chars) is visible.
            let area = centered_rect_max(80, 80, 100, 30, frame.area());
            app.provider_setup_area = area;
            frame.render_widget(Clear, area);

            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Configure GitLab",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Instance URL:"),
            ];

            // URL field
            let url_cursor_display = if *active_field == 0 {
                *url_cursor
            } else {
                url_input.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 0 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    if url_input.is_empty() {
                        "https://gitlab.com".to_string()
                    } else {
                        with_cursor_marker(url_input, url_cursor_display)
                    },
                    Style::default().fg(if url_input.is_empty() {
                        Color::DarkGray
                    } else {
                        Color::White
                    }),
                ),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from("Personal Access Token:"));

            // SEC-ragent-tui-002 (SECTASKS T-045): mask the GitLab PAT.
            let token_masked = mask_secret(token_input);
            let tok_cursor_display = if *active_field == 1 {
                *token_cursor
            } else {
                token_masked.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 1 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(&token_masked, tok_cursor_display),
                    Style::default().fg(Color::White),
                ),
            ]));

            if let Some(err) = error {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    err.as_str(),
                    Style::default().fg(Color::Red),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Tab switch fields  Enter validate & save  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" GitLab Setup ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            protocol,
            interval_field,
            timeout_field,
            port_field,
            active_field,
            error,
        } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Configure Telemetry",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            let fields: [(&str, &crate::input_field::InputField, u8, &str); 4] = [
                ("OTLP endpoint:", endpoint_field, 0, "http://localhost:4318"),
                ("Export interval (s):", interval_field, 2, "30"),
                ("Export timeout (s):", timeout_field, 3, "10"),
                ("Internal Prometheus port:", port_field, 4, "disabled"),
            ];

            for (label, field, idx, placeholder) in fields {
                let is_active = *active_field == idx;
                lines.push(Line::from(label.to_string()));
                let display = if field.text().is_empty() {
                    placeholder.to_string()
                } else if is_active {
                    with_cursor_marker(field.text(), field.cursor())
                } else {
                    field.text().to_string()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        if is_active { "> " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        display,
                        Style::default().fg(if field.text().is_empty() {
                            Color::DarkGray
                        } else {
                            Color::White
                        }),
                    ),
                ]));
                lines.push(Line::from(""));
            }

            // Protocol row is always visible and togglable.
            let proto_active = *active_field == 1;
            lines.push(Line::from("Transport protocol:"));
            lines.push(Line::from(vec![
                Span::styled(
                    if proto_active { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!(
                        "[{}]",
                        match protocol {
                            ragent_config::OtelProtocol::Http => "http",
                            ragent_config::OtelProtocol::Grpc => "grpc",
                        }
                    ),
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(""));

            if let Some(err) = error {
                lines.push(Line::from(Span::styled(
                    err.as_str(),
                    Style::default().fg(Color::Red),
                )));
                lines.push(Line::from(""));
            }

            lines.push(Line::from(Span::styled(
                "Tab switch fields  Up/Down toggle protocol  Enter save  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Telemetry Setup ")
                .border_style(Style::default().fg(Color::Cyan));
            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }

        ProviderSetupStep::GitLabValidating { .. } => {
            let lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Configure GitLab",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Validating token...",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" GitLab Setup ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SetupRouter {
            providers,
            selected_provider_ids,
            selected_provider_index,
            draft_config,
            active_bucket,
            active_bucket_index,
            left_pane_focused,
            error,
        } => {
            let area = centered_rect(80, 80, frame.area());
            app.provider_setup_area = area;
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Model Router Setup ")
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            // Split into left (providers) and right (buckets) panes.
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .margin(1)
                .split(inner);

            // -- Left pane: multi-select provider list --
            //
            // The list is windowed around the cursor: a cluster palette longer
            // than the pane scrolls one line at a time so the highlighted
            // provider is always painted, and the block title states the window
            // position whenever the list is clipped.
            let left_inner_height = chunks[0].height.saturating_sub(2) as usize;
            let mut provider_lines: Vec<Line<'_>> = Vec::new();
            let mut left_start = 0usize;
            if providers.is_empty() {
                provider_lines.push(Line::from(Span::styled(
                    "No concrete providers configured.",
                    Style::default().fg(Color::Yellow),
                )));
                provider_lines.push(Line::from(""));
                provider_lines.push(Line::from(Span::styled(
                    "Set up a provider first, then return to /provider.",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                let rows = providers.len();
                left_start = setup_window_start(*selected_provider_index, left_inner_height, rows);
                let visible = left_inner_height.max(1).min(rows);
                let end = (left_start + visible).min(rows);
                for (i, p) in providers[left_start..end].iter().enumerate() {
                    let absolute = left_start + i;
                    let is_selected = absolute == *selected_provider_index;
                    let in_cluster = selected_provider_ids.contains(&p.id);
                    let tick = if in_cluster { "[x]" } else { "[ ]" };
                    let (indicator, style) = if is_selected && *left_pane_focused {
                        (
                            "> ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        ("  ", Style::default().fg(Color::White))
                    };
                    let tick_style = if in_cluster {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    provider_lines.push(Line::from(vec![
                        Span::styled(indicator, style),
                        Span::styled(format!("{} ", tick), tick_style),
                        Span::styled(p.name.clone(), style),
                    ]));
                }
            }
            let left_title = if providers.len() > left_inner_height && !providers.is_empty() {
                let visible = left_inner_height.max(1);
                format!(
                    " Providers{} {}-{} of {} ",
                    if *left_pane_focused { " (*)" } else { "" },
                    left_start + 1,
                    (left_start + visible).min(providers.len()),
                    providers.len()
                )
            } else if *left_pane_focused {
                " Providers (*) ".to_string()
            } else {
                " Providers ".to_string()
            };
            let left_block = Block::default()
                .borders(Borders::ALL)
                .title(left_title)
                .border_style(if *left_pane_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                });
            let left_para = Paragraph::new(provider_lines)
                .block(left_block)
                .wrap(Wrap { trim: false });
            frame.render_widget(left_para, chunks[0]);

            // -- Right pane: four tier buckets in a 2x2 grid --
            let right_block = Block::default()
                .borders(Borders::ALL)
                .title(if *left_pane_focused {
                    " Tiers "
                } else {
                    " Tiers (*) "
                })
                .border_style(if *left_pane_focused {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Cyan)
                });
            let right_inner = right_block.inner(chunks[1]);
            frame.render_widget(right_block.clone(), chunks[1]);

            // Arrange the four tier buckets as 2 rows of 2 columns so the full
            // tier name and model properties remain readable on narrower
            // terminals. Tiers are laid out in ascending complexity order:
            //   row 0: SIMPLE   | MEDIUM
            //   row 1: COMPLEX   | REASONING
            let row_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(right_inner);
            let bucket_chunks: Vec<Rect> = row_chunks
                .iter()
                .flat_map(|row| {
                    Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(*row)
                        .to_vec()
                })
                .collect();

            for (idx, tier) in ragent_llm::providers::router_config::Tier::all()
                .iter()
                .enumerate()
            {
                let is_active = *active_bucket == *tier;
                let tier_config = draft_config.tiers.get(&tier.to_string());
                let models = tier_config.map(|t| t.models.as_slice()).unwrap_or(&[]);

                // Use the full tier name (e.g. "SIMPLE") in the bucket title so
                // each bucket is unambiguous at a glance.
                let bucket_block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " {}{} ",
                        tier,
                        if is_active && !left_pane_focused {
                            " *"
                        } else {
                            ""
                        }
                    ))
                    .border_style(if is_active && !left_pane_focused {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    });
                let bucket_inner = bucket_block.inner(bucket_chunks[idx]);
                let bucket_height = bucket_inner.height as usize;

                // Each assigned model paints two or three lines (name, retained
                // properties, cost estimate). The span of the selected model is
                // recorded so the pane can scroll to keep it wholly visible: a
                // bucket holding more models than rows then follows the cursor
                // instead of clipping its tail off the bottom edge.
                let mut entry_spans: Vec<(usize, usize)> = Vec::new();
                let mut bucket_lines: Vec<Line<'static>> = Vec::new();
                if models.is_empty() {
                    bucket_lines.push(Line::from(Span::styled(
                        "empty",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (midx, entry) in models.iter().enumerate() {
                        let span_start = bucket_lines.len();
                        let is_selected =
                            midx == *active_bucket_index && is_active && !left_pane_focused;
                        let style = if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };

                        // Resolve the full model metadata so its properties are
                        // retained and displayed inside the bucket. Falls back to
                        // a minimal label when the model is not advertised by the
                        // provider registry (e.g. a stale assignment).
                        let metadata = app.router_model_picker_entry(&entry.provider, &entry.model);

                        // Line 1: provider / model name.
                        let model_label = metadata
                            .as_ref()
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| entry.model.clone());
                        bucket_lines.push(Line::from(vec![
                            Span::styled(format!("{}", entry.provider), style),
                            Span::styled(
                                format!(" / {} ", model_label),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));

                        // Line 2: retained model properties - context window,
                        // features, thinking levels, and cost tier.
                        if let Some(m) = &metadata {
                            let ctx_str = if m.context_window >= 1_000_000 {
                                format!("{}M", m.context_window / 1_000_000)
                            } else if m.context_window >= 1_000 {
                                format!("{}K", m.context_window / 1_000)
                            } else {
                                m.context_window.to_string()
                            };
                            let mut features: Vec<&'static str> = Vec::new();
                            if m.reasoning {
                                features.push("R");
                            }
                            if m.vision {
                                features.push("V");
                            }
                            if m.tool_use {
                                features.push("T");
                            }
                            let features_str = if features.is_empty() {
                                "-".to_string()
                            } else {
                                features.join(",")
                            };
                            let thinking_str = App::format_thinking_levels(&m.thinking_levels);
                            let props_style = Style::default().fg(Color::DarkGray);
                            bucket_lines.push(Line::from(vec![
                                Span::styled("   ", props_style),
                                Span::styled(format!("ctx {} ", ctx_str), props_style),
                                Span::styled(format!("feat {} ", features_str), props_style),
                                Span::styled(format!("think {} ", thinking_str), props_style),
                                Span::styled(
                                    format!("cost {}*{}", m.cost_tier, m.cost_multiplier),
                                    props_style,
                                ),
                            ]));
                        }

                        // Line 3: cost estimate from the provider registry, when
                        // available (FR-022).
                        let cost = app
                            .estimate_entry_cost(&entry.provider, &entry.model)
                            .unwrap_or_default();
                        if !cost.is_empty() {
                            bucket_lines.push(Line::from(Span::styled(
                                format!("   {}", cost),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                        entry_spans.push((span_start, bucket_lines.len()));
                    }
                }
                let (span_start, span_end) = entry_spans
                    .get((*active_bucket_index).min(entry_spans.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or((0, 0));
                let scroll = if is_active && !left_pane_focused {
                    setup_scroll_to_show(span_start, span_end, bucket_height)
                } else {
                    0
                };
                let bucket_para = Paragraph::new(bucket_lines)
                    .block(bucket_block)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll as u16, 0));
                frame.render_widget(bucket_para, bucket_chunks[idx]);
            }

            // Footer with hints/error.
            let footer_text = if let Some(err) = error {
                format!(
                    "Esc cancel | Tab switch pane | ^v move | Space toggle | Enter assign | Ctrl+S save | Ctrl+^v reorder | Del remove - Error: {err}"
                )
            } else {
                "Esc cancel | Tab switch pane | ^v move | Space toggle provider | Enter assign | Ctrl+S save | Ctrl+^v reorder | Del remove".to_string()
            };
            let footer = Paragraph::new(Line::from(Span::styled(
                footer_text,
                Style::default().fg(Color::DarkGray),
            )))
            .alignment(Alignment::Center);
            let footer_height = 1u16;
            if area.height > footer_height + 2 {
                let footer_area = Rect::new(
                    area.x,
                    area.y + area.height - footer_height - 1,
                    area.width,
                    footer_height,
                );
                frame.render_widget(footer, footer_area);
            }
        }
        ProviderSetupStep::SelectRouterModel {
            provider_id: _,
            provider_name,
            models,
            selected,
            target_tier,
        } => {
            // Content-sized dialog: when models exist the dialog is just wide
            // enough to hold the widest row (clamped to the terminal); the
            // empty-model notice keeps the shared default width.
            let area = if models.is_empty() {
                centered_rect(72, 78, frame.area())
            } else {
                let cells = model_picker_cells(models, *selected);
                let inner_w = model_picker_inner_width(&cells) as u16;
                centered_rect_max(100, 78, inner_w.saturating_add(2), 30, frame.area())
            };
            app.provider_setup_area = area;
            frame.render_widget(Clear, area);

            if models.is_empty() {
                let block = setup_block(Line::from(vec![
                    Span::styled(" Assign model for ", setup_accent_style()),
                    Span::styled(provider_name.clone(), Style::default().fg(Color::White)),
                    Span::styled(" -> ", setup_muted_style()),
                    Span::styled(target_tier.to_string(), setup_accent_style()),
                    Span::raw(" "),
                ]));
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let paragraph = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No models are currently available for this provider.",
                        Style::default().fg(Color::Yellow),
                    )),
                    Line::from(""),
                    Line::from(Span::styled("Esc cancel", setup_muted_style())),
                ])
                .alignment(Alignment::Center);
                frame.render_widget(paragraph, inner);
            } else {
                let inner = setup_block(Line::default()).inner(area);
                // The same rich model-properties table the standard model picker uses,
                // so context window, cost, thinking levels and feature flags can be
                // compared before a model is assigned to a router tier bucket.
                let cells = model_picker_cells(models, *selected);
                let columns = content_sized_columns(&cells);
                let widths: Vec<usize> = columns
                    .iter()
                    .map(|c| match c {
                        Constraint::Length(w) | Constraint::Min(w) => *w as usize,
                        _ => 0,
                    })
                    .collect();

                let band_text = model_picker_heading_band(&widths);

                let list_rows = inner.height.saturating_sub(3) as usize;
                let visible = list_rows.max(1).min(models.len());
                let start = setup_window_start(*selected, visible, models.len());
                let end = (start + visible).min(models.len());

                let rows: Vec<Line<'static>> = cells
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(end - start)
                    .map(|(_, row_cells)| {
                        let mut spans: Vec<Span<'static>> = Vec::with_capacity(row_cells.len());
                        for (col, value) in row_cells.iter().enumerate() {
                            let width = widths
                                .get(col)
                                .copied()
                                .unwrap_or_else(|| value.chars().count());
                            let field = if col == 1 || col == 2 {
                                format!("{value:>width$}")
                            } else {
                                format!("{value:<width$}")
                            };
                            let style = if col == 0 {
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                setup_muted_style()
                            };
                            spans.push(Span::styled(field, style));
                            if col + 1 < row_cells.len() {
                                spans.push(Span::raw(" ".repeat(MODEL_PICKER_COLUMN_SPACING)));
                            }
                        }
                        setup_pad_line(Line::from(spans), inner.width)
                    })
                    .collect();

                let mut legend = String::new();
                if models.len() > visible {
                    legend.push_str(&format!(
                        "Showing {}-{} of {}  |  ",
                        start + 1,
                        end,
                        models.len()
                    ));
                }
                legend.push_str("^/v navigate  Enter assign  Esc cancel");
                legend.push_str("  |  R reasoning  V vision  T tools");

                render_setup_list(
                    frame,
                    area,
                    Line::from(vec![
                        Span::styled(" Assign model for ", setup_accent_style()),
                        Span::styled(provider_name.clone(), Style::default().fg(Color::White)),
                        Span::styled(" -> ", setup_muted_style()),
                        Span::styled(target_tier.to_string(), setup_accent_style()),
                        Span::raw(" "),
                    ]),
                    setup_heading(
                        vec![Span::styled(
                            format!("{} models", models.len()),
                            setup_accent_style(),
                        )],
                        "Enter assigns to the tier",
                        inner.width,
                    ),
                    &legend,
                    Some(setup_band(&band_text, inner.width)),
                    rows,
                    (*selected).saturating_sub(start),
                    None,
                );
            }
        }
    }
}

/// Render the slash-command autocomplete dropdown above the given input area.
fn render_slash_menu(frame: &mut Frame, app: &App, input_area: Rect) {
    let menu = match &app.slash_menu {
        Some(m) => m,
        None => return,
    };

    if menu.matches.is_empty() {
        return;
    }

    let total = menu.matches.len() as u16;
    let width = input_area.width.min(50);
    // Available space above the input (minus 2 for borders).
    let max_visible = input_area.y.saturating_sub(2);
    // Visible rows: as many entries as fit, capped by total.
    let visible_rows = total.min(max_visible.max(1));
    let height = visible_rows + 2; // +2 for borders

    // Compute scroll offset so the selected row is always in view.
    let sel = menu.selected as u16;
    let scroll_offset = if sel < visible_rows {
        0
    } else {
        sel - visible_rows + 1
    };

    let popup = Rect::new(
        input_area.x,
        input_area.y.saturating_sub(height),
        width,
        height,
    );

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    for (i, entry) in menu
        .matches
        .iter()
        .enumerate()
        .skip(scroll_offset as usize)
        .take(visible_rows as usize)
    {
        let is_selected = i == menu.selected;
        let (indicator, name_style, desc_style) = if is_selected {
            (
                "> ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            )
        } else {
            (
                "  ",
                Style::default().fg(if entry.is_skill {
                    Color::Yellow
                } else {
                    Color::White
                }),
                Style::default().fg(Color::DarkGray),
            )
        };
        lines.push(Line::from(vec![
            Span::styled(indicator, name_style),
            Span::styled(format!("/{}", entry.trigger), name_style),
            Span::styled(format!("  {}", entry.description), desc_style),
        ]));
    }

    // Scroll indicator in border title when list is scrolled.
    let title = if total > visible_rows {
        format!(" {}/{} ", menu.selected + 1, menu.matches.len())
    } else {
        String::new()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(title, Style::default().fg(Color::DarkGray)));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup);
}

/// Render the `@` file reference autocomplete popup above the input area.
fn render_file_menu(frame: &mut Frame, app: &App, input_area: Rect) {
    let menu = match &app.file_menu {
        Some(m) => m,
        None => return,
    };

    let hint_row_count: u16 = 1;
    let max_visible_rows: u16 = 8;
    let item_count = menu.matches.len() as u16;
    let visible_items = item_count.max(1).min(max_visible_rows);
    let height = (visible_items + hint_row_count + 2).min(input_area.y);
    let width = input_area.width.min(60);

    let popup = Rect::new(
        input_area.x,
        input_area.y.saturating_sub(height),
        width,
        height,
    );

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    if menu.matches.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  No matches",
            Style::default().fg(Color::DarkGray),
        )]));
    } else {
        let start = menu.scroll_offset.min(menu.matches.len().saturating_sub(1));
        let end = (start + visible_items as usize).min(menu.matches.len());
        for (i, entry) in menu.matches[start..end].iter().enumerate() {
            let absolute_i = start + i;
            let is_selected = absolute_i == menu.selected;
            let (indicator, path_style) = if is_selected {
                (
                    "> ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else if entry.is_dir {
                ("  ", Style::default().fg(Color::Blue))
            } else {
                ("  ", Style::default().fg(Color::White))
            };
            let icon = if entry.is_dir { "[dir] " } else { "[file] " };
            let display = shorten_middle(&entry.display, width.saturating_sub(8) as usize);
            lines.push(Line::from(vec![
                Span::styled(indicator, path_style),
                Span::raw(icon),
                Span::styled(display, path_style),
            ]));
        }
    }

    lines.push(Line::from(vec![Span::styled(
        "  Enter/Tab accept  Esc close  Ctrl+\\ hidden",
        Style::default().fg(Color::DarkGray),
    )]));

    let title = if let Some(ref dir) = menu.current_dir {
        let hidden = if app.file_menu_show_hidden {
            " hidden:on"
        } else {
            ""
        };
        format!(
            " @{}/ [{}/{}]{} ",
            dir.to_string_lossy(),
            menu.selected.saturating_add(1).min(menu.matches.len()),
            menu.matches.len(),
            hidden
        )
    } else {
        format!(
            " @{} [{}/{}] ",
            menu.query,
            menu.selected.saturating_add(1).min(menu.matches.len()),
            menu.matches.len()
        )
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup);
}

/// Width in columns of the `"> "` prompt when no queue counter is shown.
const PROMPT_WIDTH: usize = 2;
/// Width in columns of the two-digit, zero-padded queue counter.
const COUNTER_WIDTH: usize = 2;

/// Build the chat input's prompt prefix string.
///
/// The prompt is `"> "` while the input queue is empty and `"NN> "` (a
/// two-digit, zero-padded counter immediately before `>`) while entries are
/// pending (spec `inputqueue` FR-009, FR-010). The counter is clamped to `99`
/// so the prefix never widens beyond four columns (NFR-002).
fn input_prompt_prefix(queue_len: usize) -> String {
    if queue_len == 0 {
        "> ".to_string()
    } else {
        format!("{:02}> ", queue_len.min(99))
    }
}

/// Width in columns of the chat input's prompt prefix for `queue_len`.
///
/// Mirrors [`input_prompt_prefix`] without allocating.  Every geometry
/// consumer (wrapped rows, cursor position, selection mapping) derives its
/// prefix width from this so the cursor column and wrapped-row geometry stay
/// aligned to the counter prefix (spec `inputqueue` NFR-002, FR-020).
pub(crate) fn input_prompt_prefix_len(queue_len: usize) -> usize {
    if queue_len == 0 {
        PROMPT_WIDTH
    } else {
        PROMPT_WIDTH + COUNTER_WIDTH
    }
}

/// Selection-safe variant of [`input_prompt_prefix`].
///
/// Identical in width to the painted prompt prefix so mouse and keyboard
/// selection columns line up with the rendered frame (NFR-002), but the
/// counter digits are replaced by spaces so the queue counter can never be
/// copied or cut as message content (FR-020).
pub(crate) fn input_prompt_selection_prefix(queue_len: usize) -> String {
    if queue_len == 0 {
        "> ".to_string()
    } else {
        format!("{}> ", " ".repeat(COUNTER_WIDTH))
    }
}

/// Height of the chat input widget at `inner_width`, borders included.
///
/// PERF-046: the input is character-wrapped (fixed width, no word breaks), so
/// each logical line occupies `prefix_len + chars` cells of content.  Summing
/// the per-line row counts is equivalent to counting wrapped rows but performs
/// no allocation.  `prefix_len` is the prompt prefix width, which grows from
/// two columns to four while the queue counter is shown (FR-009, NFR-002).
pub(crate) fn input_widget_height(input: &str, inner_width: usize, prefix_len: usize) -> u16 {
    let width = inner_width.max(1);
    let rows: usize = input
        .split('\n')
        .map(|logical_line| {
            (prefix_len + logical_line.chars().count())
                .div_ceil(width)
                .max(1)
        })
        .sum();
    (rows.max(1) as u16) + 2 // +2 for borders
}

/// Build wrapped content lines from `Line`s that matches Paragraph word-wrapping.
///
/// This produces the same line breaks as ratatui's `Paragraph::wrap(Wrap { trim: false })`
/// so that mouse selection coordinates map correctly to content lines.
///
/// The second return vector contains, for each input `Line`, the index of its
/// first wrapped line in the returned content. This lets callers translate
/// unwrapped line numbers (e.g. the indices stored for the Memory panel cursor)
/// into wrapped line coordinates.
fn build_wrapped_content_lines_with_starts(
    lines: &[Line<'_>],
    inner_width: usize,
) -> (Vec<String>, Vec<usize>) {
    let mut result = Vec::new();
    let mut starts = Vec::with_capacity(lines.len());
    for line in lines {
        starts.push(result.len());
        let text = line
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        if text.is_empty() {
            result.push(String::new());
            continue;
        }
        // Word-wrap: split at word boundaries, breaking long words
        let mut line_start = 0usize;
        let chars: Vec<char> = text.chars().collect();
        while line_start < chars.len() {
            let remaining = chars.len() - line_start;
            if remaining <= inner_width {
                result.push(chars[line_start..].iter().collect::<String>());
                break;
            }
            // Find the best break point: try to break at whitespace
            let search_end = (line_start + inner_width).min(chars.len());
            let mut break_pos = search_end;
            // Look backwards for a whitespace character to break at
            for i in (line_start..search_end).rev() {
                if chars[i].is_whitespace() {
                    break_pos = i + 1; // Include the space at end of line
                    break;
                }
            }
            // If no whitespace found, hard break at width
            if break_pos == search_end && break_pos == line_start + inner_width {
                // No whitespace in this chunk, hard break
                break_pos = line_start + inner_width;
            }
            // If break_pos didn't move (no whitespace and we're at start), just take width chars
            if break_pos <= line_start {
                break_pos = (line_start + inner_width).min(chars.len());
            }
            result.push(chars[line_start..break_pos].iter().collect::<String>());
            // Advance past any whitespace at the start of next line (like Paragraph does with trim=false)
            line_start = break_pos;
        }
    }
    (result, starts)
}

/// Convenience wrapper that returns only the wrapped text lines.
fn build_wrapped_content_lines(lines: &[Line<'_>], inner_width: usize) -> Vec<String> {
    build_wrapped_content_lines_with_starts(lines, inner_width).0
}

/// Apply a background highlight to every span in a line.
///
/// Used to render the Alt+M panel's block cursor on the selected memory row
/// without losing the per-span foreground colours.
fn highlight_line(line: Line<'_>, bg: Color) -> Line<'_> {
    let highlighted_spans: Vec<Span> = line
        .spans
        .into_iter()
        .map(|span| {
            let style = span.style.bg(bg);
            Span::styled(span.content.to_string(), style)
        })
        .collect();
    Line::from(highlighted_spans).style(line.style)
}

fn input_cursor_display_pos(
    input: &str,
    cursor_chars: usize,
    inner_width: usize,
    prefix_len: usize,
) -> (usize, usize) {
    let inner_width = inner_width.max(1);
    let mut display_row = 0usize;
    let mut char_idx = 0usize;

    for (line_i, logical_line) in input.split('\n').enumerate() {
        let content_len = logical_line.chars().count();

        // Is the cursor within this logical line's content range (inclusive of end)?
        if char_idx <= cursor_chars && cursor_chars <= char_idx + content_len {
            let content_offset = cursor_chars - char_idx;
            let display_offset = prefix_len + content_offset;
            let row_within_line = display_offset / inner_width;
            let col = display_offset % inner_width;
            return (display_row + row_within_line, col);
        }

        // Advance: this logical line consumed content_len chars + 1 for the '\n'
        char_idx += content_len + 1;
        // Count wrapped display rows this logical line occupies
        let line_display_len = prefix_len + content_len;
        let wrapped_rows = line_display_len.div_ceil(inner_width).max(1);
        if line_i == 0 {
            display_row = wrapped_rows;
        } else {
            display_row += wrapped_rows;
        }
    }

    // Fallback: cursor at or past end of input
    (display_row, 0)
}

/// Render a credential field masked, keeping the first and last few characters.
///
/// SEC-ragent-tui-002 (SECTASKS T-045): API keys and PATs are shown in the
/// provider-setup dialog, which is visible during screen sharing or a terminal
/// recording. A short value is masked entirely; a longer one keeps its first 4
/// and last 4 characters so the operator can still confirm which key is
/// entered. Delegates to [`ragent_types::sanitize::mask_edges`].
#[must_use]
pub fn mask_secret(text: &str) -> String {
    ragent_types::sanitize::mask_edges(text, 4, 12)
}

fn with_cursor_marker(text: &str, cursor: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let c = cursor.min(chars.len());
    let mut out = String::with_capacity(text.len() + 3);
    out.extend(chars[..c].iter());
    out.push('█');
    out.extend(chars[c..].iter());
    out
}

/// PERF-046: rebuild the chat input's wrapped rows, height, and cursor
/// position, but only when one of their inputs changed.
///
/// Wrapped rows depend on `(input text, keyboard selection, inner width)`;
/// the cursor position depends on `(input text, cursor, inner width)`.  When
/// none of those changed the previous frame's values are reused, so an idle
/// or non-typing frame performs no wrapping or measurement work.
fn refresh_input_render_cache(app: &mut App, inner_width: u16) {
    let selection = app.kb_selection_char_range();
    let queue_len = app.input_queue_len();
    let prefix = input_prompt_prefix(queue_len);
    let prefix_len = input_prompt_prefix_len(queue_len);
    let height = input_widget_height(&app.input, inner_width as usize, prefix_len);
    let cache = &mut app.input_render_cache;
    let rows_current = cache.key == app.input
        && cache.selection == selection
        && cache.width == inner_width
        && cache.queue_len == queue_len
        && !cache.lines.is_empty();
    let cursor_current = cache.key == app.input
        && cache.cursor == app.input_cursor
        && cache.width == inner_width
        && cache.queue_len == queue_len
        && cache.height != 0;
    if rows_current && cursor_current {
        return;
    }
    if !rows_current {
        cache.lines =
            input_lines_with_kb_selection(&app.input, inner_width as usize, selection, &prefix);
        cache.selection = selection;
    }
    if !cursor_current {
        cache.cursor_pos = input_cursor_display_pos(
            &app.input,
            app.input_cursor,
            (inner_width as usize).max(1),
            prefix_len,
        );
        cache.cursor = app.input_cursor;
    }
    cache.height = height;
    cache.queue_len = queue_len;
    cache.key.clone_from(&app.input);
    cache.width = inner_width;
}

/// Wrap one rendered [`Line`] into styled display rows that match ratatui's
/// `Paragraph` word-wrapping (`Wrap { trim: false }`).
///
/// Rows are broken exactly where ratatui's `WordWrapper` (`ratatui-widgets`
/// `reflow.rs`, `WordWrapper::process_input`) breaks them, and each row keeps
/// the original span styles.  Because no row ever exceeds the target width,
/// re-wrapping a row inside `Paragraph` is a no-op: the cache geometry and the
/// painted output stay in one coordinate system.
fn wrap_line_styled(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    if width == 0 {
        return vec![unbounded_line(line)];
    }

    let max = width;
    // Flat stream of (grapheme symbol, patched style) mirroring
    // `Paragraph::styled_graphemes` (line style patched with span style).
    // ratatui 0.30 filters every grapheme that contains a control character
    // (`filter(|g| !g.contains(char::is_control))`), not just `"\n"` as 0.29
    // did, so tabs must be dropped here too or the port over-counts rows.
    let base = line.style;
    let graphemes: Vec<(String, ratatui::style::Style)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let span_style = base.patch(span.style);
            span.content
                .graphemes(true)
                .filter(|g| !g.contains(char::is_control))
                .map(move |g| (g.to_string(), span_style))
        })
        .collect();

    // Port of `WordWrapper::process_input` (trim = false).  State:
    // `row` is the display row being built, `word` the current word buffer,
    // `ws` the pending whitespace run.
    let mut rows: Vec<Vec<(String, ratatui::style::Style)>> = Vec::new();
    let mut row: Vec<(String, ratatui::style::Style)> = Vec::new();
    let mut row_width: usize = 0;
    let mut word: Vec<(String, ratatui::style::Style)> = Vec::new();
    let mut word_width: usize = 0;
    let mut ws: Vec<(String, ratatui::style::Style)> = Vec::new();
    let mut ws_width: usize = 0;
    let mut non_ws_previous = false;

    for (g, style) in graphemes {
        // ratatui treats NBSP as a regular (non-whitespace) grapheme
        // and ZWSP (zero-width space) as whitespace, so the port must agree
        // with `StyledGrapheme::is_whitespace` or the wrapped geometry
        // drifts from the painted output.
        let is_ws = (g.chars().all(char::is_whitespace) && g != "\u{00a0}") || g == "\u{200b}";
        // Symbols wider than the limit are ignored (as in WordWrapper).
        if g.width() > max {
            continue;
        }
        let gw = g.width();

        let word_found = non_ws_previous && is_ws;
        // The completed word (with its preceding whitespace run) would
        // overflow even on an otherwise empty row.
        let untrimmed_overflow = row.is_empty() && word_width + ws_width + gw > max;

        // Flush the completed word to the row (trim = false keeps the
        // leading whitespace run).
        if word_found || untrimmed_overflow {
            row.append(&mut ws);
            row_width += ws_width;
            row.append(&mut word);
            row_width += word_width;
            ws_width = 0;
            word_width = 0;
        }

        let row_full = row_width >= max;
        let pending_word_overflow = gw > 0 && row_width + ws_width + word_width >= max;

        if row_full || pending_word_overflow {
            let remaining = max.saturating_sub(row_width);
            rows.push(std::mem::take(&mut row));
            row_width = 0;

            // Drop whitespace up to the end of the emitted row.
            while let Some((front, _)) = ws.first() {
                let fw = front.width();
                if fw > remaining {
                    break;
                }
                ws_width -= fw;
                ws.remove(0);
            }

            // The wrapping whitespace symbol is consumed by the break.
            // (WordWrapper skips the counter update on this path too.)
            if is_ws && ws.is_empty() {
                continue;
            }
        }

        if is_ws {
            ws.push((g, style));
            ws_width += gw;
        } else {
            word.push((g, style));
            word_width += gw;
        }
        non_ws_previous = !is_ws;
    }

    // Tail: emit whatever is still buffered for this input line.  With
    // `trim = false` the pending whitespace run stays on the final row;
    // ratatui only emits a separate blank row for a whitespace-only input
    // when `trim` is set (`&& self.trim` in `WordWrapper::process_input`),
    // so the port must not add one here or the cache gains a phantom row.
    row.append(&mut ws);
    row.append(&mut word);
    if !row.is_empty() {
        rows.push(row);
    } else if rows.is_empty() {
        // Whitespace-independent blank input line: WordWrapper still emits a
        // single (blank) row so blank separator lines keep their height.
        rows.push(vec![]);
    }

    rows.into_iter().map(cells_to_line).collect()
}

/// Convert a wrapped row's grapheme/style cells back into one styled `Line`,
/// collapsing adjacent same-style graphemes into single spans.
fn cells_to_line(cells: Vec<(String, ratatui::style::Style)>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(cells.len());
    let mut text = String::new();
    let mut cur_style: Option<ratatui::style::Style> = None;
    for (g, st) in cells {
        if let Some(style) = cur_style.filter(|s| *s != st) {
            if !text.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut text), style));
            }
        }
        cur_style = Some(st);
        text.push_str(&g);
    }
    if !text.is_empty() {
        spans.push(Span::styled(text, cur_style.unwrap_or_default()));
    }
    Line::from(spans)
}

/// Re-anchor an un-wrapped [`Line`] as a `'static` line for the width == 0
/// degenerate case (no wrapping possible).
fn unbounded_line(line: &Line<'_>) -> Line<'static> {
    Line::from(
        line.spans
            .iter()
            .map(|s| Span::styled(s.content.to_string(), s.style))
            .collect::<Vec<_>>(),
    )
    .style(line.style)
}

/// Plain-text projection of pre-wrapped [`Line`]s for text-selection copy.
fn wrapped_lines_to_strings(wrapped: &[Line<'_>]) -> Vec<String> {
    wrapped
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

/// Slice a flat pre-wrapped line list to the visible window.
///
/// `scroll_from_top` is the offset into the wrapped-row coordinate space from
/// the oldest/top of the content; `visible` is the number of rows available on
/// screen. The result is suitable for passing to [`Paragraph::new`] without
/// `.wrap()` because every row is already wrapped to the inner width.
fn slice_flat_wrapped_window(
    lines: &[Line<'static>],
    scroll_from_top: u16,
    visible: u16,
) -> Vec<Line<'static>> {
    let mut window = Vec::with_capacity(visible as usize + 1);
    let window_start = scroll_from_top as usize;
    let window_end = window_start + visible.saturating_add(1) as usize;
    for (i, line) in lines.iter().enumerate() {
        if i < window_start {
            continue;
        }
        if i >= window_end {
            break;
        }
        window.push(line.clone());
    }
    window
}

/// Slice a grouped pre-wrapped line cache to the visible window.
///
/// Groups are used by the messages/log caches where each source item can span
/// multiple wrapped rows. The function walks groups in order, accumulating a
/// running wrapped-row count so the final slice is in the same coordinate space
/// as the scroll geometry.
fn slice_group_wrapped_window<G>(
    groups: &[G],
    scroll_from_top: u16,
    visible: u16,
    get_lines: impl Fn(&G) -> &[Line<'static>],
) -> Vec<Line<'static>> {
    let mut window = Vec::with_capacity(visible as usize + 1);
    let window_end = scroll_from_top.saturating_add(visible.saturating_add(1));
    let mut skipped: u16 = 0;
    'outer: for group in groups.iter() {
        let wrapped_lines = get_lines(group);
        let group_len = wrapped_lines.len() as u16;
        let group_start = skipped;
        let group_end = skipped.saturating_add(group_len);
        skipped = group_end;
        if group_end <= scroll_from_top || group_start >= window_end {
            continue;
        }
        let (start_in_group, end_in_group) = if group_start >= scroll_from_top {
            (0, group_len)
        } else {
            (scroll_from_top - group_start, group_len)
        };
        let end_in_group = end_in_group.min(window_end - group_start);
        for (li, line) in wrapped_lines.iter().enumerate() {
            let li = li as u16;
            if li < start_in_group {
                continue;
            }
            if li >= end_in_group {
                continue 'outer;
            }
            window.push(line.clone());
        }
    }
    window
}

fn input_lines_with_kb_selection(
    input: &str,
    inner_width: usize,
    selection: Option<(usize, usize)>,
    prefix: &str,
) -> Vec<ratatui::text::Line<'static>> {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};

    let inner_width = inner_width.max(1);
    let sel_style = Style::default().bg(Color::LightBlue).fg(Color::Black);

    // Build a flat list of (char, Option<char_index>) representing the full display
    // text. Prefix chars carry `None` (never selectable); content chars carry their
    // original index in `input`.
    let mut flat: Vec<(char, Option<usize>)> = Vec::new();
    let mut char_idx = 0usize;
    let logical_line_count = input.split('\n').count();
    // Continuation rows are indented under the first-row prompt so content stays
    // aligned (the counter prefix itself is only shown on the first row).
    let continuation_prefix: String = " ".repeat(prefix.chars().count());

    for (line_i, logical_line) in input.split('\n').enumerate() {
        // Each logical line starts a new display line - flush a boundary marker.
        // We represent this as a "newline flush" by letting the chunker know when
        // to start a new display row; we do this by resetting a counter below.
        // The first row shows the prompt (with the queue counter when present).
        let row_prefix = if line_i == 0 {
            prefix
        } else {
            continuation_prefix.as_str()
        };

        // Prefix chars: not selectable
        for c in row_prefix.chars() {
            flat.push((c, None));
        }

        // Content chars: carry original char_idx
        for c in logical_line.chars() {
            flat.push((c, Some(char_idx)));
            char_idx += 1;
        }

        // Account for the '\n' in char_idx (not added to flat)
        if line_i + 1 < logical_line_count {
            char_idx += 1;
        }

        // Mark end of this logical line so chunker starts a new display row.
        // We store a sentinel `('\0', None)` as a line-break marker.
        flat.push(('\0', None)); // sentinel: force display-row break
    }

    // Chunk `flat` into display lines of `inner_width`, breaking at sentinels.
    let mut display_lines: Vec<Vec<(char, Option<usize>)>> = Vec::new();
    let mut current: Vec<(char, Option<usize>)> = Vec::new();
    let mut col = 0usize;

    for (c, idx) in flat {
        if c == '\0' {
            // Logical line boundary - flush current display row.
            display_lines.push(std::mem::take(&mut current));
            col = 0;
            continue;
        }
        if col == inner_width {
            // Width wrap - flush and start new display row.
            display_lines.push(std::mem::take(&mut current));
            col = 0;
        }
        current.push((c, idx));
        col += 1;
    }
    if !current.is_empty() {
        display_lines.push(current);
    }
    if display_lines.is_empty() {
        display_lines.push(prefix.chars().map(|c| (c, None)).collect());
    }

    // Convert each display line into a ratatui Line with selection spans.
    display_lines
        .into_iter()
        .map(|chars| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut text = String::new();
            let mut in_sel = false;

            for (c, idx) in chars {
                let this_sel = idx.map_or(false, |ci| {
                    selection.map_or(false, |(s, e)| ci >= s && ci < e)
                });
                if this_sel != in_sel && !text.is_empty() {
                    let style = if in_sel { sel_style } else { Style::default() };
                    spans.push(Span::styled(std::mem::take(&mut text), style));
                }
                in_sel = this_sel;
                text.push(c);
            }
            if !text.is_empty() {
                let style = if in_sel { sel_style } else { Style::default() };
                spans.push(Span::styled(text, style));
            }
            Line::from(spans)
        })
        .collect()
}

const INPUT_PLACEHOLDER: &str =
    "Type @ to mention files, / for commands, ? for shortcuts, Alt+V to paste image";

fn render_router_save_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Save Router Configuration",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("This will overwrite your current Model Router cluster in ragent.json."),
        Line::from(""),
        Line::from(Span::styled(
            "Enter save  Esc cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let Some(ref draft) = app.pending_router_save {
        let total: usize = draft.tiers.values().map(|t| t.models.len()).sum();
        lines.insert(3, Line::from(format!("Tier entries to save: {total}")));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Save ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render the Telemetry side panel (toggled via `/telemetry_panel`).
///
/// Displays the same live counter/gauge values as `/telemetry counters`, grouped
/// by category in a compact, scrollable side panel. The panel reads from the
/// shared [`TelemetryCountersContent`] builder so it stays in sync with the chat
/// output without duplicating metric definitions.
///
/// # Arguments
/// - `frame` - the ratatui frame to render into.
/// - `app` - mutable `App` state; reads `telemetry_scroll_offset`; writes
///   `telemetry_area`, `telemetry_max_scroll`, and `telemetry_content_lines`.
/// - `area` - the rect allocated to the panel by the side-panel split.
fn render_telemetry_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Telemetry ",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.telemetry_area = area;

    let content = App::telemetry_counters_content();

    let header_style = Style::default()
        .fg(Color::LightGreen)
        .add_modifier(Modifier::BOLD);
    let metric_style = Style::default().fg(Color::White);
    let value_style = Style::default().fg(Color::LightGreen);
    let desc_style = Style::default().fg(Color::DarkGray);
    let type_style = Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> = Vec::new();

    for (title, group) in [
        ("Usage metrics", &content.usage),
        ("Performance metrics", &content.performance),
        ("Cost metrics", &content.cost),
        ("Effectiveness metrics", &content.effectiveness),
    ] {
        lines.push(Line::from(Span::styled(title.to_string(), header_style)));
        for (name, kind, desc, value) in group {
            // Compact line: "name value - type - description", matching the
            // `/telemetry counters` chat output as closely as the narrow panel
            // allows.
            lines.push(Line::from(vec![
                Span::styled(format!("{name} "), metric_style),
                Span::styled(value.to_string(), value_style),
                Span::styled(format!(" - {kind}"), type_style),
                Span::styled(format!(" - {desc}"), desc_style),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Cache plain-text content for text selection copy, matching the other
    // side panels' wrapping behaviour.
    let telemetry_inner_width = inner.width as usize;
    app.telemetry_content_lines = build_wrapped_content_lines(&lines, telemetry_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.telemetry_max_scroll = max_scroll;
    let scroll = app.telemetry_scroll_offset.min(max_scroll);
    let paragraph = paragraph.scroll((scroll, 0));
    frame.render_widget(paragraph, inner);

    // Render scrollbar when content overflows.
    if total_lines > visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        // Render in the full panel area so the scrollbar gutter aligns with
        // the mouse hit-test column used by the drag handler.
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Compact a byte/token estimate into a human-friendly short form for the
/// Context panel rows (e.g. `123`, `12.3k`, `1.2M`).
fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Render the Context side panel (toggled via `Alt+C`, contextpanel spec).
///
/// FR-018: titled "Context" with a border consistent with the active theme.
/// FR-005..FR-012: lists every context partition with its byte/token estimate
/// and a percentage bar of the model's context window; shows "unknown" for
/// ratios (FR-011) and lists zero-size partitions with a count of `0`
/// (FR-017). Values come from [`App::context_partition_snapshot`], which is
/// re-evaluated on every frame while the panel is open so the display stays
/// current without re-opening it (FR-014).
fn render_context_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Context ",
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.context_panel_area = area;

    // T-013/FR-015: consume the cached snapshot populated by the background
    // refresh (falling back to a synchronous computation on the first frame
    // while the async refresh is still running), so per-frame renders never
    // perform disk or SQLite I/O on the UI thread.
    let snapshot = app.context_effective_snapshot();
    let total = snapshot.total_tokens();

    // Push the content down by two blank lines under the border/title and add
    // a 4-space left margin so the partition rows read more cleanly.
    let margin_left: String = "    ".to_string();
    let blank_line = Line::from("");

    // Compact percentage bars sized to leave room for the 14-char label and
    // the value/percent columns even on narrow terminals.
    let bar_width = inner.width.saturating_sub(34).min(10).max(3) as usize;

    let header_style = Style::default()
        .fg(Color::LightCyan)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::White);
    let value_style = Style::default().fg(Color::LightCyan);
    let sub_label_style = Style::default().fg(Color::DarkGray);
    let dim_value_style = Style::default().fg(Color::Gray);
    let warn_style = Style::default().fg(Color::LightYellow);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Drop content two blank lines below the title/border.
    lines.push(blank_line.clone());
    lines.push(blank_line.clone());

    // FR-010/FR-011: show the selected model's advertised context-window
    // capacity directly above the "System prompt" row so the denominator for
    // every percentage below is unambiguous.
    let context_window_label = "Context window";
    match snapshot.context_window_tokens {
        Some(window) => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{margin_left}{context_window_label:<14}"),
                    label_style,
                ),
                Span::styled(
                    format!("{:>8}tk", format_tokens(window as u64)),
                    value_style,
                ),
            ]));
        }
        None => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{margin_left}{context_window_label:<14}"),
                    label_style,
                ),
                Span::styled("unknown".to_string(), sub_label_style),
            ]));
        }
    }

    // Provider-reported size of the context actually sent to the model on
    // the most recent turn (from the last TokenUsage event). Zero until the
    // first LLM request of the session completes.
    let sent_label = "Sent to model";
    let sent = snapshot.last_input_tokens;
    match snapshot.percent_of_window(sent) {
        Some(pct) if sent > 0 => {
            let bar = crate::theme::accessibility::progress_bar((pct / 100.0) as f32, bar_width);
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{sent_label:<14}"), label_style),
                Span::styled(format!("{:>8}tk ", format_tokens(sent)), value_style),
                Span::styled(bar, dim_value_style),
                Span::styled(format!("{:4.0}%", pct), value_style),
            ]));
        }
        Some(_) => {
            // No turn has completed yet: show the zero count without a bar.
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{sent_label:<14}"), sub_label_style),
                Span::styled(format!("{:>8}tk", format_tokens(sent)), sub_label_style),
            ]));
        }
        None => {
            // FR-011: no advertised capacity - show the absolute count only.
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{sent_label:<14}"), label_style),
                Span::styled(format!("{:>8}tk", format_tokens(sent)), value_style),
            ]));
        }
    }

    // FR-010/FR-011 partition rows: label, estimated tokens, percentage bar.
    // Sub-partitions are rendered indented as a breakdown of the system prompt.
    struct Row {
        label: &'static str,
        tokens: u64,
        sub: bool,
    }
    let rows = [
        Row {
            label: "System prompt",
            tokens: snapshot.system_prompt_tokens,
            sub: false,
        },
        Row {
            label: "skills",
            tokens: snapshot.skills_tokens,
            sub: true,
        },
        Row {
            label: "memory",
            tokens: snapshot.memory_tokens,
            sub: true,
        },
        Row {
            label: "agents.md",
            tokens: snapshot.agents_md_tokens,
            sub: true,
        },
        Row {
            label: "Tool catalog",
            tokens: snapshot.tool_catalog_tokens,
            sub: false,
        },
        Row {
            label: "Tool metadata",
            tokens: snapshot.tool_metadata_tokens,
            sub: false,
        },
        Row {
            label: "History",
            tokens: snapshot.history_tokens,
            sub: false,
        },
    ];

    for row in rows {
        let label = if row.sub {
            format!("   {}", row.label)
        } else {
            row.label.to_string()
        };
        let row_style = if row.sub {
            sub_label_style
        } else {
            label_style
        };
        match snapshot.percent_of_window(row.tokens) {
            Some(pct) => {
                let bar =
                    crate::theme::accessibility::progress_bar((pct / 100.0) as f32, bar_width);
                lines.push(Line::from(vec![
                    Span::styled(format!("{margin_left}{label:<14}"), row_style),
                    Span::styled(format!("{:>8}tk ", format_tokens(row.tokens)), value_style),
                    Span::styled(bar, dim_value_style),
                    Span::styled(format!("{:4.0}%", pct), value_style),
                ]));
            }
            None => {
                // FR-011: no advertised capacity - show absolute counts and
                // "unknown" for the ratio.
                lines.push(Line::from(vec![
                    Span::styled(format!("{margin_left}{label:<14}"), row_style),
                    Span::styled(format!("{:>8}tk ", format_tokens(row.tokens)), value_style),
                    Span::styled("unknown".to_string(), sub_label_style),
                ]));
            }
        }
    }

    // FR-008: message count is displayed alongside the history partition.
    lines.push(Line::from(Span::styled(
        format!(
            "{margin_left}   {} messages",
            snapshot.history_message_count
        ),
        sub_label_style,
    )));
    lines.push(blank_line.clone());

    // FR-012 total and remaining headroom.
    let total_label = String::from("Total");
    match snapshot.total_percent() {
        Some(pct) => {
            let bar = crate::theme::accessibility::progress_bar((pct / 100.0) as f32, bar_width);
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{total_label:<14}"), header_style),
                Span::styled(format!("{:>8}tk ", format_tokens(total)), value_style),
                Span::styled(bar, dim_value_style),
                Span::styled(format!("{:4.0}%", pct), value_style),
            ]));
        }
        None => {
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{total_label:<14}"), header_style),
                Span::styled(format!("{:>8}tk ", format_tokens(total)), value_style),
                Span::styled("unknown".to_string(), sub_label_style),
            ]));
        }
    }
    match snapshot.remaining_tokens() {
        Some(remaining) => {
            let style = if remaining == 0 {
                warn_style
            } else {
                value_style
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{margin_left}{:<14}", "Free"), label_style),
                Span::styled(format_tokens(remaining), style),
            ]));
        }
        None => {
            lines.push(Line::from(Span::styled(
                format!("{margin_left}Free: unknown"),
                sub_label_style,
            )));
        }
    }

    // Render network I/O metrics two lines below the context information.
    lines.push(blank_line.clone());
    lines.push(blank_line.clone());
    fn format_kilobytes(bytes: u64) -> String {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    }
    let io_label = format!(
        "io: ^{} v{}",
        format_kilobytes(app.stream_out_bytes),
        format_kilobytes(app.stream_in_bytes)
    );
    lines.push(Line::from(Span::styled(
        format!("{margin_left}{io_label}"),
        value_style,
    )));

    // Cache plain-text content for text selection copy, matching the other
    // side panels' wrapping behaviour.
    let context_inner_width = inner.width as usize;
    app.context_content_lines = build_wrapped_content_lines(&lines, context_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.context_panel_max_scroll = max_scroll;
    let scroll = app.context_scroll_offset.min(max_scroll);
    let paragraph = paragraph.scroll((scroll, 0));
    frame.render_widget(paragraph, inner);

    // Render scrollbar when content overflows.
    if total_lines > visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
// ---------------------------------------------------------------------------
// Chat screen
// ---------------------------------------------------------------------------

fn render_chat(frame: &mut Frame, app: &mut App) {
    // Compute chat input height based on wrapped text
    let chat_area = frame.area();

    // Responsive breakpoint for layout decisions
    let breakpoint = ResponsiveBreakpoint::from_width(chat_area.width);

    // Check minimum size (for graceful degradation if needed)
    let _below_min = is_below_minimum_size(chat_area);

    // Use responsive button column width, grown as needed so the Agents/Teams
    // labels (which may include live counts) are not truncated. This narrows
    // the input field only when the labels demand it.
    let button_col_w = breakpoint
        .button_column_width()
        .max(input_side_buttons_total_width(app));
    let input_inner_width = chat_area
        .width
        .saturating_sub(button_col_w)
        .saturating_sub(2)
        .max(1) as usize;
    // PERF-046: refresh the cached input rows/height/cursor only when the
    // buffer, the keyboard selection, or the inner width changed.  The
    // wrapped rows and cursor position are then reused verbatim across
    // frames, so per-frame input work is O(1) while the user is not typing.
    refresh_input_render_cache(app, input_inner_width as u16);
    let input_height = app.input_render_cache.height;

    // Whether to show the teammate strip (1 row under the status bar, FR-027).
    // A team with no members reserves no line at all (FR-066).
    let team_strip = crate::teammate_strip::is_visible(app);
    let team_strip_h = if team_strip { 1u16 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(breakpoint.status_bar_height()), // status bar (responsive)
            Constraint::Length(team_strip_h),                   // teammate strip (0 when hidden)
            Constraint::Min(3),                                 // messages + optional log
            Constraint::Length(input_height),                   // input (dynamic)
        ])
        .split(chat_area);

    // Use v2 status bar with responsive design
    crate::layout_statusbar::render_status_bar(frame, app, chunks[0]);

    if team_strip {
        crate::teammate_strip::render(frame, app, chunks[1]);
    }

    // Split the middle area horizontally when an auxiliary side panel is visible.
    // Use responsive split based on terminal width. Only one side panel is
    // visible at a time (mutual exclusion enforced in the toggle handlers), but
    // the `show_log && show_profile` branch is kept for the legacy stacked mode
    // reachable via the `/log` + `/profile` slash commands. The Tasks panel is a
    // third sibling (FR-004, FR-012) and is rendered alone in the side column
    // when `show_tasks_panel` is true.
    if app.show_log
        || app.show_profile
        || app.show_tasks_panel
        || app.show_memory
        || app.show_telemetry
        || app.show_context_panel
    {
        let (msg_pct, log_pct) = breakpoint.log_split();
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(msg_pct), // messages (responsive)
                Constraint::Percentage(log_pct), // side panel (responsive)
            ])
            .split(chunks[2]);

        app.message_area = h_chunks[0];
        render_messages(frame, app, h_chunks[0]);
        apply_selection_highlight(frame, app, SelectionPane::Messages, h_chunks[0]);

        // The Tasks panel is mutually exclusive with log/profile/memory/telemetry
        // (FR-012), so it gets its own branch that renders alone in the side
        // column.
        if app.show_tasks_panel {
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.teams_rows_area = Rect::default();
            app.memory_area = Rect::default();
            app.telemetry_area = Rect::default();
            app.tasks_area = h_chunks[1];
            render_tasks_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::Tasks, h_chunks[1]);
        } else if app.show_memory {
            // The Memory panel is mutually exclusive with log/profile/task/telemetry
            // (FR-004), so it gets its own branch that renders alone in the
            // side column. Clearing the other side-panel areas ensures mouse
            // hit-testing and scrollbar drag dispatch never target a panel
            // that is not actually visible.
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.teams_rows_area = Rect::default();
            app.tasks_area = Rect::default();
            app.telemetry_area = Rect::default();
            app.memory_area = h_chunks[1];
            render_memory_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::Memory, h_chunks[1]);
        } else if app.show_telemetry {
            // The Telemetry panel is mutually exclusive with log/profile/task/memory,
            // so it renders alone in the side column.
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.teams_rows_area = Rect::default();
            app.tasks_area = Rect::default();
            app.memory_area = Rect::default();
            app.telemetry_area = h_chunks[1];
            render_telemetry_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::Telemetry, h_chunks[1]);
        } else if app.show_context_panel {
            // The Context panel is mutually exclusive with the other side
            // panels (contextpanel FR-012/FR-003), so it renders alone in the
            // side column. Clearing the other side-panel areas keeps mouse
            // hit-testing and scrollbar drag dispatch from targeting a panel
            // that is not visible.
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.teams_rows_area = Rect::default();
            app.tasks_area = Rect::default();
            app.memory_area = Rect::default();
            app.telemetry_area = Rect::default();
            app.context_panel_area = h_chunks[1];
            render_context_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::ContextPanel, h_chunks[1]);
        } else {
            match (app.show_log, app.show_profile) {
                (true, true) => {
                    let side_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                        .split(h_chunks[1]);
                    app.log_area = side_chunks[0];
                    app.profile_area = side_chunks[1];
                    app.memory_area = Rect::default();
                    render_log_panel(frame, app, side_chunks[0]);
                    apply_selection_highlight(frame, app, SelectionPane::Log, side_chunks[0]);
                    render_profile_panel(frame, app, side_chunks[1]);
                    apply_selection_highlight(frame, app, SelectionPane::Profile, side_chunks[1]);
                }
                (true, false) => {
                    app.log_area = h_chunks[1];
                    app.profile_area = Rect::default();
                    app.memory_area = Rect::default();
                    render_log_panel(frame, app, h_chunks[1]);
                    apply_selection_highlight(frame, app, SelectionPane::Log, h_chunks[1]);
                }
                (false, true) => {
                    app.log_area = Rect::default();
                    app.profile_area = h_chunks[1];
                    app.active_agents_area = Rect::default();
                    app.teams_area = Rect::default();
                    app.teams_rows_area = Rect::default();
                    app.memory_area = Rect::default();
                    app.telemetry_area = Rect::default();
                    render_profile_panel(frame, app, h_chunks[1]);
                    apply_selection_highlight(frame, app, SelectionPane::Profile, h_chunks[1]);
                }
                (false, false) => {
                    app.memory_area = Rect::default();
                    app.telemetry_area = Rect::default();
                }
            }
        }
    } else {
        app.message_area = chunks[2];
        app.log_area = Rect::default();
        app.profile_area = Rect::default();
        app.tasks_area = Rect::default();
        app.memory_area = Rect::default();
        app.telemetry_area = Rect::default();
        app.context_panel_area = Rect::default();
        app.active_agents_area = Rect::default();
        app.teams_area = Rect::default();
        app.teams_rows_area = Rect::default();
        render_messages(frame, app, chunks[2]);
        apply_selection_highlight(frame, app, SelectionPane::Messages, chunks[2]);
    }

    let input_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(button_col_w), Constraint::Min(20)])
        .split(chunks[3]);

    app.input_area = input_chunks[1];
    render_input(frame, app, input_chunks[1]);
    apply_selection_highlight(frame, app, SelectionPane::Input, input_chunks[1]);
    draw_input_side_buttons(frame, app, input_chunks[0]);

    // Slash menu dropdown (above the chat input, if active)
    if app.slash_menu.is_some() {
        render_slash_menu(frame, app, input_chunks[1]);
    }

    // File menu dropdown (above the chat input, if active)
    if app.file_menu.is_some() {
        render_file_menu(frame, app, input_chunks[1]);
    }

    if !app.question_queue.is_empty() {
        render_question_dialog(frame, app);
    }

    if !app.permission_queue.is_empty() {
        render_permission_dialog(frame, app);
    }

    // Force-cleanup confirmation modal overlay
    if app.pending_forcecleanup.is_some() {
        render_force_cleanup_dialog(frame, app);
    }

    // Provider setup dialog overlay (if active, e.g. via /provider command)
    if app.provider_setup.is_some() {
        render_provider_setup_dialog(frame, app);
    } else {
        app.provider_setup_area = Rect::default();
    }

    // /loop setup dialog overlay (spec `agentloop` T-014)
    if app.loop_setup.is_some() {
        render_loop_setup_dialog(frame, app);
    }

    // Router save confirmation modal overlay
    // Must render after provider_setup so the confirmation sits on top of the
    // router setup dialog when the user presses Ctrl+S.
    if app.pending_router_save.is_some() {
        render_router_save_dialog(frame, app);
    }

    // Alt+X stop-agent confirmation modal overlay
    if app.pending_stop_confirm {
        render_stop_confirm_dialog(frame);
    }

    // MCP discover dialog overlay
    if app.mcp_discover.is_some() {
        render_mcp_discover_dialog(frame, app);
    }

    // `/plugins list` dialog overlay
    if app.plugins_list.is_some() {
        render_plugins_list_panel(frame, app);
    }

    // Shortcuts help panel overlay
    if app.show_shortcuts {
        render_shortcuts_panel(frame);
    }

    // Context menu overlay
    if app.context_menu.is_some() {
        render_context_menu(frame, app);
    }

    if app.show_agents_window {
        render_agents_window_overlay(frame, app);
    }
    if app.show_teams_window {
        render_teams_window_overlay(frame, app);
    }

    // Internal-LLM chat overlay (rendered above everything except output view).
    // Model loading / download progress popups (rendered above normal UI).
    if app.model_loading_state.is_some() {
        render_model_loading_popup(frame, app);
    }
    if app.model_download_state.is_some() {
        render_model_download_popup(frame, app);
    }

    // Render output overlay last so it always appears above Teams/Agents popups.
    if app.output_view.is_some() {
        render_output_view_overlay(frame, app);
    } else {
        app.output_view_area = Rect::default();
    }
    // Research markdown viewer overlay (above output view).
    if app.research_view.is_some() {
        render_research_view_overlay(frame, app);
    } else {
        app.research_view_area = Rect::default();
    }

    // Memory delete confirmation modal overlay.
    if app.pending_memory_delete.is_some() {
        render_memory_delete_dialog(frame, app);
    }

    // Full-memory overlay (above research view).
    if app.memory_view.is_some() {
        render_memory_view_overlay(frame, app);
    } else {
        app.memory_view_area = Rect::default();
    }
}

/// Render the full-memory overlay opened from the Alt+M panel.
fn render_memory_view_overlay(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(90, 80, frame.area());
    app.memory_view_area = area;
    frame.render_widget(Clear, area);

    let Some(view) = app.memory_view.as_mut() else {
        return;
    };

    let title = format!(" Memory #{} - {} ", view.row.id, view.row.category);
    let base = app.cwd_path.clone();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);

    // -- Memory-view line cache (mirrors research view) --------------------
    let inner_width = inner.width.saturating_sub(2);
    let cache_width = inner_width;
    // PERF-048: the memory viewer stores its rendered rows once, in
    // `wrapped_lines` (the `lines` field is left empty - the markdown rows
    // returned by `markdown_to_lines` are already wrapped to `cache_width`, so
    // no un-wrapped copy is needed).
    let need_rebuild =
        view.line_cache.cache_width != cache_width || view.line_cache.wrapped_lines.is_empty();

    if need_rebuild {
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}", view.row.id)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Category: ", Style::default().fg(Color::DarkGray)),
            Span::raw(view.row.category.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Confidence: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.2}", view.row.confidence)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
            Span::raw(view.row.source.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Updated: ", Style::default().fg(Color::DarkGray)),
            Span::raw(view.row.updated_at.clone()),
        ]));
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![Span::styled(
            "Content:",
            Style::default().fg(Color::DarkGray),
        )]));

        let content_lines = markdown_to_lines(&view.row.content, &base, cache_width as usize);
        lines.extend(content_lines.into_iter().map(|l| {
            Line::from(
                l.spans
                    .iter()
                    .map(|s| Span::styled(s.content.to_string(), s.style))
                    .collect::<Vec<_>>(),
            )
            .style(l.style)
        }));

        // Store the rendered rows once - no `lines.clone()` duplicate.
        view.line_cache.wrapped_lines = lines;
        view.line_cache.cache_width = cache_width;
        view.line_cache.content_lines = wrapped_lines_to_strings(&view.line_cache.wrapped_lines);
        view.line_cache.wrapped_count = view.line_cache.wrapped_lines.len() as u16;
    }

    let total = view.line_cache.wrapped_count;
    let visible = inner.height;
    view.max_scroll = total.saturating_sub(visible);
    view.scroll_offset = view.scroll_offset.min(view.max_scroll);
    let scroll_from_top = view.max_scroll.saturating_sub(view.scroll_offset);

    let window =
        slice_flat_wrapped_window(&view.line_cache.wrapped_lines, scroll_from_top, visible);

    frame.render_widget(Paragraph::new(window).block(block), area);

    if total > visible {
        let scroll_position = view.max_scroll.saturating_sub(view.scroll_offset) as usize;
        let mut sb_state = ScrollbarState::new(view.max_scroll as usize).position(scroll_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut sb_state,
        );
    }
}

/// Render the Alt+M memory delete confirmation modal.
fn render_memory_delete_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, area);

    let Some(ref pending) = app.pending_memory_delete else {
        return;
    };

    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Delete Memory?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Memory ID: {}", pending.id)),
        Line::from(""),
        Line::from(Span::styled(
            format!("Preview: {}", pending.preview),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter confirm  Esc cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Delete ")
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render the Alt+X stop-agent confirmation modal.
fn render_stop_confirm_dialog(frame: &mut Frame) {
    let area = centered_rect(50, 24, frame.area());
    frame.render_widget(Clear, area);

    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Are you sure?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Stop the running agent? Any goal-driven loop will"),
        Line::from("terminate with status `interrupted` (session stays resumable)."),
        Line::from(""),
        Line::from(Span::styled(
            "Enter Yes  Esc Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Stop Agent ")
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render the `/research open` markdown viewer overlay.
fn render_research_view_overlay(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(90, 80, frame.area());
    app.research_view_area = area;
    frame.render_widget(Clear, area);

    let Some(view) = app.research_view.as_mut() else {
        return;
    };

    let title = format!(" /research open: {} ", sanitize_for_display(&view.name));
    let base = view.base_dir.clone();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);

    // -- Research-view line cache (mirrors render_messages) ------------------
    //
    // `markdown_to_lines` already wraps its output to the supplied width, so
    // the lines it returns are one display row each.  We cache those wrapped
    // rows and slice the visible window, then render without `.wrap()` so the
    // scroll geometry and painted rows cannot diverge.
    let inner_width = inner.width.saturating_sub(2);
    let cache_width = inner_width;
    // PERF-048: the research viewer stores its rendered rows once, in
    // `wrapped_lines` (the `lines` field is left empty - `markdown_to_lines`
    // already wraps to `cache_width`, so no un-wrapped copy is needed).
    let need_rebuild =
        view.line_cache.cache_width != cache_width || view.line_cache.wrapped_lines.is_empty();

    if need_rebuild {
        let lines = markdown_to_lines(&view.markdown, &base, cache_width as usize);
        view.line_cache.wrapped_lines = lines
            .into_iter()
            .map(|l| {
                Line::from(
                    l.spans
                        .iter()
                        .map(|s| Span::styled(s.content.to_string(), s.style))
                        .collect::<Vec<_>>(),
                )
                .style(l.style)
            })
            .collect();
        view.line_cache.cache_width = cache_width;
        view.line_cache.content_lines = wrapped_lines_to_strings(&view.line_cache.wrapped_lines);
        view.line_cache.wrapped_count = view.line_cache.wrapped_lines.len() as u16;
    }

    let total = view.line_cache.wrapped_count;
    let visible = inner.height;
    view.max_scroll = total.saturating_sub(visible);
    view.scroll_offset = view.scroll_offset.min(view.max_scroll);
    let scroll_from_top = view.max_scroll.saturating_sub(view.scroll_offset);

    // Slice the cached pre-wrapped rows to the visible window.
    let window =
        slice_flat_wrapped_window(&view.line_cache.wrapped_lines, scroll_from_top, visible);

    let paragraph = Paragraph::new(window).block(block);

    frame.render_widget(paragraph, area);

    if total > visible {
        let scroll_position = view.max_scroll.saturating_sub(view.scroll_offset) as usize;
        let mut sb_state = ScrollbarState::new(view.max_scroll as usize).position(scroll_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut sb_state,
        );
    }
}

/// Convert a RESEARCH.md body into styled ratatui lines.
fn markdown_to_lines<'a>(
    markdown: &'a str,
    base_dir: &'a Path,
    wrap_width: usize,
) -> Vec<Line<'a>> {
    let sanitized = sanitize_for_display(markdown);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(&sanitized, opts);
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut current_spans: Vec<Span> = Vec::new();
    let mut list_stack: Vec<u8> = Vec::new();
    let mut in_code_block: Option<String> = None;
    let mut code_buffer = String::new();
    let mut pending_text = String::new();
    // Inline link/image state: (url, accumulated_visible_text)
    let mut link_state: Option<(String, String)> = None;
    let mut image_state: Option<(String, String)> = None;
    // Inline emphasis state stack.
    let mut style_stack: Vec<TextStyle> = Vec::new();

    for event in parser {
        match event {
            MdEvent::Start(tag) => match tag {
                Tag::CodeBlock(lang) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    let lang_str = match lang {
                        pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                        pulldown_cmark::CodeBlockKind::Indented => String::new(),
                    };
                    in_code_block = Some(lang_str);
                    code_buffer.clear();
                }
                Tag::List(start) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    list_stack.push(start.unwrap_or(1) as u8);
                }
                Tag::Item => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                }
                Tag::Link { dest_url, .. } => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    link_state = Some((dest_url.to_string(), String::new()));
                }
                Tag::Image { dest_url, .. } => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    image_state = Some((dest_url.to_string(), String::new()));
                }
                Tag::Emphasis => style_stack.push(TextStyle::Italic),
                Tag::Strong => style_stack.push(TextStyle::Bold),
                _ => {}
            },
            MdEvent::End(tag_end) => match tag_end {
                TagEnd::CodeBlock => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    if let Some(ref lang) = in_code_block {
                        let is_mermaid = lang.eq_ignore_ascii_case("mermaid");
                        let label = if is_mermaid {
                            "[Mermaid diagram - rendered as text below]"
                        } else {
                            ""
                        };
                        if is_mermaid && !code_buffer.trim().is_empty() {
                            lines.push(Line::from(Span::styled(
                                label.to_string(),
                                Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                            )));
                        }
                        for raw in code_buffer.lines() {
                            let line = sanitize_for_display(raw);
                            lines.push(Line::from(Span::styled(
                                format!("  {line}"),
                                if is_mermaid {
                                    Style::default().fg(Color::Cyan)
                                } else {
                                    Style::default().fg(Color::DarkGray)
                                },
                            )));
                        }
                    }
                    in_code_block = None;
                    code_buffer.clear();
                }
                TagEnd::List(_) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    list_stack.pop();
                }
                TagEnd::Heading(level) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                    let idx = lines.len().saturating_sub(1);
                    if let Some(line) = lines.get_mut(idx) {
                        let style = match level {
                            pulldown_cmark::HeadingLevel::H1 => Style::default()
                                .fg(Color::White)
                                .bg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                            pulldown_cmark::HeadingLevel::H2 => Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            _ => Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        };
                        for span in &mut line.spans {
                            span.style = style;
                        }
                    }
                }
                TagEnd::Paragraph => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::BlockQuote(_) => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                }
                TagEnd::Link => {
                    if let Some((url, text)) = link_state.take() {
                        flush_text_spans(&mut pending_text, &mut current_spans);
                        current_spans.push(Span::styled(
                            format!("[{text}]"),
                            Style::default().fg(Color::White),
                        ));
                        current_spans.push(Span::styled(
                            format!("({url})"),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                    }
                }
                TagEnd::Image => {
                    if let Some((url, alt)) = image_state.take() {
                        flush_text_spans(&mut pending_text, &mut current_spans);
                        let placeholder = image_dimensions_or_placeholder(&alt, &url, base_dir);
                        current_spans.push(Span::styled(
                            placeholder,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                }
                TagEnd::Emphasis | TagEnd::Strong => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    style_stack.pop();
                }
                _ => {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                }
            },
            MdEvent::Text(text) => {
                if in_code_block.is_some() {
                    code_buffer.push_str(&text);
                } else if let Some(ls) = link_state.as_mut() {
                    ls.1.push_str(&text);
                } else if let Some(is) = image_state.as_mut() {
                    is.1.push_str(&text);
                } else {
                    let prefix = list_prefix(&list_stack);
                    if !prefix.is_empty() && pending_text.is_empty() && current_spans.is_empty() {
                        pending_text.push_str(&prefix);
                    }
                    pending_text.push_str(&text);
                }
            }
            MdEvent::Code(code) => {
                if in_code_block.is_none() {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    current_spans.push(Span::styled(
                        format!(" `{code}` "),
                        Style::default()
                            .fg(Color::Green)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    code_buffer.push_str(&code);
                }
            }
            MdEvent::Html(html) => {
                if in_code_block.is_none() {
                    pending_text.push_str(&html);
                }
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                if in_code_block.is_none() {
                    flush_text_spans(&mut pending_text, &mut current_spans);
                    flush_line(&mut lines, &mut current_spans);
                } else {
                    code_buffer.push('\n');
                }
            }
            MdEvent::Rule => {
                flush_text_spans(&mut pending_text, &mut current_spans);
                flush_line(&mut lines, &mut current_spans);
                lines.push(Line::from(Span::styled(
                    "─".repeat(wrap_width.min(80)),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            _ => {}
        }
    }

    flush_text_spans(&mut pending_text, &mut current_spans);
    flush_line(&mut lines, &mut current_spans);

    // Footer note about terminal limitations for images and links.
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Esc to close * images shown as placeholders * links are plain text]".to_string(),
        Style::default().fg(Color::DarkGray),
    )));

    lines
}

/// Testable wrapper for the private `markdown_to_lines` renderer.
pub fn markdown_to_lines_testable<'a>(
    markdown: &'a str,
    base_dir: &'a std::path::Path,
    wrap_width: usize,
) -> Vec<Line<'a>> {
    markdown_to_lines(markdown, base_dir, wrap_width)
}

/// Testable wrapper for the private `wrap_line_styled` helper.
///
/// PERF-042: the streaming re-render throttle depends on `wrap_line_styled`
/// being the single wrapping path the message cache uses; benches guard it.
pub fn wrap_line_styled_testable(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    wrap_line_styled(line, width)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextStyle {
    Bold,
    Italic,
}

fn flush_text_spans(text: &mut String, spans: &mut Vec<Span>) {
    if !text.is_empty() {
        spans.push(Span::raw(std::mem::take(text)));
    }
}

fn flush_line<'a>(lines: &mut Vec<Line<'a>>, spans: &mut Vec<Span<'a>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}

fn list_prefix(list_stack: &[u8]) -> String {
    let depth = list_stack.len();
    if depth == 0 {
        return String::new();
    }
    let indent = "  ".repeat(depth.saturating_sub(1));
    let marker = if list_stack.last().copied().unwrap_or(1) == 0 {
        "* "
    } else {
        "* "
    };
    format!("{indent}{marker}")
}

/// Render a small centered popup with a spinner while a provider loads its model list.
fn render_model_loading_popup(frame: &mut Frame, app: &App) {
    let Some(ref state) = app.model_loading_state else {
        return;
    };
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let elapsed = state.started_at.elapsed().as_secs();
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(elapsed as usize) % 10];
    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            format!("Loading models for {}", state.provider_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} Fetching model list...", spinner),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", state.provider_id))
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

/// Render a small centered popup with a progress bar while a model is downloaded.
fn render_model_download_popup(frame: &mut Frame, app: &App) {
    let Some(ref state) = app.model_download_state else {
        return;
    };
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let percent = state.percent.clamp(0.0, 100.0);
    let elapsed = state.started_at.elapsed().as_secs();
    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            format!(
                "Downloading {}",
                crate::utils::shorten_middle(&state.model_id, 32)
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{:.1}% * {} elapsed", percent, format_elapsed(elapsed)),
            Style::default().fg(Color::Yellow),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", state.provider_id))
        .border_style(Style::default().fg(Color::Cyan));
    let paragraph = Paragraph::new(lines)
        .block(block.clone())
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);

    let gauge_area = {
        let inner = block.inner(area);
        let rows = inner.height.saturating_sub(4);
        // Position the gauge near the bottom of the popup, leaving room for text.
        let y = inner.y + rows.min(3);
        let h = inner.height.saturating_sub(rows.min(3));
        Rect::new(inner.x + 2, y, inner.width.saturating_sub(4), h.max(1))
    };
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(f64::from(percent) / 100.0)
        .label(format!("{:.0}%", percent))
        .use_unicode(true);
    frame.render_widget(gauge, gauge_area);
}

/// Format seconds as "M:SS" for the download popup.
fn format_elapsed(secs: u64) -> String {
    ragent_types::strutil::format_mmss(secs)
}

fn render_log_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Log ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    let log_inner = inner;

    app.active_agents_area = Rect::default();
    app.teams_area = Rect::default();
    app.teams_rows_area = Rect::default();

    // Determine which session to display logs for.
    // If a specific agent is selected, show its logs; otherwise show primary session.
    let _display_session = app
        .selected_agent_session_id
        .clone()
        .or_else(|| app.session_id.clone());

    // Show all log entries from all sessions (not filtered by session).
    // This allows viewing all agent activity in one view with agent_id labels.
    let all_entries = &app.log_entries;

    if all_entries.is_empty() {
        app.log_max_scroll = 0;
        let empty = Paragraph::new(Line::from(Span::styled(
            "No log entries yet",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, log_inner);
        return;
    }

    let inner_width = log_inner.width;
    let w = inner_width as usize;
    // C-008: each entry carries its own `seq` stamp and each cache group
    // mirrors the `seq` of the entry it was rendered for, so a new entry only
    // invalidates the *new* group instead of the entire cache. Newly appended
    // groups have `version: 0` (stale) and are rendered below.

    // -- Per-entry line cache (mirrors render_messages) -------------------
    //
    // The cache holds one `LogLineGroup` per log entry.  On every render:
    //   1. Reconcile cache length with `log_entries.len()`.
    //   2. Re-render any group whose `version` is stale (new entries).
    //   3. Re-wrap all groups when the terminal width changed.
    //   4. Flatten the cached lines and sum the wrapped counts.
    let need_rewrap = app.log_cache_width != inner_width;

    // Reconcile cache length. `push_log` keeps `log_line_cache` in lockstep
    // with `log_entries` (pushing a stale `version: 0` group per entry), so
    // the length normally matches. This is a defensive guard only.
    if app.log_line_cache.len() > all_entries.len() {
        app.log_line_cache.truncate(all_entries.len());
    }
    while app.log_line_cache.len() < all_entries.len() {
        app.log_line_cache.push(crate::app::LogLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            version: 0, // stale
        });
    }

    // Re-render stale groups. A group is stale when its `version` stamp does
    // not match the entry's own `seq`. Only newly-added groups (with
    // `version: 0`) and any group added while trimming dropped cache entries
    // fall into this path - previously-rendered groups keep their cached
    // lines and are not re-rendered (C-008).
    for (i, entry) in all_entries.iter().enumerate() {
        let group = &mut app.log_line_cache[i];
        if group.version != entry.seq {
            group.lines = log_entry_to_lines(entry, &app.sid_to_display_name);
            group.version = entry.seq;
            // Pre-wrap this entry's lines at the current width so scroll
            // geometry, selection copy, and the rendered window all share
            // one wrapped-row coordinate system (mirrors render_messages).
            group.wrapped_lines = group
                .lines
                .iter()
                .flat_map(|l| wrap_line_styled(l, w))
                .collect();
            group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
            group.wrapped_count = group.wrapped_lines.len() as u16;
        }
    }

    // Re-wrap all groups when the width changed.
    if need_rewrap {
        for group in app.log_line_cache.iter_mut() {
            group.wrapped_lines = group
                .lines
                .iter()
                .flat_map(|l| wrap_line_styled(l, w))
                .collect();
            group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
            group.wrapped_count = group.wrapped_lines.len() as u16;
        }
        app.log_cache_width = inner_width;
    }

    // Flatten all cached groups into a single Vec<Line> for rendering and
    // accumulate the total wrapped line count.  The full `all_lines` vector
    // is never materialised any more (see the scroll-window slice below);
    // only the content lines used for text-selection copy are collected.
    let mut all_content_lines: Vec<String> = Vec::new();
    let mut total_wrapped: u16 = 0;
    for group in app.log_line_cache.iter() {
        all_content_lines.extend(group.content_lines.iter().cloned());
        total_wrapped = total_wrapped.saturating_add(group.wrapped_count);
    }

    // Store the flattened content lines for text-selection copy.
    app.log_content_lines = all_content_lines;

    // Use the accumulated wrapped count as the total height.
    let total_lines = total_wrapped;
    let visible_height = log_inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.log_max_scroll = max_scroll;
    let scroll = app.log_scroll_offset.min(max_scroll);
    let scroll_from_top = max_scroll.saturating_sub(scroll);

    // Slice the cached pre-wrapped log rows to the visible window instead of
    // handing ratatui the entire log history inside one Paragraph with
    // .scroll(), which re-wraps and re-measures every line on every frame.
    // Both the slice and the scroll geometry are in wrapped-row coordinates,
    // so the pinned view always shows the newest entries.
    let window =
        slice_group_wrapped_window(&app.log_line_cache, scroll_from_top, visible_height, |g| {
            &g.wrapped_lines
        });

    // NOTE: no `.wrap(...)` - mirrors render_messages.  The cached log rows
    // are pre-wrapped to the inner width; re-wrapping whitespace-only rows in
    // WordWrapper paints an extra blank row each and shifts the visible tail
    // below the scroll geometry (hidden tail lines at the bottom of the pane).
    let paragraph = Paragraph::new(window);

    frame.render_widget(paragraph, log_inner);

    // Render scrollbar when content overflows
    if total_lines > visible_height {
        let scroll_position = max_scroll.saturating_sub(scroll) as usize;
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll_position);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        // Render the scrollbar in the full panel area (not the inner content
        // area) so the thumb lines up with the rightmost column that the
        // mouse handler hit-tests for side panels, matching the messages pane.
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Render a single log entry into a ratatui `Line` with styled spans.
///
/// Extracted from the original `render_log_panel` closure so the per-entry
/// cache can call it without borrowing `app` while iterating.
fn log_entry_to_lines(
    entry: &crate::app::LogEntry,
    sid_to_display_name: &std::collections::HashMap<String, String>,
) -> Vec<Line<'static>> {
    let ts = entry.timestamp.format("%H:%M:%S");
    // If this is a compaction start/end/trigger message, render it in bright green
    let msg_lower = entry.message.to_lowercase();
    let is_compaction_highlight = msg_lower.contains("compaction")
        && (msg_lower.contains("started")
            || msg_lower.contains("completed")
            || msg_lower.contains("triggered"));

    let (level_str, level_color) = if is_compaction_highlight {
        ("CMP", Color::LightGreen)
    } else {
        match entry.level {
            LogLevel::Info => ("INF", Color::Blue),
            LogLevel::Tool => ("TUL", Color::Cyan),
            LogLevel::Warn => ("WRN", Color::Yellow),
            LogLevel::Error => ("ERR", Color::Red),
        }
    };

    // Build agent_id span if present
    let mut spans = vec![
        Span::styled(format!("{ts} "), Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{level_str} "),
            Style::default()
                .fg(level_color)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    // Add agent_id label if present
    if let Some(agent_id) = &entry.agent_id {
        spans.push(Span::styled(
            format!("[{}] ", agent_id),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Parse and color the [sid:step] prefix in the message if present.
    //
    // SEC-ragent-types-001 (SECTASKS T-023): the log message body carries text
    // from tool results, web pages, repository files, and MCP responses. It is
    // neutralised at this render boundary so a CSI/OSC sequence can never be
    // interpreted by the terminal (spoofed output, clipboard write, retitle).
    let sanitised = sanitize_terminal(&entry.message);
    let msg: &str = &sanitised;
    if msg.starts_with('[') {
        // Try to find the "]" that ends the [sid:step] prefix
        if let Some(close_bracket) = msg.find(']') {
            let prefix = &msg[..=close_bracket];
            // Verify it looks like [sid:step] format (contains a colon)
            if prefix.contains(':') {
                let rest = &msg[close_bracket + 1..];
                // Extract the sid from the prefix to look up display name
                let sid_start = prefix.find('[').unwrap_or(0) + 1;
                let sid_end = prefix.find(':').unwrap_or(prefix.len() - 1);
                let sid = &prefix[sid_start..sid_end];
                // Extract step number (everything after ':' up to ']')
                let step_start = sid_end + 1;
                let step = &prefix[step_start..close_bracket];
                // Look up friendly display name if available
                let display_sid = sid_to_display_name
                    .get(sid)
                    .cloned()
                    .unwrap_or_else(|| sid.to_string());
                let formatted_prefix = format!("[{}:{step}]", display_sid);
                spans.push(Span::styled(
                    formatted_prefix,
                    Style::default().fg(Color::Yellow),
                ));
                spans.push(Span::raw(rest.to_string()));
            } else {
                spans.push(Span::raw(msg.to_string()));
            }
        } else {
            spans.push(Span::raw(msg.to_string()));
        }
    } else {
        spans.push(Span::raw(msg.to_string()));
    }
    drop(sanitised);
    vec![Line::from(spans)]
}

/// Render the TASKS side panel (todo2tasks T-015, FR-005, FR-007, FR-018).
///
/// Lists the tasks belonging to the active session (`app.session_id`).
/// Task rows are cached in `app.tasks_cache_rows` and only re-queried from
/// SQLite when `app.tasks_cache_dirty` is `true` (set by task-related events
/// and tool results).  This avoids a `storage.list_tasks` call and DAG
/// computation on every render frame, which caused noticeable scroll lag
/// when the panel was visible.
/// Each row is rendered as `[<STATUS>] <subject>` with the status prefix
/// coloured: `pending` = yellow, `in_progress` = cyan, `completed` = green,
/// `blocked` = red (FR-007). When a task is derived-blocked (FR-005), the
/// entire line is rendered in red and a `[blocked by #id, ...]` annotation
/// is appended (FR-018). An `(owner)` suffix is appended when the task has
/// an owner set (FR-018). When a task is `in_progress` and has an
/// `active_form`, it is rendered as an indented sub-line beneath the
/// subject (FR-018). When no rows are returned the panel shows a `No tasks`
/// placeholder in dark gray; if the storage query fails the panel shows
/// `Failed to load tasks` in red and does not panic. A vertical scrollbar
/// is rendered when the row count exceeds the visible height.
///
/// # Arguments
/// - `frame` - the ratatui frame to render into.
/// - `app` - mutable `App` state; reads `session_id`, `tasks_scroll_offset`,
///   and `storage`; writes `tasks_area`, `tasks_max_scroll`, and
///   `tasks_content_lines`.
/// - `area` - the rect allocated to the panel by the side-panel split.
fn render_tasks_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " TASKS ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.tasks_area = area;

    // session id when no specific agent session is selected, mirroring the
    // log panel's session-resolution logic.
    let session_id = app
        .selected_agent_session_id
        .clone()
        .or_else(|| app.session_id.clone());

    // Refresh the cached task rows only when dirty, avoiding a per-frame
    // SQLite query + DAG computation.  The cache is marked dirty by
    // task-related events (tool results for task_create/task_update, and
    // the TaskCompleted / SubagentStart / SubagentComplete events).
    if app.tasks_cache_dirty {
        let rows_result: anyhow::Result<Vec<ragent_storage::TaskRow>> = match &session_id {
            Some(sid) => app
                .storage
                .list_tasks(sid, None)
                .map_err(|e| anyhow::anyhow!(e.to_string())),
            None => Ok(Vec::new()),
        };
        match rows_result {
            Ok(rows) => {
                app.tasks_cache_rows = rows;
                app.tasks_cache_dirty = false;
            }
            Err(e) => {
                // Keep stale cache on error so the panel does not blank out
                // on a transient SQLite failure. Retry on the next render.
                tracing::warn!("tasks panel cache refresh failed: {e}");
                app.tasks_cache_dirty = true;
            }
        }
    }

    // Build display lines from the cached rows.  When the cache is empty
    // we must distinguish "no tasks" from "query failed" - the latter only
    // happens on the refresh attempt, so we can rely on cache emptiness
    // for the placeholder.
    let lines: Vec<Line> = if app.tasks_cache_rows.is_empty() {
        app.tasks_max_scroll = 0;
        vec![Line::from(Span::styled(
            "No tasks",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        // Compute derived DAG fields so we can show [blocked by ...]
        // annotations (FR-005, FR-018).
        let dag = ragent_storage::compute_task_dag(&app.tasks_cache_rows);

        let mut all_lines: Vec<Line> = Vec::new();
        for row in &app.tasks_cache_rows {
            let status_upper = row.status.to_uppercase();
            let status_color = match status_upper.as_str() {
                "PENDING" => Color::Yellow,
                "IN_PROGRESS" => Color::Cyan,
                "COMPLETED" | "DONE" => Color::Green,
                "BLOCKED" => Color::Red,
                _ => Color::DarkGray,
            };

            // Determine if this task is derived-blocked (FR-005).
            let derived = dag.get(&row.id);
            let is_derived_blocked = derived.is_some_and(|d| d.is_blocked);

            // Use red for derived-blocked pending tasks (FR-018).
            let line_color = if is_derived_blocked {
                Color::Red
            } else {
                status_color
            };

            let mut spans = vec![
                Span::styled(
                    format!("[{status_upper}] "),
                    Style::default().fg(line_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{} ", row.id), Style::default().fg(Color::DarkGray)),
                Span::styled(row.title.clone(), Style::default().fg(line_color)),
            ];

            // Append (owner) suffix when owner is set (FR-018).
            if let Some(ref owner) = row.owner
                && !owner.is_empty()
            {
                spans.push(Span::styled(
                    format!(" ({owner})"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            // Append [blocked by #id, ...] when derived blocked (FR-018).
            if is_derived_blocked {
                let deps: Vec<String> = row.blocked_by.iter().map(|id| format!("#{id}")).collect();
                spans.push(Span::styled(
                    format!(" [blocked by {}]", deps.join(", ")),
                    Style::default().fg(Color::Red),
                ));
            }

            all_lines.push(Line::from(spans));

            // Render active_form as indented sub-line when in_progress
            // (FR-018).
            if row.status == "in_progress"
                && let Some(ref active) = row.active_form
                && !active.is_empty()
            {
                all_lines.push(Line::from(Span::styled(
                    format!("  -> {active}"),
                    Style::default().fg(Color::Cyan),
                )));
            }
        }
        all_lines
    };

    // Cache plain-text content for text selection copy, matching the log
    // panel's wrapping behaviour.
    let todo_inner_width = inner.width as usize;
    app.tasks_content_lines = build_wrapped_content_lines(&lines, todo_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.tasks_max_scroll = max_scroll;
    let scroll = app.tasks_scroll_offset.min(max_scroll);
    let render_scroll = max_scroll.saturating_sub(scroll);
    let paragraph = paragraph.scroll((render_scroll, 0));
    frame.render_widget(paragraph, inner);

    // Render scrollbar when content overflows.
    if total_lines > visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(render_scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        // Render in the full panel area so the scrollbar gutter aligns with
        // the mouse hit-test column used by the drag handler.
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_profile_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Profile ",
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.profile_area = area;

    let snapshot = ragent_agent::session::profiler::agent_loop_profiler().snapshot();
    if !snapshot.enabled {
        app.profile_content_lines = vec!["Profiler is disabled".to_string()];
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "Profiler is disabled",
                Style::default().fg(Color::DarkGray),
            ))),
            inner,
        );
        return;
    }

    let mut lines = vec![
        Line::from(vec![
            Span::styled("uptime ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.1}s", snapshot.running_for_ms as f64 / 1000.0)),
            Span::raw("  "),
            Span::styled("samples ", Style::default().fg(Color::DarkGray)),
            Span::raw(snapshot.total_samples.to_string()),
        ]),
        Line::from(vec![
            Span::styled("ops ", Style::default().fg(Color::DarkGray)),
            Span::raw(snapshot.operations.len().to_string()),
            Span::raw("  "),
            Span::styled("sorted by self time", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(
            "count     avg ms    total ms     self ms     max ms    last ms  operation",
            Style::default().fg(Color::Yellow),
        )),
    ];

    if snapshot.operations.is_empty() {
        lines.push(Line::from(Span::styled(
            "Waiting for agent loop activity...",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for op in snapshot.operations {
            lines.push(Line::from(format!(
                "{:>5}  {:>10.2}  {:>10.2}  {:>10.2}  {:>9.2}  {:>9.2}  {}",
                op.count, op.avg_ms, op.total_ms, op.self_total_ms, op.max_ms, op.last_ms, op.name
            )));
        }
    }

    let profile_inner_width = inner.width as usize;
    app.profile_content_lines = build_wrapped_content_lines(&lines, profile_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.profile_max_scroll = max_scroll;
    let scroll = app.profile_scroll_offset.min(max_scroll);
    let paragraph = paragraph.scroll((max_scroll.saturating_sub(scroll), 0));
    frame.render_widget(paragraph, inner);

    if total_lines > visible_height {
        let scroll_position = max_scroll.saturating_sub(scroll) as usize;
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll_position);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        // Render in the full panel area so the scrollbar gutter aligns with
        // the mouse hit-test column used by the drag handler.
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Render the Memory side panel (toggled via `Alt+M`).
///
/// Lists the project's structured memories stored in SQLite, grouped by
/// category, with a scrollable view. Each entry shows its row id, category,
/// confidence, and a content preview. The panel also reports the total
/// memory count and the last refresh time.
fn render_memory_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Memory ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.memory_area = area;

    let mut lines: Vec<Line<'_>> = Vec::new();
    // Track which rendered line corresponds to each selectable memory row so
    // cursor navigation can scroll the selection into view.
    app.memory_row_line_indices.clear();
    app.memory_row_count = 0;

    // M-032: use the real cwd cached in `App` (it cannot change at runtime)
    // instead of calling `std::env::current_dir()` (a syscall) on every
    // rendered frame while the memory panel is visible.  We intentionally use
    // `cwd_path` rather than `cwd` because `cwd` is `~`-collapsed for display
    // and would not match the real project keys stored in the memories table.
    let project_dir = &app.cwd_path;

    // Refresh the cached memory data only when dirty, avoiding per-frame
    // N+1 SQLite queries (count + list + N x get_memory_tags).  The cache
    // is marked dirty by memory-related tool results (memory_store,
    // memory_recall, memory_forget).
    if app.memory_cache_dirty {
        let project_dir = std::path::Path::new(project_dir);
        let count = app
            .storage
            .count_memories_for_project(project_dir)
            .unwrap_or(0);
        let entries = app
            .storage
            .list_memories_for_project(project_dir, 100)
            .unwrap_or_default();
        // Pre-fetch tags for each row so the render path does not issue a
        // per-row SQLite query.
        for row in &entries {
            let _ = app.storage.get_memory_tags(row.id); // INTENTIONAL: read hint; empty/None on error is acceptable for display
        }
        app.memory_cache_count = count;
        app.memory_cache_entries = entries;
        // Sort the panel rows by category then id so the visual order and
        // the cursor index both use the same deterministic ordering.
        app.memory_cache_entries
            .sort_by(|a, b| a.category.cmp(&b.category).then(a.id.cmp(&b.id)));
        app.memory_cache_dirty = false;
        // Cursor may now be out of bounds; clamp it on the next render after
        // `memory_row_count` is recomputed below.
    }

    let count = app.memory_cache_count;
    let entries = &app.memory_cache_entries;

    lines.push(Line::from(vec![
        Span::styled(
            "Structured memories: ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(format!("{count}")),
        Span::styled(
            format!(
                " ({} bytes)",
                entries.iter().map(|r| r.content.len()).sum::<usize>()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::raw(""));

    if entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no memories for this project)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Group by category.
        let mut by_category: std::collections::BTreeMap<&str, Vec<&MemoryRow>> =
            std::collections::BTreeMap::new();
        for row in entries {
            by_category
                .entry(row.category.as_str())
                .or_default()
                .push(row);
        }

        for (category, rows) in &by_category {
            lines.push(Line::from(Span::styled(
                format!("{category} ({})", rows.len()),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for row in rows {
                let preview = truncate_bytes(&row.content, 80);
                let bytes = row.content.len();
                // Record the line index of this memory's preview row so the
                // cursor can scroll it into view.
                app.memory_row_line_indices.push(lines.len());
                app.memory_row_count += 1;
                let is_selected = app.memory_row_count.saturating_sub(1) == app.memory_cursor;
                let preview_line = Line::from(vec![
                    Span::styled(format!("#{}", row.id), Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::styled(
                        format!("{:.2}", row.confidence),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::raw(" "),
                    Span::styled(format!("{}b", bytes), Style::default().fg(Color::DarkGray)),
                    Span::raw(" "),
                    Span::raw(preview),
                ]);
                lines.push(if is_selected {
                    highlight_line(preview_line, Color::Rgb(50, 50, 80))
                } else {
                    preview_line
                });

                // Tags were pre-fetched during the cache refresh; fetch them
                // here for display.  This is still O(N) SQLite queries, but
                // only on the frame where the cache is refreshed (not every
                // frame).  When the cache is clean this path is not reached.
                let tags = app.storage.get_memory_tags(row.id).unwrap_or_default();
                if !tags.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("    tags: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(tags.join(", "), Style::default().fg(Color::Cyan)),
                    ]));
                }

                if !row.source.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("    source: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(row.source.clone()),
                    ]));
                }
            }
            lines.push(Line::raw(""));
        }

        // Keyboard hint for the interactive cursor (FR-016).
        lines.push(Line::from(Span::styled(
            "^v select  Enter open  Delete delete",
            Style::default().fg(Color::DarkGray),
        )));

        // Ensure the cursor index remains valid after a refresh or deletion.
        if app.memory_row_count > 0 {
            app.memory_cursor = app.memory_cursor.min(app.memory_row_count - 1);
        } else {
            app.memory_cursor = 0;
        }
    }

    let memory_inner_width = inner.width as usize;
    let (wrapped_lines, wrapped_starts) =
        build_wrapped_content_lines_with_starts(&lines, memory_inner_width);
    app.memory_content_lines = wrapped_lines;
    // The indices stored above are unwrapped `lines` positions; translate them
    // to wrapped-line coordinates so the cursor-scroll math uses the same unit
    // as the rendered panel height.
    for raw_idx in &mut app.memory_row_line_indices {
        *raw_idx = wrapped_starts.get(*raw_idx).copied().unwrap_or(0);
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.memory_max_scroll = max_scroll;
    let scroll = app.memory_scroll_offset.min(max_scroll);
    let paragraph = paragraph.scroll((scroll, 0));
    frame.render_widget(paragraph, inner);

    if total_lines > visible_height {
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_output_view_overlay(frame: &mut Frame, app: &mut App) {
    // The overlay reuses the shared modal window frame for its border and its
    // footer band, but carries no Close button: `Esc` (or a click outside the
    // overlay) dismisses it, and the footer row restates the scrolling keys.
    let geo = window_frame(
        OUTPUT_WINDOW_PERCENT_W,
        OUTPUT_WINDOW_PERCENT_H,
        frame.area(),
    );
    let area = geo.area;
    app.output_view_area = area;
    frame.render_widget(Clear, area);

    let Some(view) = app.output_view.as_mut() else {
        return;
    };

    // If this is a TeamMember view with a missing session_id, try to resolve
    // it from the in-memory team_members list (which is refreshed from disk
    // every render cycle via `refresh_team_member_session_ids`).
    if let OutputViewTarget::TeamMember {
        ref agent_id,
        ref mut session_id,
        ..
    } = view.target
    {
        if session_id.is_none() {
            if let Some(member) = app.team_members.iter().find(|m| m.agent_id == *agent_id) {
                if let Some(ref sid) = member.session_id {
                    *session_id = Some(sid.clone());
                }
            }
        }
    }

    let (title, target_session): (String, Option<String>) = match &view.target {
        OutputViewTarget::Session { session_id, label } => {
            (format!(" Output: {label} "), Some(session_id.clone()))
        }
        OutputViewTarget::TeamMember {
            teammate_name,
            agent_id,
            session_id,
            ..
        } => (
            format!(" Output: {} [{}] ", teammate_name, agent_id),
            session_id.clone(),
        ),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(geo.content);

    // -- Output-view line cache (mirrors render_messages / render_log_panel) -
    //
    // The cache holds the un-wrapped lines for the current target, the
    // pre-wrapped styled rows at the cached width, and the plain-text content
    // projection.  Only rebuild the un-wrapped lines when the source
    // generation changes (message count/last-edit-seq or log seq), and only
    // re-wrap when the terminal width changes.
    let inner_width = inner.width.saturating_sub(2);
    let w = inner_width.max(1) as usize;
    let need_rewrap = view.line_cache.cache_width != inner_width;

    // Compute a cheap source-generation key that captures changes to the
    // displayed messages.  For the primary session we use the in-memory
    // messages (last message edit_seq); for storage-backed sessions we fetch
    // once and derive generation from the same result.
    let (current_generation, session_messages) = {
        // Mix in target identity so switching targets invalidates the cache.
        let target_seed = match &target_session {
            Some(sid) => {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hasher::write(&mut h, sid.as_bytes());
                std::hash::Hasher::finish(&h)
            }
            None => 0,
        };
        let mut generation = target_seed;

        let session_messages: Option<std::borrow::Cow<'_, [Message]>> =
            target_session.as_ref().map(|sid| {
                if app.session_id.as_deref() == Some(sid.as_str()) {
                    // Borrow the in-memory transcript without cloning; the
                    // overlay only reads it for this frame.
                    std::borrow::Cow::Borrowed(app.messages.as_slice())
                } else {
                    // Storage-backed sessions: fetch once here; derive
                    // generation and render from the same result. Surface DB
                    // errors instead of silently showing "No output yet".
                    match app.storage.get_messages(sid) {
                        Ok(msgs) => std::borrow::Cow::Owned(msgs),
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                session_id = %sid,
                                "Failed to load session messages for output view"
                            );
                            std::borrow::Cow::Owned(vec![Message::assistant_text(
                                sid.clone(),
                                format!("[warn] Failed to load output: {e}"),
                            )])
                        }
                    }
                }
            });

        if let Some(ref msgs) = session_messages {
            let msg_count = msgs.len() as u64;
            let last_edit_seq = msgs.last().map(|m| m.edit_seq).unwrap_or(0);
            generation =
                generation.wrapping_add(msg_count.wrapping_mul(31).wrapping_add(last_edit_seq));
        }

        // Live-progress hint: for another session's target (a running
        // sub-agent or teammate) the generation key above is frozen mid-run -
        // the assistant placeholder is created once (constant message count)
        // and `edit_seq` is not persisted to SQLite. Mix in the same
        // per-session step / tool-call counters the Agents panel shows so the
        // cache rebuilds as the agent advances and completed steps appear live.
        let live_progress = target_session.as_deref().map_or(0, |sid| {
            let steps = app.event_bus.current_step(sid);
            steps
                .wrapping_mul(1_003)
                .wrapping_add(app.event_bus.current_tool_calls(sid))
        });
        generation = generation.wrapping_add(live_progress.wrapping_mul(7_919));

        (generation, session_messages)
    };

    let cache_stale = view.line_cache.source_generation != current_generation
        || view.line_cache.wrapped_lines.is_empty()
        || need_rewrap;

    if cache_stale {
        let mut lines: Vec<Line<'static>> = Vec::new();
        if let Some(ref msgs) = session_messages {
            // Build a step map from the message transcript itself so that
            // storage-backed sessions (sub-agents / teammates) get the same
            // [N.M] step/parallel-tool prefixes as the primary live session.
            let local_step_map = build_step_map_for_messages(msgs.as_ref());
            lines = messages_to_lines(
                msgs.as_ref(),
                &local_step_map,
                &app.sid_to_display_name,
                &app.cwd,
            );
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No output yet for this target",
                Style::default().fg(Color::DarkGray),
            )));
        }

        // PERF-048: store the rendered rows once, in `wrapped_lines`; the
        // un-wrapped `lines` are transient and are not retained.
        view.line_cache.wrapped_lines = lines.iter().flat_map(|l| wrap_line_styled(l, w)).collect();
        view.line_cache.content_lines = wrapped_lines_to_strings(&view.line_cache.wrapped_lines);
        view.line_cache.wrapped_count = view.line_cache.wrapped_lines.len() as u16;
        view.line_cache.cache_width = inner_width;
        view.line_cache.source_generation = current_generation;
    }

    // Compute scroll geometry from the cached wrapped count.
    let total = view.line_cache.wrapped_count;
    let visible = inner.height;
    let max_scroll = total.saturating_sub(visible);
    view.max_scroll = max_scroll;
    view.scroll_offset = view.scroll_offset.min(max_scroll);
    let scroll_from_top = max_scroll.saturating_sub(view.scroll_offset);

    // Slice the cached pre-wrapped rows to the visible window and render without `.wrap`
    // so the geometry and paint coordinate systems stay identical.
    let window =
        slice_flat_wrapped_window(&view.line_cache.wrapped_lines, scroll_from_top, visible);

    let paragraph = Paragraph::new(window).block(block);

    frame.render_widget(paragraph, geo.content);

    if total > visible {
        let scroll_position = view.max_scroll.saturating_sub(view.scroll_offset) as usize;
        let mut sb_state = ScrollbarState::new(view.max_scroll as usize).position(scroll_position);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            geo.content,
            &mut sb_state,
        );
    }

    // The footer restates the scroll keys; `Esc` dismisses the overlay, so the
    // line is the only affordance it needs.
    render_window_key_legend(
        frame,
        "PgUp/PgDn scroll  Ctrl+PgUp/PgDn jump  Esc close",
        geo.legend,
    );
}

/// Terminal-width percentage the Agents popup occupies (spec `agentui` FR-006).
///
/// 70 percent is the narrowest centred modal that still keeps the pinned
/// `[stop]` / `[x]` control columns fully inside the table: the controls end at
/// column 81 (FR-015), so their host terminal needs at least 120 columns and a
/// 70 percent modal is the first centred width that yields an inner area wide
/// enough. Below a 120-column terminal the last control is clipped (accepted).
const AGENTS_WINDOW_PERCENT_W: u16 = 70;

/// Terminal-height percentage the Agents popup occupies (spec `agentui` FR-006).
const AGENTS_WINDOW_PERCENT_H: u16 = 70;

/// Terminal-width percentage the Teams popup occupies (spec `agentui` FR-017).
const TEAMS_WINDOW_PERCENT_W: u16 = 90;

/// Terminal-height percentage the Teams popup occupies (spec `agentui` FR-017).
const TEAMS_WINDOW_PERCENT_H: u16 = 70;

/// Terminal-width percentage the output view overlay occupies. Kept at 90 per
/// cent for continuity with the pre-FR-074 overlay geometry (width was never
/// spec-pinned), now expressed through the shared [`window_frame`] helper.
const OUTPUT_WINDOW_PERCENT_W: u16 = 90;

/// Terminal-height percentage the output view overlay occupies (see
/// [`OUTPUT_WINDOW_PERCENT_W`]).
const OUTPUT_WINDOW_PERCENT_H: u16 = 70;

/// Terminal rows reserved at the bottom of a popup for the FR-077 key legend.
///
/// The legend band is the modal's last inner row. The content area above it
/// never paints that row, so the key hints read as a distinct footer strip
/// rather than competing with the table's last data row.
const WINDOW_LEGEND_ROWS: u16 = 1;

/// The Agents window's FR-077 key legend.
///
/// `Esc close` is the only dismissal the window offers: the modal has no Close
/// button, so the legend names the key a user needs to get out.
const AGENTS_WINDOW_LEGEND: &str =
    "Up/Down select  PgUp/PgDn page  Space stop/resume  Del kill  Enter open  Esc close";

/// The Teams window's FR-077 key legend (same bindings, `Del shut down`).
const TEAMS_WINDOW_LEGEND: &str =
    "Up/Down select  PgUp/PgDn page  Space stop/resume  Del shut down  Enter open  Esc close";

/// The computed rectangles of a bordered popup-window frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowFrame {
    /// The bordered modal rectangle.
    area: Rect,
    /// The scrollable content (table) area: the inner area above the legend band.
    content: Rect,
    /// The bottom inner row carrying the FR-077 key legend.
    legend: Rect,
}

/// Compute the frame rectangles for a centred popup window.
///
/// The modal is centred at `percent_w` x `percent_h` of `screen` and clamped to
/// the terminal rectangle, so it stays fully visible at any size (NFR-010). The
/// content area is the inner area minus the [`WINDOW_LEGEND_ROWS`] footer band,
/// clamped to a minimum height of one row (FR-008) and to the inner area so it
/// can never paint outside the frame (NFR-010). The footer band spans the full
/// inner width so the key legend has the whole panel to lay its hints out in.
fn window_frame(percent_w: u16, percent_h: u16, screen: Rect) -> WindowFrame {
    let area = centered_rect(percent_w, percent_h, screen).intersection(screen);
    let inner = Block::default().borders(Borders::ALL).inner(area);

    let content_h = inner
        .height
        .saturating_sub(WINDOW_LEGEND_ROWS)
        .max(1)
        .min(inner.height);
    let content = Rect::new(inner.x, inner.y, inner.width, content_h);
    let legend = Rect::new(
        inner.x,
        inner.bottom().saturating_sub(WINDOW_LEGEND_ROWS),
        inner.width,
        WINDOW_LEGEND_ROWS.min(inner.height),
    );

    WindowFrame {
        area,
        content,
        legend,
    }
}

/// Testable wrapper for the private [`window_frame`] geometry helper.
///
/// Returns `(modal, content, legend)` so the popup-frame arithmetic can be
/// asserted offline without a live terminal (spec `agentui` NFR-001).
#[doc(hidden)]
pub fn window_frame_testable(percent_w: u16, percent_h: u16, screen: Rect) -> (Rect, Rect, Rect) {
    let geo = window_frame(percent_w, percent_h, screen);
    (geo.area, geo.content, geo.legend)
}

/// The percentage width the Agents popup occupies (spec `agentui` FR-006).
#[doc(hidden)]
pub fn agents_window_percent_w() -> u16 {
    AGENTS_WINDOW_PERCENT_W
}

/// The percentage height the Agents popup occupies (spec `agentui` FR-006).
#[doc(hidden)]
pub fn agents_window_percent_h() -> u16 {
    AGENTS_WINDOW_PERCENT_H
}

/// The percentage width the Teams popup occupies (spec `agentui` FR-017).
#[doc(hidden)]
pub fn teams_window_percent_w() -> u16 {
    TEAMS_WINDOW_PERCENT_W
}

/// The percentage height the Teams popup occupies (spec `agentui` FR-017).
#[doc(hidden)]
pub fn teams_window_percent_h() -> u16 {
    TEAMS_WINDOW_PERCENT_H
}

fn render_agents_window_overlay(frame: &mut Frame, app: &mut App) {
    let geo = window_frame(
        AGENTS_WINDOW_PERCENT_W,
        AGENTS_WINDOW_PERCENT_H,
        frame.area(),
    );
    // Clear the underlay so the modal never shows the chat through its gaps.
    frame.render_widget(Clear, geo.area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Agents ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, geo.area);

    app.active_agents_area = geo.content;
    render_active_agents_subpanel(frame, app, geo.content);

    render_window_key_legend(frame, AGENTS_WINDOW_LEGEND, geo.legend);
}

/// Paint the popup's key legend, dim, along the footer band beneath the table
/// (spec `agentui` FR-077).
///
/// The band is the modal's last inner row, which the content area deliberately
/// leaves unpainted, so the legend never competes with the table rows. The
/// legend is the window's only dismissal affordance - `Esc close` sits at the
/// end of every hint line - because neither popup carries a Close button. The
/// text is clipped to the available width, and on a very narrow terminal an
/// empty rect paints nothing.
fn render_window_key_legend(frame: &mut Frame, text: &str, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

fn render_teams_window_overlay(frame: &mut Frame, app: &mut App) {
    let geo = window_frame(TEAMS_WINDOW_PERCENT_W, TEAMS_WINDOW_PERCENT_H, frame.area());
    // Clear the underlay so the modal never shows the chat through its gaps.
    frame.render_widget(Clear, geo.area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Teams ",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Blue));
    frame.render_widget(block, geo.area);

    app.teams_area = geo.content;
    crate::layout_teams::render_teams_subpanel(frame, app, geo.content);

    render_window_key_legend(frame, TEAMS_WINDOW_LEGEND, geo.legend);
}

/// Render a single message into formatted lines (FR-003, FR-006).
///
/// Delegates to `messages_to_lines` with a single-message slice so the
/// per-message line cache can re-render only the changed message during
/// streaming instead of rebuilding the entire timeline.
fn message_to_lines(
    msg: &Message,
    tool_step_map: &std::collections::HashMap<String, (String, u32, u32)>,
    sid_to_display: &std::collections::HashMap<String, String>,
    cwd: &str,
) -> Vec<Line<'static>> {
    messages_to_lines(
        std::slice::from_ref(msg),
        tool_step_map,
        sid_to_display,
        cwd,
    )
}

/// Build a `(call_id -> (short_sid, step, substep))` map from the
/// message transcript itself. This lets storage-backed sessions (sub-agents,
/// teammates, resumed sessions) render the same `[N.M]` step/parallel-tool
/// prefixes as the primary live session, without relying on the transient
/// in-memory `tool_step_map` populated from `ToolCallStart` events.
fn build_step_map_for_messages(
    messages: &[Message],
) -> std::collections::HashMap<String, (String, u32, u32)> {
    let mut map = std::collections::HashMap::new();
    let mut step = 0u32;
    for msg in messages {
        if msg.role != Role::Assistant {
            continue;
        }
        let tool_call_count = msg
            .parts
            .iter()
            .filter(|p| matches!(p, MessagePart::ToolCall { .. }))
            .count();
        if tool_call_count == 0 {
            continue;
        }
        step += 1;
        let mut substep = 0u32;
        for part in &msg.parts {
            if let MessagePart::ToolCall { call_id, .. } = part {
                substep += 1;
                let short_sid = {
                    let s = &msg.session_id;
                    let start = s.char_indices().rev().nth(7).map(|(i, _)| i).unwrap_or(0);
                    s[start..].to_string()
                };
                map.insert(call_id.clone(), (short_sid, step, substep));
            }
        }
    }
    map
}

/// Render a slice of messages into formatted lines using the rich format
/// from the primary Messages panel.  Both `render_messages` and
/// `render_output_view_overlay` delegate here so teammate output looks
/// identical to the lead agent's chat window.
fn messages_to_lines(
    messages: &[Message],
    tool_step_map: &std::collections::HashMap<String, (String, u32, u32)>,
    #[allow(clippy::used_underscore_binding)] _sid_to_display: &std::collections::HashMap<
        String,
        String,
    >,
    cwd: &str,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    for msg in messages {
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    // Agent notices are rendered in bright yellow, one item per line.
                    if msg.role == Role::Assistant && is_agent_notice(text) {
                        lines.extend(render_agent_notice_lines(text));
                        continue;
                    }

                    // SEC-080: this is the LIVE transcript renderer, which
                    // previously emitted model/tool text via `Span::raw` with no
                    // neutralisation (only the log panel sanitised). Assistant
                    // text can carry an escape sequence copied from a tool result
                    // or fetched page, so neutralise it here as well.
                    let text = sanitize_terminal(text);
                    let text: &str = &text;

                    let (dot, dot_style, indent) = match msg.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        // Context-compaction summaries are internal
                        // bookkeeping, not the agent's own output: label them
                        // so a mid-run auto-compaction summary (which uses the
                        // same "## Objective / ## Work State" shape as a
                        // sub-agent's final report) is never mistaken for the
                        // completion report in the output overlay.
                        Role::Compaction => {
                            ("[compaction] ", Style::default().fg(Color::DarkGray), 13)
                        }
                        Role::Assistant => (
                            "* ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            2,
                        ),
                    };
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            // Blank line before the "You:" prompt for visual separation.
                            if msg.role == Role::User {
                                lines.push(Line::from(String::new()));
                            }
                            lines.push(Line::from(vec![
                                Span::styled(dot, dot_style),
                                Span::raw(line.to_owned()),
                            ]));
                        } else {
                            // PERF-047: indentation is emitted as its own
                            // borrowed span instead of `format!`-ing a fresh
                            // String with `repeat()` per line (two allocations
                            // saved per continuation line).
                            lines.push(Line::from(vec![
                                Span::raw(&INDENT_SPACES[..indent]),
                                Span::raw(line.to_string()),
                            ]));
                        }
                    }
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    let step_tag = if let Some((_sid, step, substep)) = tool_step_map.get(call_id) {
                        format!("[{step}.{substep}] ")
                    } else {
                        String::new()
                    };
                    let (indicator, ind_style, name_style) = match state.status {
                        ToolCallStatus::Completed => (
                            "* ",
                            Style::default().fg(Color::Green),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Error => (
                            "[x] ",
                            Style::default().fg(Color::Red),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "* ",
                            Style::default().fg(Color::DarkGray),
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, cwd);

                    let inline_diff = if state.status == ToolCallStatus::Completed {
                        tool_inline_diff(tool, &state.output)
                    } else {
                        None
                    };

                    let mut spans = vec![
                        Span::styled(indicator, ind_style),
                        Span::styled(
                            step_tag,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    // The tool name always comes immediately after the step
                    // counter, with every other parameter rendered after it.
                    //
                    // A `think` call renders its own thought under the header
                    // from its tool state, so the header keeps the name only -
                    // unlike the other tools there is no input summary to
                    // print, and printing the model's full argument inline
                    // would duplicate the lines that follow.
                    let (name_span, argument_span) = if canonical_tool_name(tool) == "think" {
                        (display_name, None)
                    } else if summary.is_empty() {
                        (display_name, None)
                    } else {
                        (
                            format!("{} ", display_name),
                            Some(Span::styled(summary, Style::default().fg(Color::DarkGray))),
                        )
                    };
                    spans.push(Span::styled(name_span, name_style));
                    if let Some(argument) = argument_span {
                        spans.push(argument);
                    }
                    if tool == "read" {
                        if let Some(range) = read_line_range(&state.output) {
                            spans.push(Span::styled(
                                format!(" {}", range),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                    if let Some((added, removed)) = inline_diff {
                        spans.push(Span::styled(" (", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("+{}", added),
                            Style::default().fg(Color::Green),
                        ));
                        spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("-{}", removed),
                            Style::default().fg(Color::Red),
                        ));
                        spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
                    }
                    // Add duration_ms display for completed tool calls
                    if state.status == ToolCallStatus::Completed {
                        if let Some(duration_ms) = state.duration_ms {
                            let duration_str = if duration_ms < 1000 {
                                format!(" ({}ms)", duration_ms)
                            } else {
                                format!(" ({:.1}s)", duration_ms as f64 / 1000.0)
                            };
                            spans.push(Span::styled(
                                duration_str,
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                    }
                    lines.push(Line::from(spans));

                    if state.status == ToolCallStatus::Completed {
                        if tool == "think" {
                            if let Some(thought) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("thought"))
                                .and_then(|v| v.as_str())
                                .or_else(|| {
                                    state
                                        .output
                                        .as_ref()
                                        .and_then(|out| out.get("thinking"))
                                        .and_then(|v| v.as_str())
                                })
                                .or_else(|| {
                                    state
                                        .output
                                        .as_ref()
                                        .and_then(|out| out.get("text"))
                                        .and_then(|v| v.as_str())
                                })
                                .or_else(|| {
                                    state
                                        .input
                                        .get("thought")
                                        .and_then(|v| v.as_str())
                                        .filter(|_| state.output.is_none())
                                })
                            {
                                // SEC-080: `thought` is tool output; neutralise
                                // any embedded terminal escapes before rendering.
                                let thought = sanitize_terminal(thought);
                                let mut thought_lines = Vec::new();
                                for line in thought.as_ref().lines() {
                                    thought_lines.push(Line::from(Span::styled(
                                        format!("  {}", line),
                                        theme::think(),
                                    )));
                                }
                                // A model may narrate its next tool call as
                                // markup INSIDE the thought; the loop recovers
                                // and executes that call, so the transcript
                                // shows the executed call instead of the raw
                                // narration.
                                lines.extend(scrub_narrated_toolcalls(thought_lines));
                            }
                        } else if tool == "multiedit" || tool == "multi_edit" {
                            if let Some(file_stats) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("file_stats"))
                                .and_then(|v| v.as_array())
                            {
                                let rel_paths: Vec<String> = file_stats
                                    .iter()
                                    .map(|fs| {
                                        fs.get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| make_relative_path(p, cwd))
                                            .unwrap_or_default()
                                    })
                                    .collect();
                                let max_len = rel_paths.iter().map(|p| p.len()).max().unwrap_or(0);
                                for (fs, rel_path) in file_stats.iter().zip(rel_paths.iter()) {
                                    let added =
                                        fs.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let removed =
                                        fs.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let padding =
                                        " ".repeat(max_len.saturating_sub(rel_path.len()));
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            format!("  └ {}{} ", rel_path, padding),
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(
                                            format!("+{}", added),
                                            Style::default().fg(Color::Green),
                                        ),
                                        Span::styled(" ", Style::default()),
                                        Span::styled(
                                            format!("-{}", removed),
                                            Style::default().fg(Color::Red),
                                        ),
                                    ]));
                                }
                            } else if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, cwd)
                            {
                                // SEC-080: tool output is untrusted; neutralise
                                // any terminal escapes before rendering.
                                let result = sanitize_terminal(&result);
                                lines.push(Line::from(Span::styled(
                                    format!("  └ {}", result),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                        } else if tool == "agent_complete" {
                            // Render the full task-completion summary one output line
                            // per ratatui Line. Embedding a multi-line summary in a
                            // single Line does not work: ratatui strips '\n'
                            // graphemes inside a Span, so the summary would render
                            // as one continuous paragraph.
                            let summary = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("summary"))
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            // SEC-080: summary is tool output; neutralise escapes.
                            let summary = sanitize_terminal(summary);
                            let summary: &str = &summary;
                            if summary.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "  \\- Task complete",
                                    Style::default().fg(Color::Green),
                                )));
                            } else {
                                for line in summary.lines() {
                                    lines.push(Line::from(Span::styled(
                                        format!("  \\- {line}"),
                                        Style::default().fg(Color::Green),
                                    )));
                                }
                            }
                        } else if tool != "Edit"
                            && let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, cwd)
                        {
                            // SEC-080: tool output is untrusted; neutralise escapes.
                            let result = sanitize_terminal(&result);
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", result),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                    if state.status == ToolCallStatus::Error {
                        if let Some(ref err) = state.error {
                            // SEC-080: tool error text is untrusted; neutralise.
                            let err = sanitize_terminal(err);
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", err),
                                Style::default().fg(Color::Red),
                            )));
                        }
                    }
                }
                MessagePart::Reasoning { text, .. } => {
                    // SEC-080: reasoning text is model output and may embed an
                    // escape sequence; neutralise before rendering.
                    let text = sanitize_terminal(text);
                    let mut reasoning_lines = Vec::new();
                    for line in text.as_ref().lines() {
                        reasoning_lines.push(Line::from(Span::styled(
                            format!("  {}", line),
                            theme::think(),
                        )));
                    }
                    // Reasoning text is also a place models narrate tool-call
                    // markup into; scrub it the same way as think outputs.
                    lines.extend(scrub_narrated_toolcalls(reasoning_lines));
                }
                MessagePart::Image(img) => {
                    let name = img
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("image");
                    lines.push(Line::from(Span::styled(
                        format!("  {} [image: {}]", theme::ICON_ATTACHMENT, name),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }

            lines.push(Line::from(""));
        }
    }

    lines
}

/// Render the message timeline panel into `area`.
///
/// Uses the per-message line cache (`app.message_line_cache`) to avoid
/// re-rendering unchanged messages on every frame (FR-003, FR-006).
pub fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    // Determine which session to display messages for.
    // If a specific agent is selected, show its messages; otherwise show primary session.
    let _display_session = app
        .selected_agent_session_id
        .clone()
        .or_else(|| app.session_id.clone());

    // Filter messages to the selected agent's session.
    // For now, messages are still stored globally, so we match by session_id if available.
    // Tasks: Implement proper multi-session message storage to filter by _display_session.
    // This is a placeholder for future multi-session message handling.
    let messages_to_show = &app.messages;
    let inner_width = area.width.saturating_sub(2);

    // -- Per-message line cache (FR-003, FR-006) --------------------------
    //
    // The cache holds one `MessageLineGroup` per message.  Each group stores
    // the un-wrapped `Line<'static>` values (width-independent) and the
    // pre-wrapped styled rows at the cached width (`wrapped_lines`, one per
    // display row) together with the wrapped-line count.
    //
    // On every render we:
    //   1. Reconcile the cache length with `messages.len()`.
    //   2. Re-render any group whose `edit_seq` is stale (message content
    //      changed).  When only the last message changed (the common streaming
    //      case), only that one group is re-rendered (FR-006).
    //   3. Re-wrap all groups when the terminal width changed.
    //   4. Sum the wrapped counts and slice the cached wrapped rows to the
    //      visible window.
    //
    // Staleness is tracked per message via `Message::edit_seq` (bumped by
    // `Message::touch()` on every in-place mutation) rather than a global
    // version counter, so mutating one message never invalidates the cached
    // renders of the others.

    let need_rewrap = app.message_cache_width != inner_width;

    // Reconcile cache length: if messages were added or removed, adjust.
    let cache_len_before = app.message_line_cache.len();
    if app.message_line_cache.len() > messages_to_show.len() {
        app.message_line_cache.truncate(messages_to_show.len());
        // The cache shrank; the watermark may now point past the end.
        app.message_cache_dirty_from = app.message_cache_dirty_from.min(messages_to_show.len());
    }

    // Ensure every message has a cache slot.
    while app.message_line_cache.len() < messages_to_show.len() {
        app.message_line_cache.push(crate::app::MessageLineGroup {
            lines: Vec::new(),
            wrapped_lines: Vec::new(),
            content_lines: Vec::new(),
            wrapped_count: 0,
            edit_seq: 0, // rendered on the first pass below
        });
    }
    let cache_len_changed = app.message_line_cache.len() != cache_len_before;

    // Re-render stale groups.  A group is stale when its cached `edit_seq`
    // no longer matches the message's current `edit_seq`, or when the slot
    // was just created and has no lines yet.  When only the last message
    // was modified by streaming (append_assistant_text,
    // update_tool_call_status, etc.) only that one group is re-rendered.
    //
    // PERF-043: the scan starts at the lowest index any mutation lowered the
    // watermark to, so an idle frame iterates zero groups instead of the
    // whole transcript.
    let w = inner_width.max(1) as usize;
    let scan_from = app.message_cache_dirty_from.min(messages_to_show.len());
    // PERF-042: bound streaming re-render frequency.  `edit_seq` is bumped on
    // every `TextDelta`, so without throttling a growing reply is re-parsed and
    // re-wrapped on every token - O(n^2) over the reply.  A group that has
    // already been rendered may refresh at most once per
    // `MESSAGE_STREAM_MIN_INTERVAL`; intermediate edits stay pending via the
    // dirty watermark and render on a later frame.
    let throttle_ok = app
        .message_stream_throttle_at
        .is_none_or(|t| t.elapsed() >= MESSAGE_STREAM_MIN_INTERVAL);
    let mut any_rendered = false;
    let mut pending_from = messages_to_show.len();
    for i in scan_from..messages_to_show.len() {
        let msg = &messages_to_show[i];
        let group = &mut app.message_line_cache[i];
        if group.edit_seq != msg.edit_seq || group.lines.is_empty() {
            // A brand-new group must render immediately so it is never blank;
            // otherwise the throttle governs how often the group may refresh.
            if !group.lines.is_empty() && !throttle_ok {
                if i < pending_from {
                    pending_from = i;
                }
                continue;
            }
            group.lines =
                message_to_lines(msg, &app.tool_step_map, &app.sid_to_display_name, &app.cwd);
            group.edit_seq = msg.edit_seq;
            // Pre-wrap this group's lines at the current width.  Scroll
            // geometry, selection-copy content, and the rendered window are
            // all derived from `wrapped_lines`, so the wrapped and painted
            // coordinate systems can never diverge.  Without this,
            // newly-streamed messages keep `wrapped_count: 0` and the total
            // height never grows, so the view never auto-scrolls to show
            // new content.
            group.wrapped_lines = group
                .lines
                .iter()
                .flat_map(|l| wrap_line_styled(l, w))
                .collect();
            group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
            group.wrapped_count = group.wrapped_lines.len() as u16;
            any_rendered = true;
        }
    }
    // Everything before `pending_from` is up to date; a throttled group keeps
    // its index so the next frame retries it.
    app.message_cache_dirty_from = pending_from;
    if any_rendered {
        app.message_stream_throttle_at = Some(std::time::Instant::now());
    }

    // Re-wrap all groups when the width changed (FR-003).
    let mut rewrapped_clean = false;
    if need_rewrap {
        for group in app.message_line_cache.iter_mut() {
            group.wrapped_lines = group
                .lines
                .iter()
                .flat_map(|l| wrap_line_styled(l, w))
                .collect();
            group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
            group.wrapped_count = group.wrapped_lines.len() as u16;
        }
        app.message_cache_width = inner_width;
        rewrapped_clean = true;
    }

    // Sum the cached wrapped counts, and rebuild the flat plain-text copy
    // buffer only when a group's wrapped rows changed (PERF-041).  Idle frames
    // no longer clone every rendered row into `message_content_lines`.
    let mut total_wrapped: u16 = 0;
    for group in app.message_line_cache.iter() {
        total_wrapped = total_wrapped.saturating_add(group.wrapped_count);
    }

    // PERF-041: rebuild the flat plain-text copy buffer only when a group was
    // re-rendered, a clean group was re-wrapped at a new width, or the cache
    // length changed this frame.  Idle frames no longer clone every rendered
    // row into `message_content_lines`; a copy path that needs fresh data
    // calls `App::ensure_copy_content_lines`, which rebuilds on demand from
    // the per-message cache.
    if rewrapped_clean || cache_len_changed {
        app.message_content_lines_dirty = true;
    }
    if any_rendered || app.message_content_lines_dirty {
        let mut all_content_lines: Vec<String> = Vec::new();
        for group in app.message_line_cache.iter() {
            all_content_lines.extend(group.content_lines.iter().cloned());
        }
        app.message_content_lines = all_content_lines;
        app.message_content_lines_dirty = false;
    }

    // Compute scroll geometry from the cached wrapped counts (cheap - no
    // re-wrapping involved).
    let session_display = app
        .session_id
        .as_deref()
        .map(short_id)
        .unwrap_or_else(|| "none".to_string());
    let title = format!(
        " Messages │ agent: {} │ session: {} ",
        app.agent_name, session_display
    );
    let messages_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let total = total_wrapped;
    let visible = area.height.saturating_sub(2);
    let max_scroll = total.saturating_sub(visible);
    // Clamp scroll_offset when content shrinks to prevent blank timeline
    // (C3 fix: Timeline no longer goes blank when content shrinks)
    app.scroll_offset = app.scroll_offset.min(max_scroll);
    app.message_max_scroll = max_scroll;
    let scroll_from_top = max_scroll.saturating_sub(app.scroll_offset);

    // -- Scroll-window slice (idle-CPU fix) --------------------------------
    //
    // Handing ratatui a Paragraph containing the ENTIRE transcript with
    // `.scroll((offset, 0))` makes Paragraph::render re-run WordWrapper and
    // unicode-width measurement over EVERY line on EVERY frame (only the
    // no-wrap path skips work).  With hundreds of messages that is millions of
    // width computations per frame, which pinned a core even when the agent
    // was idle.
    //
    // Instead, slice the cached pre-wrapped rows down to the visible window
    // and let ratatui lay out only ~`visible` rows.  Because the slice and
    // the scroll geometry are both expressed in the same wrapped-row
    // coordinate system, the pinned-bottom view always shows the true tail
    // of the transcript and newly appended messages stay visible.
    let window =
        slice_group_wrapped_window(&app.message_line_cache, scroll_from_top, visible, |g| {
            &g.wrapped_lines
        });

    // NOTE: no `.wrap(...)` here.  The cached rows are already pre-wrapped to
    // the inner width, and re-wrapping them with `Wrap { trim: false }` would
    // split whitespace-only rows (a whitespace-only input line paints as a
    // blank row PLUS a row of spaces) and add one phantom row per such row to
    // the painted output.  The scroll window and the geometry are both derived
    // from the cache, so painting must consume exactly one row per cached row:
    // without `.wrap`, the LineTruncator path renders each cached row verbatim
    // (rows never exceed the inner width) and the two coordinate systems
    // cannot diverge.
    let paragraph = Paragraph::new(window).block(messages_block);

    frame.render_widget(paragraph, area);

    // Render scrollbar when content overflows
    if total > visible {
        let scroll_position = scroll_from_top as usize;
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll_position);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// PERF-041: render any per-message cache group that is still behind its
/// message, then rebuild the flat plain-text copy buffer from the cache.
///
/// The render loop throttles re-rendering of an actively-streaming group
/// (PERF-042), so at the moment a copy is requested that group may still hold
/// the previous frame's rows.  Render the stale groups first so the copy
/// reads exactly what is on screen, then project the wrapped rows into
/// `message_content_lines`.
pub fn ensure_message_copy_lines(app: &mut App) {
    let w = app.message_cache_width.max(1) as usize;
    // Render only the groups whose `edit_seq` is behind (usually the single
    // streaming message), so a copy costs one group render at most.
    for i in 0..app.message_line_cache.len() {
        let stale = app
            .messages
            .get(i)
            .map(|msg| app.message_line_cache[i].edit_seq != msg.edit_seq)
            .unwrap_or(false);
        if !stale {
            continue;
        }
        let lines = message_to_lines(
            &app.messages[i],
            &app.tool_step_map,
            &app.sid_to_display_name,
            &app.cwd,
        );
        let edit_seq = app.messages[i].edit_seq;
        let group = &mut app.message_line_cache[i];
        group.lines = lines;
        group.edit_seq = edit_seq;
        group.wrapped_lines = group
            .lines
            .iter()
            .flat_map(|l| wrap_line_styled(l, w))
            .collect();
        group.content_lines = wrapped_lines_to_strings(&group.wrapped_lines);
        group.wrapped_count = group.wrapped_lines.len() as u16;
    }
    app.message_cache_dirty_from = app.message_line_cache.len();
    let mut rebuilt = Vec::new();
    for group in &app.message_line_cache {
        rebuilt.extend(group.content_lines.iter().cloned());
    }
    app.message_content_lines = rebuilt;
    app.message_content_lines_dirty = false;
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    // Build title: show focused teammate or staged attachments in the block title.
    let (title, title_style) = if let Some(ref focused_id) = app.focused_teammate {
        let name = app
            .team_members
            .iter()
            .find(|m| m.agent_id == *focused_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        (
            format!(" -> {name} (focused) "),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.pending_attachments.is_empty() {
        (
            " Input ".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let names: Vec<String> = app
            .pending_attachments
            .iter()
            .filter_map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| format!("{}{s}", theme::ICON_ATTACHMENT))
            })
            .collect();
        (
            format!(" Input  {} ", names.join("  ")),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, title_style))
        .border_style(Style::default().fg(if app.is_input_locked() {
            Color::Red
        } else {
            Color::White
        }));

    // Prompt prefix: a two-digit queue counter plus "> " while entries are
    // pending, otherwise the bare "> " prompt (FR-009, FR-010). The prefix is a
    // fixed width per state so the cursor and wrap geometry stay stable
    // (NFR-002).
    let prefix = input_prompt_prefix(app.input_queue_len());

    if app.input.is_empty() {
        // Show the prompt with dimmed placeholder text so the line doesn't jump.
        let ghost = Line::from(vec![
            Span::raw(prefix.clone()),
            Span::styled(INPUT_PLACEHOLDER, Style::default().fg(Color::DarkGray)),
        ]);
        let paragraph = Paragraph::new(ghost).block(block);
        frame.render_widget(paragraph, area);
        // Cursor sits right after the prompt prefix.
        let prefix_cols = input_prompt_prefix_len(app.input_queue_len()) as u16;
        frame.set_cursor_position((area.x + 1 + prefix_cols, area.y + 1));
    } else {
        // PERF-046: reuse the rows and cursor position cached by
        // `refresh_input_render_cache` instead of re-wrapping and re-measuring
        // the buffer on every frame.
        let cache = &app.input_render_cache;
        let wrapped_lines = cache.lines.clone();
        let paragraph = Paragraph::new(wrapped_lines).block(block);
        frame.render_widget(paragraph, area);

        // Position cursor accounting for wrapped lines.
        // Use the character index (not byte length) so unicode content behaves.
        let (cursor_line, cursor_col) = cache.cursor_pos;
        let cursor_x = area.x + 1 + cursor_col as u16;
        let cursor_y = area.y + 1 + cursor_line as u16;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// All documented keybindings: (keys column, description column).
const KEYBINDINGS: &[(&str, &str)] = &[
    // -- Typing ----------------------------------------------------------
    ("@", "Mention a file - opens file picker"),
    ("/", "Slash command - opens command menu"),
    ("?", "Show this keybindings help panel"),
    (
        "Shift+Enter / Alt+Enter",
        "Insert a newline (multiline input)",
    ),
    ("Left/Right", "Move cursor within the input line"),
    ("Shift+Left/Right", "Extend/shrink keyboard selection"),
    ("Ctrl+Left/Right", "Move cursor by word"),
    ("Ctrl+Shift+Left/Right", "Extend/shrink selection by word"),
    ("Home/End", "Jump to start/end of input"),
    ("Ctrl+Home/End", "Jump to input start/end"),
    ("Ctrl+E", "Jump to end of input (terminal style)"),
    ("Ctrl+B / Ctrl+F", "Move cursor left/right (terminal style)"),
    ("Ctrl+A", "Select all input text"),
    ("Ctrl+C", "Copy selection (or quit if no selection)"),
    ("Ctrl+X", "Cut selection to clipboard"),
    ("Ctrl+V", "Paste text from clipboard"),
    ("Delete", "Delete character under cursor"),
    ("Ctrl+W", "Delete previous word"),
    ("Ctrl+K", "Delete to end of line"),
    ("Alt+V", "Paste image from clipboard as attachment"),
    ("Alt+L", "Toggle log panel visibility"),
    ("Alt+P", "Toggle profiler panel visibility"),
    ("Alt+I", "Toggle Tasks panel visibility"),
    ("Alt+O", "Toggle autopilot mode (autonomous operation)"),
    ("Alt+C", "Toggle context panel visibility"),
    ("Alt+Y", "Toggle YOLO mode (bypass safety checks)"),
    ("Alt+G", "Toggle GCF tool-result encoding"),
    ("Alt+X", "Stop the running agent (asks Are you sure?)"),
    // -- Sending ---------------------------------------------------------
    ("Enter", "Send message / confirm"),
    ("Ctrl+C, Ctrl+D", "Quit application (guarded sequence)"),
    // -- Navigation ------------------------------------------------------
    ("Shift+^ / PageUp", "Scroll messages up"),
    ("Shift+v / PageDown", "Scroll messages down"),
    ("^ / v", "Browse input history"),
    ("Ctrl+PageUp", "Scroll log panel up"),
    ("Ctrl+PageDown", "Scroll log panel down"),
    ("PageUp / PageDown", "Scroll opened output overlay"),
    ("Ctrl+PageUp/PageDown", "Output overlay: jump start/end"),
    // -- Agent ------------------------------------------------------------
    ("Tab", "Cycle to next agent"),
    ("Esc / Ctrl+X", "Cancel running agent (while processing)"),
    // -- Teams ------------------------------------------------------------
    ("Alt+v", "Focus next teammate"),
    ("Alt+^", "Focus previous teammate (or clear focus)"),
    // -- Dialogs ----------------------------------------------------------
    ("Esc", "Close any open dialog or menu"),
    ("y / a / n", "Allow / Always / Deny permission request"),
];

fn render_shortcuts_panel(frame: &mut Frame) {
    let full = frame.area();
    // Responsive sizing: up to 120 wide, up to (rows+2) tall, capped at screen.
    let w = 120u16.min(full.width.saturating_sub(4));
    let content_h = KEYBINDINGS.len() as u16 + 2; // rows + footer + borders
    let h = content_h.min(full.height.saturating_sub(2));
    let area = Rect {
        x: (full.width.saturating_sub(w)) / 2,
        y: (full.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, area);

    // Column widths inside the border (w - 2 for border, - 1 for gutter).
    let inner_w = (w.saturating_sub(3)) as usize;
    let key_col = 24usize;
    let desc_col = inner_w.saturating_sub(key_col + 2);

    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut lines: Vec<Line<'_>> = Vec::new();

    for (keys, desc) in KEYBINDINGS {
        // Pad key column to fixed width for alignment.
        let key_padded = format!("{:<width$}", keys, width = key_col);
        // Truncate desc if it overflows.
        let desc_str: &str = if desc.len() > desc_col {
            &desc[..desc_col]
        } else {
            desc
        };
        lines.push(Line::from(vec![
            Span::styled(key_padded, key_style),
            Span::styled("  ", dim_style),
            Span::styled(desc_str, desc_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc or ? to close",
        dim_style,
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " ? Shortcuts ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_context_menu(frame: &mut Frame, app: &App) {
    let menu = match app.context_menu.as_ref() {
        Some(m) => m,
        None => return,
    };

    let item_count = menu.items.len();
    let w = 12u16;
    let h = item_count as u16 + 2; // border top + items + border bottom

    // Clamp position so menu stays on screen.
    let full = frame.area();
    let x = menu.x.min(full.width.saturating_sub(w));
    let y = menu.y.min(full.height.saturating_sub(h));

    let area = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, area);

    let enabled_style = Style::default().fg(Color::White);
    let disabled_style = Style::default().fg(Color::DarkGray);
    let selected_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);

    let lines: Vec<Line<'_>> = menu
        .items
        .iter()
        .enumerate()
        .map(|(idx, &(action, enabled))| {
            let label = match action {
                ContextAction::Cut => "Cut",
                ContextAction::Copy => "Copy",
                ContextAction::Paste => "Paste",
            };
            let padded = format!(" {:<8}", label);
            if idx == menu.selected && enabled {
                Line::from(Span::styled(padded, selected_style))
            } else if enabled {
                Line::from(Span::styled(padded, enabled_style))
            } else {
                Line::from(Span::styled(padded, disabled_style))
            }
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_permission_dialog(frame: &mut Frame, app: &App) {
    let Some(ref req) = app.permission_queue.front() else {
        return;
    };
    // Standard permission dialog: y/a/n with countdown timer.
    let area = centered_rect(60, 40, frame.area()); // Increased height for better visibility
    frame.render_widget(Clear, area);

    // Calculate remaining time for countdown
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let created_at = req
        .metadata
        .get("created_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(now);
    let timeout_secs = req
        .metadata
        .get("timeout_secs")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(120);
    let elapsed = now.saturating_sub(created_at);
    let remaining = timeout_secs.saturating_sub(elapsed);

    let countdown_text = if remaining == 0 {
        "(EXPIRED)".to_string()
    } else {
        let remaining_mins = remaining / 60;
        let remaining_secs = remaining % 60;
        format!("({}:{:02} remaining)", remaining_mins, remaining_secs)
    };

    // Wrap the dialog in a block with strong styling to make it prominent
    let text = vec![
        Line::from(""),
        Line::from(format!("Permission: {}", req.permission)),
        Line::from(""),
        Line::from("Details:"),
        Line::from(Span::styled(
            req.patterns.first().map(|s| s.as_str()).unwrap_or(""),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [y] to allow  [a] to always allow  [n] to deny",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
    ];

    let queue_depth = app.permission_queue.len();
    let title_suffix = if queue_depth > 1 {
        format!(
            "[warn]  Permission Required {} ({} queued)",
            countdown_text, queue_depth
        )
    } else {
        format!("[warn]  Permission Required {}", countdown_text)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double) // Double border for emphasis
        .title(title_suffix)
        .style(Style::default().fg(Color::Yellow).bg(Color::Black)); // Ensure contrast

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

fn render_question_dialog(frame: &mut Frame, app: &App) {
    let Some(req) = app.question_queue.front() else {
        return;
    };

    if !req.options.is_empty() {
        let now = crate::utils::now_unix_ms();
        let question_lines = req.question.lines().count().max(1);
        let option_count = req.options.len();
        let total_lines = question_lines + option_count + 7;
        let height_percent = ((total_lines * 100) / frame.area().height as usize)
            .min(60)
            .max(20) as u16;
        let area = centered_rect(70, height_percent, frame.area());
        frame.render_widget(Clear, area);

        let countdown_text = question_countdown_text(req, now);

        let mut text: Vec<Line> = vec![
            Line::from(Span::styled(
                "Agent Question",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                req.question.as_str(),
                Style::default().fg(Color::White),
            )),
            Line::from(""),
        ];

        for (i, option) in req.options.iter().enumerate() {
            let is_selected = i == app.question_selected_index;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if is_selected { "> " } else { "  " };
            let marker = if i == req.resolved_default_index() {
                " (default)"
            } else {
                ""
            };
            text.push(Line::from(Span::styled(
                format!("{prefix}{option}{marker}"),
                style,
            )));
        }

        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            format!("^/v or j/k to navigate  Enter to select  Esc to dismiss{countdown_text}"),
            Style::default().fg(Color::DarkGray),
        )));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Question ")
            .style(Style::default().fg(Color::Cyan));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(ratatui::widgets::Wrap { trim: false });

        frame.render_widget(paragraph, area);
        return;
    }

    let area = centered_rect(70, 40, frame.area());
    frame.render_widget(Clear, area);

    // A secret question (the sudo askpass password) masks every typed character,
    // the same way the provider API-key field does, so a password is never
    // painted in clear text while it is being entered (audit high finding).
    let input_display = if req.secret {
        let masked = "*".repeat(app.pending_question_input.chars().count());
        format!("> {masked}_")
    } else {
        format!("> {}_", app.pending_question_input)
    };
    let text = vec![
        Line::from(Span::styled(
            "Agent Question",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            req.question.as_str(),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            input_display,
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter to submit  Esc to dismiss",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Question ")
        .style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Render the countdown suffix for a timed multiple-choice question, or an
/// empty string when the question carries no deadline (free-text / askpass).
fn question_countdown_text(req: &crate::app::QuestionRequest, now_ms: u64) -> String {
    let Some(deadline) = req.expire_at_ms else {
        return String::new();
    };
    let remaining = deadline.saturating_sub(now_ms) / 1000;
    if remaining == 0 {
        "  (auto-answering default...)".to_string()
    } else {
        format!(
            "  ({} to default)",
            ragent_types::strutil::format_mmss(remaining)
        )
    }
}
fn render_force_cleanup_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 40, frame.area());
    frame.render_widget(Clear, area);

    if let Some(ref pending) = app.pending_forcecleanup {
        let mut lines: Vec<Line<'_>> = vec![
            Line::from(Span::styled(
                "Force-cleanup Confirmation",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Team: {}", pending.team_name)),
            Line::from(""),
        ];

        if !pending.active_members.is_empty() {
            lines.push(Line::from("Active teammates:"));
            lines.push(Line::from(""));
            for m in &pending.active_members {
                lines.push(Line::from(format!("  - {}", m)));
            }
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            "Enter confirm  Esc cancel",
            Style::default().fg(Color::DarkGray),
        )));

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Force-cleanup ")
            .border_style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }
}

/// Render the `/loop` setup dialog overlay (spec `agentloop` T-014).
///
/// Layout: agent picker at the top (navigated with arrow keys), then the
/// goal / verification / scope / constraints / tool-set text fields, the
/// step and cost limit fields, the checkpoints toggle, and a footer with
/// the active-field cursor marker plus any validation error.
fn render_loop_setup_dialog(frame: &mut Frame, app: &App) {
    let Some(state) = app.loop_setup.as_ref() else {
        return;
    };
    let area = centered_rect_max(80, 80, 90, 40, frame.area());
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Start a goal-driven loop",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Agent picker row.
    let agent_marker = if state.active_field == crate::app::LoopSetupField::Agent {
        "> "
    } else {
        "  "
    };
    let mut agent_line = vec![Span::styled(
        format!("{agent_marker}agent "),
        Style::default().fg(Color::Yellow),
    )];
    for (i, (name, _desc)) in state.agents.iter().enumerate() {
        let style = if i == state.selected_agent {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        agent_line.push(Span::styled(format!("[{name}] "), style));
    }
    lines.push(Line::from(agent_line));

    // Text fields.
    let text_fields: [(
        &str,
        crate::app::LoopSetupField,
        &crate::input_field::InputField,
    ); 7] = [
        ("goal", crate::app::LoopSetupField::Goal, &state.goal_field),
        (
            "verify cmd",
            crate::app::LoopSetupField::VerifyCmd,
            &state.verify_cmd_field,
        ),
        (
            "scope",
            crate::app::LoopSetupField::Scope,
            &state.scope_field,
        ),
        (
            "read-only",
            crate::app::LoopSetupField::ReadOnly,
            &state.read_only_field,
        ),
        (
            "tools",
            crate::app::LoopSetupField::ToolSet,
            &state.tool_set_field,
        ),
        (
            "max steps",
            crate::app::LoopSetupField::MaxSteps,
            &state.max_steps_field,
        ),
        (
            "cost limit",
            crate::app::LoopSetupField::CostLimit,
            &state.cost_limit_field,
        ),
    ];
    for (label, field, input) in text_fields {
        let marker = if state.active_field == field {
            "> "
        } else {
            "  "
        };
        let cursor = if state.active_field == field {
            input.cursor()
        } else {
            input.text().chars().count()
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{marker}{label} "),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                with_cursor_marker(input.text(), cursor),
                Style::default().fg(Color::White),
            ),
        ]));
    }

    // Checkpoints toggle.
    let cp_marker = if state.active_field == crate::app::LoopSetupField::Checkpoints {
        "> "
    } else {
        "  "
    };
    let (cp_label, cp_style) = if state.checkpoints {
        ("on", Style::default().fg(Color::Green))
    } else {
        ("off", Style::default().fg(Color::DarkGray))
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{cp_marker}checkpoints "),
            Style::default().fg(Color::Cyan),
        ),
        Span::styled(cp_label, cp_style),
    ]));

    lines.push(Line::from(""));
    if let Some(error) = &state.error {
        lines.push(Line::from(Span::styled(
            format!("error: {error}"),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Tab next field - Up/Down navigate - Enter start - Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /loop setup ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// Render the interactive MCP discovery dialog overlay.
///
/// A bordered modal listing every discovered MCP server under a block cursor.
/// `Up`/`Down` move the cursor (the list scrolls, with a scrollbar, to keep it
/// on screen), `Space` toggles the highlighted server's enabled state, and `Esc`
/// closes the dialog. Each row carries an `[x]`/`[ ]` enabled marker. The body
/// uses only ASCII glyphs.
fn render_mcp_discover_dialog(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let (total, selected, prev_offset) = match app.mcp_discover.as_ref() {
        Some(s) => (s.servers.len(), s.selected, s.scroll_offset),
        None => return,
    };

    // Size the dialog: taller when there are more servers. title(2) + heading(1)
    // + rows + feedback(1) + hints(1) + borders(2).
    let body_rows = total.max(1) as u16;
    let dialog_height = (body_rows + 8).clamp(9, 40);
    // Wide enough for the fixed columns plus an absolute defining-file path:
    // the former 90-column cap clipped every path past `servers/...`, which hid
    // the very column that tells a bridged row from a configured one.
    let area = {
        let full = frame.area();
        let h = dialog_height.min(full.height.saturating_sub(4));
        let w = full.width.min(170);
        ratatui::layout::Rect {
            x: (full.width.saturating_sub(w)) / 2,
            y: (full.height.saturating_sub(h)) / 2,
            width: w,
            height: h,
        }
    };
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /mcp discover ")
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Empty state: advisory text only.
    if total == 0 {
        let lines = vec![
            Line::from(Span::styled(
                "MCP Server Discovery",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  No MCP servers detected.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  Install MCP servers via npm (e.g. @modelcontextprotocol/server-filesystem)",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  or place configs in ~/.config/ragent/servers/ and retry.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Esc to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    // Vertical split: title, pinned column heading, list, feedback, key hints.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let title_area = chunks[0];
    let heading_area = chunks[1];
    let list_area = chunks[2];
    let feedback_area = chunks[3];
    let hint_area = chunks[4];

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "MCP Server Discovery",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))),
        title_area,
    );

    // Clamp the scroll window so the cursor is always on screen, and persist it
    // so the next frame keeps the same position.
    let visible_height = list_area.height as usize;
    let scroll_offset = if visible_height == 0 {
        0
    } else if selected < prev_offset {
        selected
    } else if selected >= prev_offset + visible_height {
        selected + 1 - visible_height
    } else {
        prev_offset
    };
    if let Some(s) = app.mcp_discover.as_mut() {
        s.scroll_offset = scroll_offset;
    }

    let Some(state) = app.mcp_discover.as_ref() else {
        return;
    };

    // Pinned column heading (dark on light), matching the other list panels.
    let heading_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Gray)
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                "     {:<20}  {:<38}  {}",
                "ID", "Name", "Source / Defined in"
            ),
            heading_style,
        ))),
        heading_area,
    );

    // The block cursor: a full-row background on the highlighted row.
    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let enabled_style = Style::default().fg(Color::Yellow);
    let normal_style = Style::default().fg(Color::White);

    let items: Vec<ListItem> = state
        .servers
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, srv)| {
            let is_enabled = state.enabled.get(i).copied().unwrap_or(false);
            let marker = if is_enabled { "[x]" } else { "[ ]" };
            let name = truncate_bytes_no_ellipsis(&srv.name, 38);
            let source = match &srv.source {
                ragent_agent::mcp::McpDiscoverySource::SystemPath => "PATH",
                ragent_agent::mcp::McpDiscoverySource::NpmGlobal { .. } => "npm global",
                ragent_agent::mcp::McpDiscoverySource::McpRegistry { .. } => "MCP registry",
                ragent_agent::mcp::McpDiscoverySource::Plugin { .. } => "plugin",
                ragent_agent::mcp::McpDiscoverySource::Connector { .. } => "connector",
            };
            let row = format!(
                " {marker} {:<20}  {:<38}  {}",
                truncate_bytes_no_ellipsis(&srv.id, 20),
                name,
                match &srv.definition_file {
                    // A bridged server shows the file that declares it, so the
                    // user can see `/mcp remove` would not be the right command.
                    Some(path) => format!("{} [{}]", source, path.display()),
                    None => source.to_string(),
                },
            );
            let style = if is_enabled {
                enabled_style
            } else {
                normal_style
            };
            ListItem::new(Line::from(Span::styled(row, style)))
        })
        .collect();

    let list = List::new(items).highlight_style(cursor_style);
    let mut list_state = ListState::default();
    list_state.select(Some(selected.saturating_sub(scroll_offset)));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    // Scrollbar when the list overflows its panel.
    if total > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut sb_state = ScrollbarState::new(total).position(scroll_offset);
        frame.render_stateful_widget(scrollbar, list_area, &mut sb_state);
    }

    // Feedback line (error or success).
    let (feedback_text, feedback_color) = match state.feedback.as_deref() {
        Some(msg) if msg.starts_with("[ok]") => (msg.to_string(), Color::Green),
        Some(msg) => (msg.to_string(), Color::Red),
        None => (String::new(), Color::DarkGray),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {feedback_text}"),
            Style::default().fg(feedback_color),
        ))),
        feedback_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Up/Down move  Space toggle  Esc close",
            Style::default().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}

/// Render the interactive `/plugins list` dialog overlay (spec `plugins`
/// FR-009).
///
/// A bordered modal with the plugin table pinned at the top - a fixed column
/// heading and separator above a scrollable list under a block cursor - and the
/// capability blocks (contributions, unsupported capabilities, errors, summary,
/// and telemetry when verbose) listed at the end of the same scrollable body.
/// `Up`/`Down` move the cursor (the list scrolls, with a scrollbar, to keep it on
/// screen), `Space` toggles the highlighted plugin's enabled state, and `Esc`
/// closes the dialog. An enabled plugin's table row is tinted yellow. The body
/// uses only ASCII glyphs.
fn render_plugins_list_panel(frame: &mut Frame, app: &mut App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let (total, selected, prev_offset, has_heading) = match app.plugins_list.as_ref() {
        Some(s) => (
            s.view.body.len(),
            s.selected,
            s.scroll_offset,
            s.heading.is_some(),
        ),
        None => return,
    };

    // Size the dialog: taller when there are more body lines. title(2) +
    // header(1) + separator(1) + rows + feedback(1) + hint(1) + borders(2).
    let body_rows = total.max(1) as u16;
    let dialog_height = (body_rows + 8).clamp(9, 40);
    let area = {
        let full = frame.area();
        let h = dialog_height.min(full.height.saturating_sub(4));
        // The plugin table is wide (12 columns); use the full width up to a
        // 160-column ceiling so the columns stay aligned.
        let w = full.width.min(160);
        ratatui::layout::Rect {
            x: (full.width.saturating_sub(w)) / 2,
            y: (full.height.saturating_sub(h)) / 2,
            width: w,
            height: h,
        }
    };
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /plugins list ")
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Vertical split: title, pinned column heading, pinned separator, list,
    // feedback, hint.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let title_area = chunks[0];
    let heading_area = chunks[1];
    let separator_area = chunks[2];
    let list_area = chunks[3];
    let feedback_area = chunks[4];
    let hint_area = chunks[5];

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Installed Plugins",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))),
        title_area,
    );

    // Clamp the scroll window so the cursor is always on screen, and persist it
    // so the next frame keeps the same position.
    let visible_height = list_area.height as usize;
    let scroll_offset = if visible_height == 0 {
        0
    } else if selected < prev_offset {
        selected
    } else if selected >= prev_offset + visible_height {
        selected + 1 - visible_height
    } else {
        prev_offset
    };
    if let Some(s) = app.plugins_list.as_mut() {
        s.scroll_offset = scroll_offset;
    }

    let Some(state) = app.plugins_list.as_ref() else {
        return;
    };

    // Pinned column heading and separator (dark on light), matching the other
    // list panels and never scrolling with the rows.
    let heading_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Gray)
        .add_modifier(Modifier::BOLD);
    if has_heading {
        let heading = state.heading.clone().unwrap_or_default();
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(heading, heading_style))),
            heading_area,
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                state.view.separator.clone().unwrap_or_default(),
                heading_style,
            ))),
            separator_area,
        );
    }

    // The block cursor: a full-row background on the highlighted row.
    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let enabled_style = Style::default().fg(Color::Yellow);
    let normal_style = Style::default().fg(Color::White);

    // The first `state.view.rows.len()` body lines are plugin table rows in the
    // same order as `rows`; an enabled plugin's row is tinted yellow so its state
    // is visible at a glance (the `State` column still carries the exact value).
    let row_count = state.view.rows.len();
    let items: Vec<ListItem> = state
        .view
        .body
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, line)| {
            let style = if i < row_count && state.view.rows[i].enabled {
                enabled_style
            } else {
                normal_style
            };
            ListItem::new(Line::from(Span::styled(line.clone(), style)))
        })
        .collect();

    let list = List::new(items).highlight_style(cursor_style);
    let mut list_state = ListState::default();
    list_state.select(Some(selected.saturating_sub(scroll_offset)));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    // Scrollbar when the list overflows its panel.
    if total > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut sb_state = ScrollbarState::new(total).position(scroll_offset);
        frame.render_stateful_widget(scrollbar, list_area, &mut sb_state);
    }

    // Feedback line (error or success).
    let (feedback_text, feedback_color) = match state.feedback.as_deref() {
        Some(msg) if msg.starts_with("[ok]") => (msg.to_string(), Color::Green),
        Some(msg) => (msg.to_string(), Color::Red),
        None => (String::new(), Color::DarkGray),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {feedback_text}"),
            Style::default().fg(feedback_color),
        ))),
        feedback_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Up/Down move  Space toggle  Esc close",
            Style::default().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}

/// Render the `/history` picker overlay.
fn render_history_picker(frame: &mut Frame, app: &App) {
    use ratatui::widgets::List;
    use ratatui::widgets::ListItem;
    use ratatui::widgets::ListState;

    let picker = match &app.history_picker {
        Some(p) => p,
        None => return,
    };

    let area = frame.area();
    let popup = centered_rect(80, 70, area);
    frame.render_widget(Clear, popup);

    let visible_height = (popup.height.saturating_sub(2)) as usize; // subtract border
    let total = picker.entries.len();
    // Clamp scroll_offset so selected is always visible
    let scroll_offset = if picker.selected < picker.scroll_offset {
        picker.selected
    } else if picker.selected >= picker.scroll_offset + visible_height {
        picker.selected + 1 - visible_height
    } else {
        picker.scroll_offset
    };

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, entry)| {
            let truncated = if entry.len() > (popup.width as usize).saturating_sub(4) {
                format!(
                    "{}...",
                    &entry[..entry
                        .char_indices()
                        .map(|(pos, _)| pos)
                        .take_while(|&pos| pos < (popup.width as usize).saturating_sub(5))
                        .last()
                        .unwrap_or(0)]
                )
            } else {
                entry.clone()
            };
            let style = if i == picker.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(truncated).style(style)
        })
        .collect();

    let title = format!(
        " History ({} entries) - ^/v navigate * Enter select * c copy * Esc close ",
        total
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(picker.selected.saturating_sub(scroll_offset)));
    frame.render_stateful_widget(list, popup, &mut list_state);

    // Scrollbar
    if total > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut sb_state = ScrollbarState::new(total).position(scroll_offset);
        let sb_area = Rect {
            x: popup.right().saturating_sub(1),
            y: popup.y + 1,
            width: 1,
            height: popup.height.saturating_sub(2),
        };
        frame.render_stateful_widget(scrollbar, sb_area, &mut sb_state);
    }
}

/// Render the `/config list` save-picker overlay.
fn render_config_save_picker(frame: &mut Frame, app: &App) {
    use ratatui::widgets::{List, ListItem, ListState};

    let picker = match &app.config_save_picker {
        Some(p) => p,
        None => return,
    };

    let area = frame.area();
    let popup = centered_rect(80, 70, area);
    frame.render_widget(Clear, popup);

    let visible_height = (popup.height.saturating_sub(2)) as usize; // subtract border
    let total = picker.entries.len();
    // Clamp scroll_offset so selected is always visible
    let scroll_offset = if picker.selected < picker.scroll_offset {
        picker.selected
    } else if picker.selected >= picker.scroll_offset + visible_height {
        picker.selected + 1 - visible_height
    } else {
        picker.scroll_offset
    };

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(i, path)| {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("<unknown>")
                .to_string();
            let meta_label = match path.metadata().and_then(|m| m.modified()) {
                Ok(t) => {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    format!("  {}", dt.format("%Y-%m-%d %H:%M:%S"))
                }
                Err(_) => String::new(),
            };
            let label = format!("{file_name}{meta_label}");
            let truncated = if label.len() > (popup.width as usize).saturating_sub(4) {
                format!(
                    "{}...",
                    &label[..label
                        .char_indices()
                        .map(|(pos, _)| pos)
                        .take_while(|&pos| pos < (popup.width as usize).saturating_sub(5))
                        .last()
                        .unwrap_or(0)]
                )
            } else {
                label
            };
            let style = if i == picker.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(truncated).style(style)
        })
        .collect();

    let title = format!(
        " Saved configurations ({} entries) - ^/v navigate * Enter restore * Esc close ",
        total
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let list = List::new(items).block(block);
    let mut list_state = ListState::default();
    list_state.select(Some(picker.selected.saturating_sub(scroll_offset)));
    frame.render_stateful_widget(list, popup, &mut list_state);

    // Scrollbar
    if total > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut sb_state = ScrollbarState::new(total).position(scroll_offset);
        let sb_area = Rect {
            x: popup.right().saturating_sub(1),
            y: popup.y + 1,
            width: 1,
            height: popup.height.saturating_sub(2),
        };
        frame.render_stateful_widget(scrollbar, sb_area, &mut sb_state);
    }
}

#[cfg(test)]
#[path = "../tests/inline/layout_tests.rs"]
mod tests;
