//! Journal viewer panel — browse and search journal entries.
//!
//! Renders as an overlay listing journal entries in chronological order.
//! Supports keyboard navigation (j/k, Enter to expand, Esc to close),
//! tag filtering, and FTS5 search.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use ragent_core::storage::{JournalEntryRow, Storage};

use crate::app::App;

/// State for the journal viewer overlay panel.
#[derive(Debug, Clone)]
pub struct JournalViewerState {
    /// Loaded journal entries (newest first).
    pub entries: Vec<JournalEntryRow>,
    /// Tags for each entry (index-correlated with `entries`).
    pub entry_tags: Vec<Vec<String>>,
    /// Index of the currently highlighted entry.
    pub selected: usize,
    /// Vertical scroll offset for the entry list.
    pub scroll_offset: u16,
    /// When Some, the full content of the selected entry is shown.
    pub expanded: Option<usize>,
    /// Content scroll offset when an entry is expanded.
    pub content_scroll_offset: u16,
    /// Active tag filter (None = show all).
    pub tag_filter: Option<String>,
    /// Search query text (empty = no search).
    pub search_query: String,
    /// Whether the search input is focused.
    pub search_focused: bool,
    /// Search cursor position.
    pub search_cursor: usize,
}

impl JournalViewerState {
    /// Load journal entries from storage.
    pub fn load(storage: &Storage) -> Self {
        let entries = storage.list_journal_entries(100).unwrap_or_default();
        let entry_tags: Vec<Vec<String>> = entries
            .iter()
            .map(|e| storage.get_journal_tags(&e.id).unwrap_or_default())
            .collect();

        Self {
            entries,
            entry_tags,
            selected: 0,
            scroll_offset: 0,
            expanded: None,
            content_scroll_offset: 0,
            tag_filter: None,
            search_query: String::new(),
            search_focused: false,
            search_cursor: 0,
        }
    }

