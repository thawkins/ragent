//! Memory browser panel — lists all memory blocks with content preview.
//!
//! Renders as an overlay listing both global and project memory blocks.
//! Supports keyboard navigation (j/k, Enter to expand, Esc to close)
//! and highlights blocks that are near their size limit (>90%).

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};

use ragent_core::memory::{BlockScope, FileBlockStorage, MemoryBlock, load_all_blocks};
use ragent_core::storage::Storage;

use crate::app::App;
use crate::utils::centered_rect;

/// State for the memory browser overlay panel.
#[derive(Debug, Clone)]
pub struct MemoryBrowserState {
    /// All loaded memory blocks: (scope, block).
    pub blocks: Vec<(BlockScope, MemoryBlock)>,
    /// Index of the currently highlighted block.
    pub selected: usize,
    /// Vertical scroll offset for the block list.
    pub scroll_offset: u16,
    /// When Some, the full content of the selected block is shown.
    pub expanded: Option<usize>,
    /// Content scroll offset when a block is expanded.
    pub content_scroll_offset: u16,
    /// Number of structured memories in the SQLite store.
    pub structured_count: u64,
}

impl MemoryBrowserState {
    /// Load all memory blocks and structured memory counts from storage.
    pub fn load(storage: &Storage) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let block_storage = FileBlockStorage::new();
        let blocks = load_all_blocks(&block_storage, &working_dir);

        let structured_count = storage.count_memories().unwrap_or(0);