    /// Refresh entries from storage, applying current filters.
    pub fn refresh(&mut self, storage: &Storage) {
        if let Some(ref tag) = self.tag_filter {
            self.entries = storage
                .list_journal_entries_by_tag(tag, 100)
                .unwrap_or_default();
        } else {
            self.entries = storage.list_journal_entries(100).unwrap_or_default();
        }
        self.entry_tags = self
            .entries
            .iter()
            .map(|e| storage.get_journal_tags(&e.id).unwrap_or_default())
            .collect();
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    /// Run a search and replace the entry list with results.
    pub fn search(&mut self, storage: &Storage) {
        if self.search_query.is_empty() {
            self.refresh(storage);
            return;
        }
        self.entries = storage
            .search_journal_entries(&self.search_query, None, 100)
            .unwrap_or_default();
        self.entry_tags = self
            .entries
            .iter()
            .map(|e| storage.get_journal_tags(&e.id).unwrap_or_default())
            .collect();
        self.selected = 0;
        self.expanded = None;
    }

    /// Move selection up (vim k / up arrow).
    pub fn move_up(&mut self) {
        if self.expanded.is_none() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down (vim j / down arrow).
    pub fn move_down(&mut self) {
        if self.expanded.is_none() && self.selected < self.entries.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Expand or collapse the selected entry.
    pub fn toggle_expand(&mut self) {
        if self.expanded == Some(self.selected) {
            self.expanded = None;
            self.content_scroll_offset = 0;
        } else if !self.entries.is_empty() {
            self.expanded = Some(self.selected);
            self.content_scroll_offset = 0;
        }
    }

    /// Collapse the expanded view.
    pub fn collapse(&mut self) {
        if self.expanded.is_some() {
            self.expanded = None;
            self.content_scroll_offset = 0;
        }
    }

    /// Scroll content in expanded view up.
    pub fn scroll_content_up(&mut self, lines: u16) {
        self.content_scroll_offset = self.content_scroll_offset.saturating_sub(lines);
    }

    /// Scroll content in expanded view down.
    pub fn scroll_content_down(&mut self, max: u16, lines: u16) {
        if self.content_scroll_offset + lines <= max {
            self.content_scroll_offset += lines;
        } else {
            self.content_scroll_offset = max;
        }
    }

    /// Total number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Render the journal viewer overlay panel.
pub fn render_journal_viewer(frame: &mut Frame, app: &mut App) {
    let state = match &mut app.journal_viewer {
        Some(s) => s,
        None => return,
    };

    let area = centered_rect(76, 64, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Journal ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Close button area
    let close_w = 10u16.min(inner.width);
    let close_h = 3u16.min(inner.height);
    let close_x = inner.right().saturating_sub(close_w);
    let close_y = inner.bottom().saturating_sub(close_h);
    let close_area = Rect::new(close_x, close_y, close_w, close_h);
    app.journal_viewer_close_area = close_area;

    // Search bar (2 lines at top of content area)
    let search_h = 2u16;
    let search_area = Rect::new(inner.x, inner.y, inner.width, search_h);
    let content_top = inner.y + search_h + 1;
    let content_h = inner.height.saturating_sub(close_h + 1 + search_h + 1);
    let content_area = Rect::new(inner.x, content_top, inner.width, content_h.max(1));
    app.journal_viewer_area = content_area;

    // Render search bar
    let search_prefix = Span::styled(" 🔍 ", Style::default().fg(Color::Yellow));
    let search_text = if state.search_query.is_empty() {
        Span::styled(
            "Type to search (FTS5)…",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )
    } else {
        Span::styled(
            state.search_query.clone(),
            Style::default().fg(Color::White),
        )
    };
    let cursor_char = if state.search_focused { "█" } else { " " };
    let cursor = Span::styled(cursor_char, Style::default().fg(Color::Yellow));

    let filter_text = if let Some(ref tag) = state.tag_filter {
        Span::styled(format!("  tag:{tag}"), Style::default().fg(Color::Blue))
    } else {
        Span::raw("")
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            search_prefix,
            search_text,
            cursor,
            filter_text,
        ])),
        search_area,
    );

    // Render content
    if state.entries.is_empty() {
        let empty_text = if state.search_query.is_empty() {
            "No journal entries found.\n\n\
             Create entries with: /journal add <title>"
                .to_string()
        } else {
            format!("No results for '{}'.", state.search_query)
        };
        frame.render_widget(
            Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: false }),
            content_area,
        );
    } else if let Some(expanded_idx) = state.expanded {
        if expanded_idx < state.entries.len() {
            render_expanded_entry(frame, state, expanded_idx, content_area);
        }
    } else {
        render_entry_list(frame, state, content_area);
    }

    // Close button
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Close ",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center),
        close_area,
    );
}

/// Render the list of journal entries.
fn render_entry_list(frame: &mut Frame, state: &mut JournalViewerState, area: Rect) {
    // Header line
    let header = Line::from(vec![Span::styled(
        format!(" {} entries ", state.entries.len()),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )]);

    let mut lines: Vec<Line<'_>> = vec![header, Line::raw("")];

    for (i, entry) in state.entries.iter().enumerate() {
        let is_selected = i == state.selected;
        let cursor = if is_selected { "▶ " } else { "  " };

        // Title
        let title_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let title = Span::styled(&entry.title, title_style);

        // Timestamp
        let updated_rel = format_relative_time(&entry.timestamp);
        let time_span = Span::styled(
            format!("  {updated_rel}"),
            Style::default().fg(Color::DarkGray),
        );

        // Tags
        let tags = if i < state.entry_tags.len() && !state.entry_tags[i].is_empty() {
            let tag_str = state.entry_tags[i]
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ");
            Span::styled(format!("  {tag_str}"), Style::default().fg(Color::Blue))
        } else {
            Span::raw("")
        };

        let row_bg = if is_selected {
            Color::Rgb(40, 40, 60)
        } else {
            Color::default()
        };

        lines.push(
            Line::from(vec![
                Span::styled(cursor.to_string(), Style::default().fg(Color::Yellow)),
                title,
                time_span,
                tags,
            ])
            .style(Style::default().bg(row_bg)),
        );

        // Content preview for selected item
        if is_selected {
            let preview = truncate_preview(&entry.content, 80);
            if !preview.is_empty() {
                lines.push(Line::styled(
                    format!("     {}", preview),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
    }

    // Help line
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(" j/k ", Style::default().fg(Color::Yellow)),
        Span::styled("navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter ", Style::default().fg(Color::Yellow)),
        Span::styled("expand  ", Style::default().fg(Color::DarkGray)),
        Span::styled("/ ", Style::default().fg(Color::Yellow)),
        Span::styled("search  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc ", Style::default().fg(Color::Yellow)),
        Span::styled("close", Style::default().fg(Color::DarkGray)),
    ]));

    let content = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(content, area);
}

/// Render the expanded content of a selected journal entry.
fn render_expanded_entry(
    frame: &mut Frame,
    state: &mut JournalViewerState,
    idx: usize,
    area: Rect,
) {
    let entry = &state.entries[idx];
    let tags = if idx < state.entry_tags.len() {
        &state.entry_tags[idx]
    } else {
        &Vec::new()
    };

    let mut header_lines = Vec::new();

    header_lines.push(Line::from(vec![Span::styled(
        format!(" {} ", entry.title),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]));

    // Tags
    if !tags.is_empty() {
        let tag_spans: Vec<Span<'_>> = tags
            .iter()
            .flat_map(|t| {
                vec![
                    Span::styled(format!("#{t}"), Style::default().fg(Color::Blue)),
                    Span::raw(" "),
                ]
            })
            .collect();
        header_lines.push(Line::from(tag_spans));
    }

    // Metadata
    let meta = format!(
        " id: {}… | project: {} | {} ",
        &entry.id[..8.min(entry.id.len())],
        if entry.project.is_empty() {
            "-"
        } else {
            &entry.project
        },
        format_relative_time(&entry.timestamp),
    );
    header_lines.push(Line::from(Span::styled(
        meta,
        Style::default().fg(Color::DarkGray),
    )));
    header_lines.push(Line::raw("─".repeat(area.width as usize)));
    header_lines.push(Line::raw(""));

    // Content lines
    let content_lines: Vec<Line<'_>> = entry
        .content
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let total_lines = header_lines.len() + content_lines.len();
    let visible = area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible) as u16;

    if state.content_scroll_offset > max_scroll {
        state.content_scroll_offset = max_scroll;
    }

    let scroll = state.content_scroll_offset as usize;

    let mut all_lines: Vec<Line<'_>> = header_lines;
    all_lines.extend(content_lines);

    let visible_lines: Vec<Line<'_>> = all_lines
        .iter()
        .skip(scroll)
        .take(visible)
        .cloned()
        .collect();

    let paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);

    // Scrollbar
    if total_lines > visible {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut scrollbar_state,
        );
    }
}

/// Format a relative time string from an ISO 8601 timestamp.
fn format_relative_time(iso: &str) -> String {
    let ts = match chrono::DateTime::parse_from_rfc3339(iso) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(_) => return iso.to_string(),
    };
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(ts);
    if diff.num_seconds() < 0 {
        return "just now".to_string();
    }
    if diff.num_seconds() < 60 {
        return format!("{}s ago", diff.num_seconds());
    }
    if diff.num_minutes() < 60 {
        return format!("{}m ago", diff.num_minutes());
    }
    if diff.num_hours() < 24 {
        return format!("{}h ago", diff.num_hours());
    }
    if diff.num_days() < 30 {
        return format!("{}d ago", diff.num_days());
    }
    format!("{}mo ago", diff.num_days() / 30)
}

/// Truncate content for a one-line preview.
fn truncate_preview(content: &str, max_chars: usize) -> String {
    let first_line = content.lines().next().unwrap_or("");
    let truncated: String = first_line.chars().take(max_chars).collect();
    if first_line.chars().count() > max_chars {
        format!("{}…", truncated)
    } else {
        truncated
    }
}

/// Centered rectangle helper.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