        Self {
            blocks,
            selected: 0,
            scroll_offset: 0,
            expanded: None,
            content_scroll_offset: 0,
            structured_count,
        }
    }

    /// Refresh the block list and structured memory count.
    pub fn refresh(&mut self, storage: &Storage) {
        let working_dir = std::env::current_dir().unwrap_or_default();
        let block_storage = FileBlockStorage::new();
        self.blocks = load_all_blocks(&block_storage, &working_dir);
        self.structured_count = storage.count_memories().unwrap_or(0);
        if self.selected >= self.blocks.len() && !self.blocks.is_empty() {
            self.selected = self.blocks.len().saturating_sub(1);
        }
    }

    /// Move selection up (vim j).
    pub fn move_up(&mut self) {
        if self.expanded.is_none() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down (vim k).
    pub fn move_down(&mut self) {
        if self.expanded.is_none() && self.selected < self.blocks.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Expand or collapse the selected block.
    pub fn toggle_expand(&mut self) {
        if self.expanded == Some(self.selected) {
            self.expanded = None;
            self.content_scroll_offset = 0;
        } else if !self.blocks.is_empty() {
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

    /// Total number of blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }
}

/// Render the memory browser overlay panel.
pub fn render_memory_browser(frame: &mut Frame, app: &mut App) {
    let state = match &mut app.memory_browser {
        Some(s) => s,
        None => return,
    };

    let area = centered_rect(72, 62, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Memory Browser ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Close button area
    let close_w = 10u16.min(inner.width);
    let close_h = 3u16.min(inner.height);
    let close_x = inner.right().saturating_sub(close_w);
    let close_y = inner.bottom().saturating_sub(close_h);
    let close_area = Rect::new(close_x, close_y, close_w, close_h);
    app.memory_browser_close_area = close_area;

    let content_h = inner.height.saturating_sub(close_h + 1);
    let content_area = Rect::new(inner.x, inner.y, inner.width, content_h.max(1));
    app.memory_browser_area = content_area;

    if state.blocks.is_empty() {
        let empty_text = format!(
            "No memory blocks found.\n\n\
             Structured memories: {}\n\n\
             Create blocks with: /memory write <label> <content>\n\
             Store structured memories with: memory_store tool",
            state.structured_count
        );
        frame.render_widget(
            Paragraph::new(empty_text)
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: false }),
            content_area,
        );
    } else if let Some(expanded_idx) = state.expanded {
        if expanded_idx < state.blocks.len() {
            render_expanded_block(frame, state, expanded_idx, content_area);
        }
    } else {
        render_block_list(frame, state, content_area);
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

/// Render the list of memory blocks.
fn render_block_list(frame: &mut Frame, state: &mut MemoryBrowserState, area: Rect) {
    // Header line
    let header = Line::from(vec![Span::styled(
        format!(
            " {} blocks, {} structured memories ",
            state.blocks.len(),
            state.structured_count
        ),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )]);

    let mut lines: Vec<Line<'_>> = vec![header, Line::raw("")];

    for (i, (scope, block)) in state.blocks.iter().enumerate() {
        let is_selected = i == state.selected;
        let cursor = if is_selected { "▶ " } else { "  " };

        // Scope badge
        let scope_badge = match scope {
            BlockScope::Global => Span::styled("[global] ", Style::default().fg(Color::Blue)),
            BlockScope::Project => Span::styled("[project]", Style::default().fg(Color::Green)),
        };

        // Label
        let label_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let label = Span::styled(format!(" {}", block.label), label_style);

        // Size indicator with near-limit highlight
        let content_len = block.content.len();
        let size_text = if block.limit > 0 {
            let pct = (content_len as f64 / block.limit as f64 * 100.0) as u8;
            let size_style = if pct > 90 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if pct > 70 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Span::styled(
                format!(" {pct}%({content_len}/{})", block.limit),
                size_style,
            )
        } else {
            let size_str = if content_len >= 1024 {
                format!(" {:.1}KB", content_len as f64 / 1024.0)
            } else {
                format!(" {content_len}B")
            };
            Span::styled(size_str, Style::default().fg(Color::DarkGray))
        };

        // Read-only badge
        let read_only = if block.read_only {
            Span::styled(" 🔒", Style::default().fg(Color::Red))
        } else {
            Span::raw("")
        };

        // Description (truncated)
        let desc = if !block.description.is_empty() {
            let d: String = block.description.chars().take(40).collect();
            Span::styled(format!(" — {d}"), Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        };

        // Relative time
        let updated_rel = format_relative_time(&block.updated_at.to_rfc3339());
        let time_span = Span::styled(
            format!("  {updated_rel}"),
            Style::default().fg(Color::DarkGray),
        );

        let row_bg = if is_selected {
            Color::Rgb(40, 40, 60)
        } else {
            Color::default()
        };

        lines.push(
            Line::from(vec![
                Span::styled(cursor.to_string(), Style::default().fg(Color::Yellow)),
                scope_badge,
                label,
                size_text,
                read_only,
                desc,
                time_span,
            ])
            .style(Style::default().bg(row_bg)),
        );

        // Content preview for selected item
        if is_selected {
            lines.push(Line::styled(
                format!("     {}", truncate_preview(&block.content, 80)),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    // Help line
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(" j/k ", Style::default().fg(Color::Yellow)),
        Span::styled("navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter ", Style::default().fg(Color::Yellow)),
        Span::styled("expand  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc ", Style::default().fg(Color::Yellow)),
        Span::styled("close", Style::default().fg(Color::DarkGray)),
    ]));

    let content = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(content, area);
}

/// Render the expanded content of a selected block.
fn render_expanded_block(
    frame: &mut Frame,
    state: &mut MemoryBrowserState,
    idx: usize,
    area: Rect,
) {
    let (scope, block) = &state.blocks[idx];

    let scope_label = match scope {
        BlockScope::Global => "global",
        BlockScope::Project => "project",
    };

    let mut header_lines = Vec::new();

    header_lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", block.label),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[{scope_label}]"),
            Style::default().fg(if *scope == BlockScope::Global {
                Color::Blue
            } else {
                Color::Green
            }),
        ),
    ]));

    if !block.description.is_empty() {
        header_lines.push(Line::from(Span::styled(
            format!(" {} ", block.description),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let meta = format!(
        " size: {}B | limit: {} | read-only: {} | updated: {} ",
        block.content.len(),
        if block.limit > 0 {
            format!("{}B", block.limit)
        } else {
            "none".to_string()
        },
        if block.read_only { "yes" } else { "no" },
        format_relative_time(&block.updated_at.to_rfc3339()),
    );
    header_lines.push(Line::from(Span::styled(
        meta,
        Style::default().fg(Color::DarkGray),
    )));
    header_lines.push(Line::raw("─".repeat(area.width as usize)));
    header_lines.push(Line::raw(""));

    // Content lines
    let content_lines: Vec<Line<'_>> = block
        .content
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let total_lines = header_lines.len() + content_lines.len();
    let visible = area.height as usize;
    let max_scroll = total_lines.saturating_sub(visible) as u16;

    // Adjust content scroll
    if state.content_scroll_offset > max_scroll {
        state.content_scroll_offset = max_scroll;
    }

    let scroll = state.content_scroll_offset as usize;

    // Combine header + content, then apply scroll window
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

