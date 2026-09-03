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

use crate::app::{image_dimensions_or_placeholder, sanitize_for_display};

use crate::widgets::message_widget::make_relative_path;
use ragent_types::ThinkingLevel;
use ragent_types::strutil::truncate_bytes;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Gauge, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, Wrap,
    },
};

use crate::layout_active_agents::render_active_agents_subpanel;

use crate::theme;
use crate::utils::{
    ResponsiveBreakpoint, centered_rect, centered_rect_max, is_below_minimum_size, shorten_middle,
};

use ragent_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use ragent_storage::storage::MemoryRow;

use crate::app::{
    App, ContextAction, LogLevel, ModelPickerEntry, OutputViewTarget, PROVIDER_LIST,
    ProviderSetupStep, SelectionPane,
};
use crate::widgets::message_widget::{
    canonical_tool_name, capitalize_tool_name, is_agent_notice, read_line_range,
    render_agent_notice_lines, tool_inline_diff, tool_input_summary, tool_result_summary,
};

/// Padding applied to each side of a content-sized table column.
const COLUMN_PADDING_CHARS: usize = 1;

/// Column header labels shared by the model-picker dialogs.
const MODEL_PICKER_HEADERS: [&str; 5] = ["Model", "Context", "Cost", "Thinking", "Features"];

/// Default spacing between table columns (matches `Table::column_spacing`).
const MODEL_PICKER_COLUMN_SPACING: usize = 1;

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
    let cost_str = format!("{} · {}", entry.cost_tier, entry.cost_multiplier);
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
        format!("▸ {}", entry.name)
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
    // Config-save picker overlay — rendered on top of chat, before history
    // picker so that whichever picker is open gets drawn last (i.e. on top).
    if app.config_save_picker.is_some() {
        render_config_save_picker(frame, app);
    }
    // History picker overlay — rendered on top of everything.
    if app.history_picker.is_some() {
        render_history_picker(frame, app);
    }
    // Run-cost summary banner (FR-012): a transient one-line overlay rendered
    // last so it sits above all other UI. Dismissed on the next keypress.
    if app.run_cost_banner.is_some() {
        render_run_cost_banner(frame, app);
    }
}

/// Render the transient run-complete banner as a centered one-line popup
/// near the top of the screen (FR-012).
///
/// The banner text is produced by the `Event::RunCostSummary` handler and
/// takes the form `⟡ run complete · {in}+{out} tokens · ${cost} · {dur}s`.
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

fn draw_input_side_buttons(frame: &mut Frame, app: &mut App, button_col_area: Rect) {
    let gap = 1u16;
    let button_w = ((button_col_area.width.saturating_sub(gap)) / 2).max(7);
    let agents_x = button_col_area.x;
    let teams_x = agents_x.saturating_add(button_w).saturating_add(gap);
    let y = button_col_area.y;
    let h = button_col_area.height.max(3);

    app.agents_button_area = Rect::new(agents_x, y, button_w, h);
    app.teams_button_area = Rect::new(teams_x, y, button_w, h);

    let agents_enabled = !app.active_tasks.is_empty() || !app.bg_tasks.is_empty();
    let teams_enabled = app.active_team.is_some();
    let agents_active = agents_enabled && app.show_agents_window;
    let teams_active = teams_enabled && app.show_teams_window;

    let agents_text_style = if agents_active {
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else if agents_enabled {
        Style::default().fg(Color::White)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };
    let teams_text_style = if teams_active {
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD)
    } else if teams_enabled {
        Style::default().fg(Color::White)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };

    let agents_border_style = if agents_active {
        Style::default().fg(Color::Blue).bg(Color::Blue)
    } else if agents_enabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };
    let teams_border_style = if teams_active {
        Style::default().fg(Color::Blue).bg(Color::Blue)
    } else if teams_enabled {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" Agents ", agents_text_style)))
            .style(agents_text_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(agents_border_style),
            ),
        app.agents_button_area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" Teams ", teams_text_style)))
            .style(teams_text_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(teams_border_style),
            ),
        app.teams_button_area,
    );
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

fn render_provider_setup_dialog(frame: &mut Frame, app: &mut App) {
    // Use a taller, capped dialog so longer provider/model lists fit without
    // clipping on typical terminal sizes.  80% height up to 30 rows gives the
    // provider picker enough room to show most entries.
    let area = centered_rect_max(60, 80, 80, 30, frame.area());
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
                Line::from(Span::styled(
                    format!("Loading models for {}", provider_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    format!("{} Fetching model list…", spinner),
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
                .title(format!(" {} ", provider_id))
                .border_style(Style::default().fg(Color::Cyan));
            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectProvider { selected, .. } => {
            // Split the dialog area into header, scrollable provider list, and footer.
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Provider Setup ")
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);

            // Determine which providers have saved credentials so we can show a ✓ tick.
            let configured_ids: std::collections::HashSet<String> =
                App::get_configured_providers(&app.storage)
                    .into_iter()
                    .map(|p| p.id)
                    .collect();

            // Build provider entry lines.
            let mut provider_lines: Vec<Line<'_>> = Vec::with_capacity(PROVIDER_LIST.len());
            for (i, (pid, pname)) in PROVIDER_LIST.iter().enumerate() {
                let (indicator, style) = if i == *selected {
                    (
                        "▸ ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                let tick = if configured_ids.contains(*pid) {
                    " ✓"
                } else {
                    ""
                };
                let badge = if *pid == "ollama" { " [local]" } else { "" };
                provider_lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(*pname, style),
                    Span::styled(badge, Style::default().fg(Color::Yellow)),
                    Span::styled(tick, Style::default().fg(Color::Green)),
                ]));
            }

            // Fixed header and footer consume 4 inner rows (2 header + 2 footer).
            const HEADER_ROWS: u16 = 2;
            const FOOTER_ROWS: u16 = 2;
            let provider_area_height = inner.height.saturating_sub(HEADER_ROWS + FOOTER_ROWS);
            let needs_scroll = provider_lines.len() > provider_area_height as usize;

            // Compute a scroll offset that keeps the selected provider visible.
            let max_offset = provider_lines
                .len()
                .saturating_sub(provider_area_height as usize);
            let half = (provider_area_height as usize) / 2;
            let scroll_offset = if *selected < half {
                0
            } else {
                (*selected - half).min(max_offset)
            };

            let visible_providers = if provider_area_height == 0 {
                &provider_lines[..0]
            } else {
                let end = (scroll_offset + provider_area_height as usize).min(provider_lines.len());
                &provider_lines[scroll_offset..end]
            };

            // Header text.
            let header_lines = vec![
                Line::from(Span::styled(
                    "Select a Provider",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            // Strong scrolling indicators: explicit arrows when clipped, with the
            // current position shown, so users always know the list is scrollable.
            let footer_lines = if needs_scroll {
                let total = provider_lines.len();
                let visible_start = scroll_offset + 1;
                let visible_end = (scroll_offset + visible_providers.len()).min(total);
                let scroll_hint = format!(
                    "▲ {} more above — {}–{} of {} ▼ {} more below",
                    scroll_offset,
                    visible_start,
                    visible_end,
                    total,
                    total.saturating_sub(visible_end)
                );
                vec![
                    Line::from(Span::styled(
                        "↑/↓ navigate  Enter select  Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(Span::styled(
                        scroll_hint,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                ]
            } else {
                vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "↑/↓ navigate  Enter select  Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ]
            }; // Layout: header, provider list, footer.
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(HEADER_ROWS),
                    Constraint::Min(0),
                    Constraint::Length(FOOTER_ROWS),
                ])
                .split(inner);

            frame.render_widget(Clear, area);
            frame.render_widget(block, area);
            frame.render_widget(
                Paragraph::new(header_lines).alignment(Alignment::Center),
                chunks[0],
            );
            frame.render_widget(
                Paragraph::new(visible_providers.to_vec()).alignment(Alignment::Center),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(footer_lines).alignment(Alignment::Center),
                chunks[2],
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
            // Wider dialog so the full API key (≥ 48 chars) is visible.
            let area = centered_rect_max(80, 80, 100, 30, frame.area());
            frame.render_widget(Clear, area);

            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    format!("Configure {}", provider_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Enter your API key:"),
                Line::from(""),
            ];

            // Show the key unmasked so the user can verify the full value.
            let key_text = key_field.text();
            let key_cursor_display = if *active_field == 0 {
                key_field.cursor()
            } else {
                key_text.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 0 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(key_text, key_cursor_display),
                    Style::default().fg(Color::White),
                ),
            ]));

            if provider_id == "generic_openai" || provider_id == "azure_foundry" {
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Endpoint URL (optional, e.g. http://localhost:11434/v1):",
                ));
                let ep_text = endpoint_field.text();
                let endpoint_cursor_display = if *active_field == 1 {
                    endpoint_field.cursor()
                } else {
                    ep_text.chars().count()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        if *active_field == 1 { "> " } else { "  " },
                        Style::default().fg(Color::Cyan),
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
                    Style::default().fg(Color::DarkGray),
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
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Enter API Key ")
                .border_style(Style::default().fg(Color::Cyan));

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
                Line::from(Span::styled(
                    header,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
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
                    Span::raw("Code: "),
                    Span::styled(
                        user_code.as_str(),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Waiting for authorisation…",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "c copy code  Esc cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Cyan));

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
            // terminals still fit.
            let cells = model_picker_cells(models, *selected);
            let inner_w = model_picker_inner_width(&cells) as u16;
            let area = centered_rect_max(100, 80, inner_w.saturating_add(2), 30, frame.area());
            frame.render_widget(Clear, area);

            if models.is_empty() {
                let paragraph = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No models are currently available for this provider.",
                        Style::default().fg(Color::Yellow),
                    )),
                    Line::from(""),
                    Line::from("Check provider setup, authentication, or model discovery."),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" Select Model - {} ", provider_name))
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .alignment(Alignment::Center);
                frame.render_widget(paragraph, area);
                return;
            }

            // Create header row
            let header = Row::new(MODEL_PICKER_HEADERS).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            );

            // Calculate visible rows based on available height
            let header_height = 3; // Header + border lines
            let footer_height = 3; // Footer hint + spacing
            let available_rows = area.height.saturating_sub(header_height + footer_height) as usize;
            let visible = available_rows.max(1).min(models.len());
            let start = if *selected >= visible {
                (*selected + 1).saturating_sub(visible)
            } else {
                0
            };
            let end = (start + visible).min(models.len());

            let rows: Vec<Row> = cells
                .iter()
                .enumerate()
                .skip(start)
                .take(end - start)
                .map(|(i, row_cells)| {
                    let is_selected = i == *selected;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Row::new(row_cells.clone()).style(style)
                })
                .collect();

            let table = Table::new(rows, content_sized_columns(&cells))
                .header(header)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" Select Model - {} ", provider_name))
                        .border_style(Style::default().fg(Color::Cyan)),
                );

            frame.render_widget(table, area);

            // Render footer hint at the bottom of the area
            if area.height > 2 {
                let hint = Span::styled(
                    "↑/↓ navigate  Enter select  Esc cancel",
                    Style::default().fg(Color::DarkGray),
                );
                let hint_line = Line::from(hint);
                let hint_area = Rect::new(
                    area.x + 2,
                    area.y + area.height - 2,
                    area.width.saturating_sub(4),
                    1,
                );
                frame.render_widget(Paragraph::new(hint_line), hint_area);
            }

            // Render "showing X of Y" if needed
            if models.len() > visible && area.height > 4 {
                let showing = Span::styled(
                    format!("Showing {}-{} of {}", start + 1, end, models.len()),
                    Style::default().fg(Color::DarkGray),
                );
                let showing_line = Line::from(showing);
                let showing_area = Rect::new(
                    area.x + 2,
                    area.y + area.height - 3,
                    area.width.saturating_sub(4),
                    1,
                );
                frame.render_widget(
                    Paragraph::new(showing_line).alignment(Alignment::Right),
                    showing_area,
                );
            }
        }
        ProviderSetupStep::SelectAzureResource {
            entries,
            selected,
            error,
        } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Select Azure Resource",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            if let Some(err) = error {
                lines.push(Line::from(Span::styled(
                    err.as_str(),
                    Style::default().fg(Color::Yellow),
                )));
                lines.push(Line::from(""));
            } else if entries.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No resources found.",
                    Style::default().fg(Color::Yellow),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Place an azureresources.json file in ~/.config/ragent/ or .ragent/",
                ));
            } else {
                for (i, entry) in entries.iter().enumerate() {
                    let (indicator, style) = if i == *selected {
                        (
                            "▸ ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        ("  ", Style::default().fg(Color::White))
                    };
                    lines.push(Line::from(vec![
                        Span::styled(indicator, style),
                        Span::styled(entry.name.clone(), style),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ to move, Enter to select, Esc to cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Select Azure Resource ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectThinkingLevel {
            model, selected, ..
        } => {
            let lines: Vec<Line<'_>> = model
                .thinking_levels
                .iter()
                .enumerate()
                .flat_map(|(i, level)| {
                    let style = if i == *selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
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
                        Line::from(vec![
                            Span::styled(if i == *selected { "▸ " } else { "  " }, style),
                            Span::styled(label, style),
                        ]),
                        Line::from(Span::styled(
                            format!("    {}", desc),
                            Style::default().fg(Color::DarkGray),
                        )),
                    ]
                })
                .collect();

            let mut content = vec![
                Line::from(Span::styled(
                    format!("Select thinking for {}", model.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            content.extend(lines);
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter select  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Select Thinking Level ")
                .border_style(Style::default().fg(Color::Cyan));
            frame.render_widget(
                Paragraph::new(content)
                    .block(block)
                    .alignment(Alignment::Left),
                area,
            );
        }
        ProviderSetupStep::Done {
            provider_name,
            model_name,
        } => {
            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "✓ Provider Configured",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(format!("{} is now ready to use.", provider_name)),
            ];

            if let Some(model) = model_name {
                lines.push(Line::from(format!("Model: {}", model)));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press any key to continue",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Success ")
                .border_style(Style::default().fg(Color::Green));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectAgent { agents, selected } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Select an Agent",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            for (i, (name, desc, is_custom)) in agents.iter().enumerate() {
                let is_current = i == app.current_agent_index;
                let (indicator, style) = if i == *selected {
                    (
                        "▸ ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                let current_marker = if is_current { " ●" } else { "" };
                let mut spans = vec![
                    Span::styled(indicator, style),
                    Span::styled(name.as_str(), style),
                ];
                if *is_custom {
                    spans.push(Span::styled(
                        " [custom]",
                        Style::default().fg(Color::Yellow),
                    ));
                }
                spans.push(Span::styled(
                    format!("  {}{}", desc, current_marker),
                    Style::default().fg(Color::DarkGray),
                ));
                lines.push(Line::from(spans));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter select  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Select Agent ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, area);
        }
        // Renderer for the configured-provider picker (used by `/model`).
        ProviderSetupStep::SelectConfiguredProvider {
            providers,
            selected,
        } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    " Switch Provider ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            for (i, prov) in providers.iter().enumerate() {
                let (indicator, style) = if i == *selected {
                    (
                        "▸ ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                let checkmark = " ✓";
                lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(format!("{}", prov.name), style),
                    Span::styled(checkmark, Style::default().fg(Color::Green)),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter select  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Switch Provider ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::ShowProviderConfig {
            providers,
            selected,
        } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    " Show Provider Configuration ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            for (i, prov) in providers.iter().enumerate() {
                let (indicator, style) = if i == *selected {
                    (
                        "▸ ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(format!("{}", prov.name), style),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter show config  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Provider Config ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::ResetProvider { selected } => {
            let active_id = app.configured_provider.as_ref().map(|p| p.id.as_str());
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Reset Provider Credentials",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            for (i, (pid, pname)) in PROVIDER_LIST.iter().enumerate() {
                let is_active = active_id == Some(*pid);
                let (indicator, style) = if i == *selected {
                    (
                        "▸ ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                let active_marker = if is_active { " ●" } else { "" };
                let badge = if *pid == "ollama" { " [local]" } else { "" };
                lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(*pname, style),
                    Span::styled(badge, Style::default().fg(Color::Yellow)),
                    Span::styled(active_marker, Style::default().fg(Color::Green)),
                ]));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter reset  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Provider Reset ")
                .border_style(Style::default().fg(Color::Yellow));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }

        // ── GitLab setup form ────────────────────────────────────────────
        ProviderSetupStep::GitLabSetup {
            url_input,
            url_cursor,
            token_input,
            token_cursor,
            active_field,
            error,
        } => {
            // Wider dialog so the full token (≥ 48 chars) is visible.
            let area = centered_rect_max(80, 80, 100, 30, frame.area());
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

            // Token field (unmasked so the user can verify the full value)
            let tok_cursor_display = if *active_field == 1 {
                *token_cursor
            } else {
                token_input.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 1 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(token_input, tok_cursor_display),
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
                    "Validating token…",
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

            // ── Left pane: multi-select provider list ──
            let left_block = Block::default()
                .borders(Borders::ALL)
                .title(if *left_pane_focused {
                    " Providers (*) "
                } else {
                    " Providers "
                })
                .border_style(if *left_pane_focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                });
            let _left_inner = left_block.inner(chunks[0]);
            let mut provider_lines: Vec<Line<'_>> = Vec::new();
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
                for (i, p) in providers.iter().enumerate() {
                    let is_selected = i == *selected_provider_index;
                    let in_cluster = selected_provider_ids.contains(&p.id);
                    let tick = if in_cluster { "[x]" } else { "[ ]" };
                    let (indicator, style) = if is_selected && *left_pane_focused {
                        (
                            "▸ ",
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
            let left_para = Paragraph::new(provider_lines)
                .block(left_block)
                .wrap(Wrap { trim: false });
            frame.render_widget(left_para, chunks[0]);

            // ── Right pane: four tier buckets in a 2×2 grid ──
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
                let _bucket_inner = bucket_block.inner(bucket_chunks[idx]);

                let mut bucket_lines: Vec<Line<'_>> = Vec::new();
                if models.is_empty() {
                    bucket_lines.push(Line::from(Span::styled(
                        "empty",
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for (midx, entry) in models.iter().enumerate() {
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

                        // Line 2: retained model properties — context window,
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
                                    format!("cost {}·{}", m.cost_tier, m.cost_multiplier),
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
                    }
                }
                let bucket_para = Paragraph::new(bucket_lines)
                    .block(bucket_block)
                    .wrap(Wrap { trim: false });
                frame.render_widget(bucket_para, bucket_chunks[idx]);
            }

            // Footer with hints/error.
            let footer_text = if let Some(err) = error {
                format!(
                    "Esc cancel | Tab switch pane | ↑↓ move | Space toggle | Enter assign | Ctrl+S save | Ctrl+↑↓ reorder | Del remove — Error: {err}"
                )
            } else {
                "Esc cancel | Tab switch pane | ↑↓ move | Space toggle provider | Enter assign | Ctrl+S save | Ctrl+↑↓ reorder | Del remove".to_string()
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
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Assign model for {} → {} ",
                    provider_name, target_tier
                ))
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            frame.render_widget(block.clone(), area);

            if models.is_empty() {
                let paragraph = Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "No models are currently available for this provider.",
                        Style::default().fg(Color::Yellow),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    )),
                ])
                .alignment(Alignment::Center);
                frame.render_widget(paragraph, inner);
            } else {
                // Render the same rich model-properties table used by the
                // standard model picker so users can compare context window,
                // cost, thinking levels, and feature flags before assigning a
                // model to a router tier bucket.
                let header = Row::new(MODEL_PICKER_HEADERS).style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Cyan),
                );

                let header_height = 3;
                let footer_height = 3;
                let available_rows =
                    inner.height.saturating_sub(header_height + footer_height) as usize;
                let visible = available_rows.max(1).min(models.len());
                let start = if *selected >= visible {
                    (*selected + 1).saturating_sub(visible)
                } else {
                    0
                };
                let end = (start + visible).min(models.len());

                let cells = model_picker_cells(models, *selected);
                let rows: Vec<Row> = cells
                    .iter()
                    .enumerate()
                    .skip(start)
                    .take(end - start)
                    .map(|(i, row_cells)| {
                        let is_selected = i == *selected;
                        let style = if is_selected {
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        };
                        Row::new(row_cells.clone()).style(style)
                    })
                    .collect();

                let table = Table::new(rows, content_sized_columns(&cells))
                    .header(header)
                    .block(block);
                frame.render_widget(table, area);

                // Footer hint.
                if inner.height > 1 {
                    let hint = Span::styled(
                        "↑/↓ navigate  Enter assign  Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    );
                    let hint_area = Rect::new(
                        inner.x,
                        inner.y + inner.height.saturating_sub(1),
                        inner.width,
                        1,
                    );
                    frame.render_widget(Paragraph::new(Line::from(hint)), hint_area);
                }

                if models.len() > visible && inner.height > 2 {
                    let showing = Span::styled(
                        format!("Showing {}-{} of {}", start + 1, end, models.len()),
                        Style::default().fg(Color::DarkGray),
                    );
                    let showing_area = Rect::new(
                        inner.x,
                        inner.y + inner.height.saturating_sub(2),
                        inner.width,
                        1,
                    );
                    frame.render_widget(
                        Paragraph::new(Line::from(showing)).alignment(Alignment::Right),
                        showing_area,
                    );
                }
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
                "▸ ",
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
                    "▸ ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else if entry.is_dir {
                ("  ", Style::default().fg(Color::Blue))
            } else {
                ("  ", Style::default().fg(Color::White))
            };
            let icon = if entry.is_dir { "📁 " } else { "📄 " };
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

/// Split `text` into fixed-width character-wrapped lines.
///
/// Unlike word wrapping, this breaks at exact character boundaries so that
/// cursor positioning via `pos / width` and `pos % width` is always correct.
fn char_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lines = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        lines.push(chars[start..end].iter().collect::<String>());
        start = end;
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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
) -> (usize, usize) {
    let inner_width = inner_width.max(1);
    let mut display_row = 0usize;
    let mut char_idx = 0usize;

    for (line_i, logical_line) in input.split('\n').enumerate() {
        let prefix_len = 2usize; // "> " or "  "
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

fn with_cursor_marker(text: &str, cursor: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let c = cursor.min(chars.len());
    let mut out = String::with_capacity(text.len() + 3);
    out.extend(chars[..c].iter());
    out.push('█');
    out.extend(chars[c..].iter());
    out
}

fn input_widget_height(input: &str, inner_width: usize) -> u16 {
    let num_lines = input_widget_lines(input, inner_width).len();
    (num_lines as u16).max(1) + 2 // +2 for borders
}

fn input_widget_lines(input: &str, inner_width: usize) -> Vec<String> {
    if inner_width == 0 {
        return vec![format!("> {}", input.replace('\n', " ↵ "))];
    }
    let mut result = Vec::new();
    for (i, logical_line) in input.split('\n').enumerate() {
        let prefix = if i == 0 { "> " } else { "  " };
        let prefixed = format!("{}{}", prefix, logical_line);
        let wrapped = char_wrap(&prefixed, inner_width);
        result.extend(wrapped);
    }
    if result.is_empty() {
        result.push("> ".to_string());
    }
    result
}

/// Wrap one rendered [`Line`] into styled display rows that match ratatui's
/// `Paragraph` word-wrapping (`Wrap { trim: false }`).
///
/// Rows are broken exactly where ratatui's `WordWrapper` (0.29
/// `widgets/reflow.rs`, `WordWrapper::process_input`) breaks them, and each
/// row keeps the original span styles.  Because no row ever exceeds the
/// target width, re-wrapping a row inside `Paragraph` is a no-op: the cache
/// geometry and the painted output stay in one coordinate system.
fn wrap_line_styled(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    if width == 0 {
        return vec![unbounded_line(line)];
    }

    let max = width;
    // Flat stream of (grapheme symbol, patched style) mirroring
    // `Paragraph::styled_graphemes` (line style patched with span style).
    let base = line.style;
    let graphemes: Vec<(String, ratatui::style::Style)> = line
        .spans
        .iter()
        .flat_map(|span| {
            let span_style = base.patch(span.style);
            span.content
                .graphemes(true)
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
        // ratatui 0.29 treats NBSP as a regular (non-whitespace) grapheme
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

    // Tail: emit whatever is still buffered for this input line (trim =
    // false keeps trailing whitespace on the row).
    if row.is_empty() && word.is_empty() && !ws.is_empty() {
        rows.push(vec![]);
    }
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
        if cur_style.is_some_and(|s| s != st) && !text.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut text), cur_style.unwrap()));
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

    for (line_i, logical_line) in input.split('\n').enumerate() {
        // Each logical line starts a new display line — flush a boundary marker.
        // We represent this as a "newline flush" by letting the chunker know when
        // to start a new display row; we do this by resetting a counter below.
        let prefix = if line_i == 0 { "> " } else { "  " };

        // Prefix chars: not selectable
        for c in prefix.chars() {
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
            // Logical line boundary — flush current display row.
            display_lines.push(std::mem::take(&mut current));
            col = 0;
            continue;
        }
        if col == inner_width {
            // Width wrap — flush and start new display row.
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
        display_lines.push(vec![('>', None), (' ', None)]);
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

/// Render the Telemetry side panel (toggled via `Alt+O`).
///
/// Displays the same live counter/gauge values as `/telemetry counters`, grouped
/// by category in a compact, scrollable side panel. The panel reads from the
/// shared [`TelemetryCountersContent`] builder so it stays in sync with the chat
/// output without duplicating metric definitions.
///
/// # Arguments
/// - `frame` — the ratatui frame to render into.
/// - `app` — mutable `App` state; reads `telemetry_scroll_offset`; writes
///   `telemetry_area`, `telemetry_max_scroll`, and `telemetry_content_lines`.
/// - `area` — the rect allocated to the panel by the side-panel split.
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
            // Compact line: "name value — type — description", matching the
            // `/telemetry counters` chat output as closely as the narrow panel
            // allows.
            lines.push(Line::from(vec![
                Span::styled(format!("{name} "), metric_style),
                Span::styled(value.to_string(), value_style),
                Span::styled(format!(" — {kind}"), type_style),
                Span::styled(format!(" — {desc}"), desc_style),
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

/// Render the Context side panel (toggled via `Alt+X`, contextpanel spec).
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
        "io: ↑{} ↓{}",
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

    // Use responsive button column width
    let button_col_w = breakpoint.button_column_width();
    let input_inner_width = chat_area
        .width
        .saturating_sub(button_col_w)
        .saturating_sub(2)
        .max(1) as usize;
    let input_height = input_widget_height(&app.input, input_inner_width);

    // Whether to show the teammate strip (1 row under the status bar).
    let team_strip = app.active_team.is_some() && !app.team_members.is_empty();
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
    crate::layout_statusbar::render_status_bar_v2(frame, app, chunks[0]);

    if team_strip {
        render_teammate_strip(frame, app, chunks[1]);
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
    }

    // Router save confirmation modal overlay
    // Must render after provider_setup so the confirmation sits on top of the
    // router setup dialog when the user presses Ctrl+S.
    if app.pending_router_save.is_some() {
        render_router_save_dialog(frame, app);
    }

    // MCP discover dialog overlay
    if app.mcp_discover.is_some() {
        render_mcp_discover_dialog(frame, app);
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
    } else {
        app.agents_close_button_area = Rect::default();
    }
    if app.show_teams_window {
        render_teams_window_overlay(frame, app);
    } else {
        app.teams_close_button_area = Rect::default();
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

    let title = format!(" Memory #{} — {} ", view.row.id, view.row.category);
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

    // ── Memory-view line cache (mirrors research view) ────────────────────
    let inner_width = inner.width.saturating_sub(2);
    let cache_width = inner_width;
    let need_rebuild =
        view.line_cache.cache_width != cache_width || view.line_cache.lines.is_empty();

    if need_rebuild {
        let mut lines: Vec<Line<'_>> = Vec::new();
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

        view.line_cache.lines = lines;
        view.line_cache.cache_width = cache_width;
        view.line_cache.wrapped_lines = view.line_cache.lines.clone();
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

    // ── Research-view line cache (mirrors render_messages) ──────────────────
    //
    // `markdown_to_lines` already wraps its output to the supplied width, so
    // the lines it returns are one display row each.  We cache those wrapped
    // rows and slice the visible window, then render without `.wrap()` so the
    // scroll geometry and painted rows cannot diverge.
    let inner_width = inner.width.saturating_sub(2);
    let cache_width = inner_width;
    let need_rebuild =
        view.line_cache.cache_width != cache_width || view.line_cache.lines.is_empty();

    if need_rebuild {
        let lines = markdown_to_lines(&view.markdown, &base, cache_width as usize);
        view.line_cache.lines = lines
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
        view.line_cache.wrapped_lines = view.line_cache.lines.clone();
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
                            "[Mermaid diagram — rendered as text below]"
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
        "[Esc to close · images shown as placeholders · links are plain text]".to_string(),
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
        "• "
    } else {
        "• "
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
            format!("{} Fetching model list…", spinner),
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
            format!("{:.1}% • {} elapsed", percent, format_elapsed(elapsed)),
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
    let m = secs / 60;
    let s = secs % 60;
    format!("{}:{:02}", m, s)
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

    // ── Per-entry line cache (mirrors render_messages) ───────────────────
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
    // fall into this path — previously-rendered groups keep their cached
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

    // NOTE: no `.wrap(...)` — mirrors render_messages.  The cached log rows
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

    // Parse and color the [sid:step] prefix in the message if present
    let msg = &entry.message;
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
                spans.push(Span::raw(msg.clone()));
            }
        } else {
            spans.push(Span::raw(msg.clone()));
        }
    } else {
        spans.push(Span::raw(msg.clone()));
    }
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
/// entire line is rendered in red and a `[blocked by #id, …]` annotation
/// is appended (FR-018). An `(owner)` suffix is appended when the task has
/// an owner set (FR-018). When a task is `in_progress` and has an
/// `active_form`, it is rendered as an indented sub-line beneath the
/// subject (FR-018). When no rows are returned the panel shows a `No tasks`
/// placeholder in dark gray; if the storage query fails the panel shows
/// `Failed to load tasks` in red and does not panic. A vertical scrollbar
/// is rendered when the row count exceeds the visible height.
///
/// # Arguments
/// - `frame` — the ratatui frame to render into.
/// - `app` — mutable `App` state; reads `session_id`, `tasks_scroll_offset`,
///   and `storage`; writes `tasks_area`, `tasks_max_scroll`, and
///   `tasks_content_lines`.
/// - `area` — the rect allocated to the panel by the side-panel split.
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
    // we must distinguish "no tasks" from "query failed" — the latter only
    // happens on the refresh attempt, so we can rely on cache emptiness
    // for the placeholder.
    let lines: Vec<Line> = if app.tasks_cache_rows.is_empty() {
        app.tasks_max_scroll = 0;
        vec![Line::from(Span::styled(
            "No tasks",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        // Compute derived DAG fields so we can show [blocked by …]
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

            // Append [blocked by #id, …] when derived blocked (FR-018).
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
                    format!("  → {active}"),
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
            let _ = app.storage.get_memory_tags(row.id);
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
    let area = centered_rect(90, 70, frame.area());
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

    let (title, target_session, team_filter): (
        String,
        Option<String>,
        Option<(String, String, String)>,
    ) = match &view.target {
        OutputViewTarget::Session { session_id, label } => {
            (format!(" Output: {label} "), Some(session_id.clone()), None)
        }
        OutputViewTarget::TeamMember {
            team_name,
            agent_id,
            teammate_name,
            session_id,
        } => (
            format!(" Output: {} [{}] ", teammate_name, agent_id),
            session_id.clone(),
            Some((team_name.clone(), agent_id.clone(), teammate_name.clone())),
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
    let inner = block.inner(area);

    // ── Output-view line cache (mirrors render_messages / render_log_panel) ─
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
    // displayed messages and log entries.  For the primary session we use
    // the in-memory messages (last message edit_seq); for storage-backed
    // sessions we fetch once and derive generation from the same result.
    // `app.log_seq` covers new log entries appended while the view is open.
    let (current_generation, session_messages) = {
        let mut generation = app.log_seq;
        // Mix in target identity so switching targets invalidates the cache.
        let target_seed = match &target_session {
            Some(sid) => {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hasher::write(&mut h, sid.as_bytes());
                std::hash::Hasher::finish(&h)
            }
            None => 0,
        };
        generation = generation.wrapping_add(target_seed);

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

        (generation, session_messages)
    };

    let cache_stale =
        view.line_cache.source_generation != current_generation || view.line_cache.lines.is_empty();

    if cache_stale || need_rewrap {
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

        for entry in app.log_entries.iter().filter(|entry| {
            if let Some((ref team_name, ref agent_id, ref teammate_name)) = team_filter {
                entry.message.contains(&format!("[{team_name}]"))
                    && (entry.message.contains(agent_id) || entry.message.contains(teammate_name))
            } else if let Some(ref sid) = target_session {
                entry.session_id.as_deref() == Some(sid.as_str())
                    || (entry.session_id.is_none()
                        && app.session_id.as_deref() == Some(sid.as_str()))
            } else {
                false
            }
        }) {
            let ts = entry.timestamp.format("%H:%M:%S");
            lines.push(Line::from(vec![
                Span::styled(format!("{ts} LOG "), Style::default().fg(Color::DarkGray)),
                Span::raw(entry.message.clone()),
            ]));
        }

        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "No output yet for this target",
                Style::default().fg(Color::DarkGray),
            )));
        }

        view.line_cache.lines = lines;
        view.line_cache.source_generation = current_generation;
    }

    // Re-wrap when the width changed or the source lines were rebuilt.
    if cache_stale || need_rewrap {
        view.line_cache.wrapped_lines = view
            .line_cache
            .lines
            .iter()
            .flat_map(|l| wrap_line_styled(l, w))
            .collect();
        view.line_cache.content_lines = wrapped_lines_to_strings(&view.line_cache.wrapped_lines);
        view.line_cache.wrapped_count = view.line_cache.wrapped_lines.len() as u16;
        view.line_cache.cache_width = inner_width;
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

fn render_agents_window_overlay(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(58, 56, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Agents ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let close_w = 10u16.min(inner.width);
    let close_h = 3u16.min(inner.height);
    let close_x = inner.right().saturating_sub(close_w);
    let close_y = inner.bottom().saturating_sub(close_h);
    let close_area = Rect::new(close_x, close_y, close_w, close_h);
    app.agents_close_button_area = close_area;

    let content_h = inner.height.saturating_sub(close_h + 1);
    let content_area = Rect::new(inner.x, inner.y, inner.width, content_h.max(1));
    app.active_agents_area = content_area;
    render_active_agents_subpanel(frame, app, content_area);

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

fn render_teams_window_overlay(frame: &mut Frame, app: &mut App) {
    let area = centered_rect(90, 56, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Teams ",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Blue));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let close_w = 10u16.min(inner.width);
    let close_h = 3u16.min(inner.height);
    let close_x = inner.right().saturating_sub(close_w);
    let close_y = inner.bottom().saturating_sub(close_h);
    let close_area = Rect::new(close_x, close_y, close_w, close_h);
    app.teams_close_button_area = close_area;

    let content_h = inner.height.saturating_sub(close_h + 1);
    let content_area = Rect::new(inner.x, inner.y, inner.width, content_h.max(1));
    app.teams_area = content_area;
    crate::layout_teams::render_teams_subpanel(frame, app, content_area);

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

/// Render a single-row teammate strip below the status bar.
///
/// Shows each teammate as a compact pill: `[icon] name (status)`.
/// The focused teammate is highlighted with a different background.
fn render_teammate_strip(frame: &mut Frame, app: &App, area: Rect) {
    use ragent_team::team::MemberStatus;

    let bg = Color::Rgb(30, 30, 40);
    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::styled(
        " 👥 ",
        Style::default().fg(Color::Blue).bg(bg),
    ));

    for member in &app.team_members {
        let is_focused = app.focused_teammate.as_ref() == Some(&member.agent_id);

        let (status_icon, status_color) = match member.status {
            MemberStatus::Working => ("▶", Color::Cyan),
            MemberStatus::Idle => ("●", Color::Green),
            MemberStatus::Spawning => ("◌", Color::Yellow),
            MemberStatus::Blocked => ("◈", Color::DarkGray),
            MemberStatus::PlanPending => ("◎", Color::Magenta),
            MemberStatus::Suspended => ("[pause]", Color::DarkGray),
            MemberStatus::ShuttingDown => ("◌", Color::Yellow),
            MemberStatus::Stopped => ("○", Color::DarkGray),
            MemberStatus::Failed => ("✗", Color::Red),
        };

        let pill_bg = if is_focused {
            Color::Rgb(50, 50, 80)
        } else {
            bg
        };
        let name_color = if is_focused {
            Color::White
        } else {
            Color::Gray
        };
        let border_char = if is_focused { "▸" } else { " " };

        spans.push(Span::styled(
            format!("{border_char}{status_icon} "),
            Style::default().fg(status_color).bg(pill_bg),
        ));
        // Truncate name to 16 chars
        let display_name: String = member.name.chars().take(16).collect();
        spans.push(Span::styled(
            display_name,
            Style::default()
                .fg(name_color)
                .bg(pill_bg)
                .add_modifier(if is_focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
        spans.push(Span::styled(" ", Style::default().bg(bg)));
    }

    // Hint at right edge
    let hint = " Alt+↑↓:cycle ";
    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let remaining = (area.width as usize).saturating_sub(used + hint.len());
    spans.push(Span::styled(" ".repeat(remaining), Style::default().bg(bg)));
    spans.push(Span::styled(
        hint,
        Style::default().fg(Color::DarkGray).bg(bg),
    ));

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(bg));
    frame.render_widget(bar, area);
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

                    let (dot, dot_style, indent) = match msg.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        Role::Assistant | Role::Compaction => (
                            "● ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            2,
                        ),
                    };
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(dot, dot_style),
                                Span::raw(line.to_owned()),
                            ]));
                        } else {
                            lines.push(Line::from(Span::raw(format!(
                                "{}{}",
                                " ".repeat(indent),
                                line
                            ))));
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
                            "● ",
                            Style::default().fg(Color::Green),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Error => (
                            "✗ ",
                            Style::default().fg(Color::Red),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "● ",
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
                    if canonical_tool_name(tool) == "think" || summary.is_empty() {
                        spans.push(Span::styled(display_name, name_style));
                    } else {
                        // Extract icon (emoji + space) from the beginning of summary
                        let mut parts = summary.splitn(2, ' ');
                        let icon = parts.next().unwrap_or("");
                        let rest = parts.next().unwrap_or("");
                        if !icon.is_empty() {
                            let icon_style = if canonical_tool_name(tool) == "think" {
                                theme::think_summary()
                            } else {
                                Style::default().fg(Color::DarkGray)
                            };
                            spans.push(Span::styled(format!("{} ", icon), icon_style));
                        }
                        spans.push(Span::styled(format!("{} ", display_name), name_style));
                        if !rest.is_empty() {
                            let summary_style = if canonical_tool_name(tool) == "think" {
                                theme::think_summary()
                            } else {
                                Style::default().fg(Color::DarkGray)
                            };
                            spans.push(Span::styled(rest.to_string(), summary_style));
                        }
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
                            {
                                for line in thought.lines() {
                                    lines.push(Line::from(Span::styled(
                                        format!("  💭 {}", line),
                                        theme::think(),
                                    )));
                                }
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
                            if summary.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "  └ [ok] Task complete",
                                    Style::default().fg(Color::Green),
                                )));
                            } else {
                                for line in summary.lines() {
                                    lines.push(Line::from(Span::styled(
                                        format!("  └ [ok] {line}"),
                                        Style::default().fg(Color::Green),
                                    )));
                                }
                            }
                        } else if tool != "edit"
                            && let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, cwd)
                        {
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", result),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                    if state.status == ToolCallStatus::Error {
                        if let Some(ref err) = state.error {
                            lines.push(Line::from(Span::styled(
                                format!("  └ {}", err),
                                Style::default().fg(Color::Red),
                            )));
                        }
                    }
                }
                MessagePart::Reasoning { text } => {
                    for line in text.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  💭 {}", line),
                            theme::think(),
                        )));
                    }
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

    // ── Per-message line cache (FR-003, FR-006) ──────────────────────────
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
    if app.message_line_cache.len() > messages_to_show.len() {
        app.message_line_cache.truncate(messages_to_show.len());
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

    // Re-render stale groups.  A group is stale when its cached `edit_seq`
    // no longer matches the message's current `edit_seq`, or when the slot
    // was just created and has no lines yet.  When only the last message
    // was modified by streaming (append_assistant_text,
    // update_tool_call_status, etc.) only that one group is re-rendered.
    let w = inner_width.max(1) as usize;
    for (i, msg) in messages_to_show.iter().enumerate() {
        let group = &mut app.message_line_cache[i];
        if group.edit_seq != msg.edit_seq || group.lines.is_empty() {
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
        }
    }

    // Re-wrap all groups when the width changed (FR-003).
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
    }

    // Accumulate the total wrapped line count and collect the content lines
    // used for text-selection copy.  The full `all_lines` vector is never
    // materialised any more (see the scroll-window slice below) — only the
    // visible window is passed to ratatui.
    let mut all_content_lines: Vec<String> = Vec::new();
    let mut total_wrapped: u16 = 0;
    for group in app.message_line_cache.iter() {
        all_content_lines.extend(group.content_lines.iter().cloned());
        total_wrapped = total_wrapped.saturating_add(group.wrapped_count);
    }

    // Store the flattened content lines for text-selection copy.
    app.message_content_lines = all_content_lines;

    // Compute scroll geometry from the cached wrapped counts (cheap — no
    // re-wrapping involved).
    let session_display = app
        .session_id
        .as_deref()
        .map(|s| &s[..8.min(s.len())])
        .unwrap_or("none");
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

    // ── Scroll-window slice (idle-CPU fix) ────────────────────────────────
    //
    // Handing ratatui a Paragraph containing the ENTIRE transcript with
    // `.scroll((offset, 0))` makes Paragraph::render re-run WordWrapper and
    // unicode-width measurement over EVERY line on EVERY frame (ratatui
    // 0.29 only skips work for the no-wrap path).  With hundreds of
    // messages that is millions of width computations per frame, which
    // pinned a core even when the agent was idle.
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
    // the inner width, and ratatui 0.29 re-wrapping them with `Wrap { trim:
    // false }` would split whitespace-only rows (a whitespace-only input line
    // paints as a blank row PLUS a row of spaces) and add one phantom row per
    // such row to the painted output.  The scroll window and the geometry are
    // both derived from the cache, so painting must consume exactly one row
    // per cached row: without `.wrap`, the LineTruncator path renders each
    // cached row verbatim (rows never exceed the inner width) and the two
    // coordinate systems cannot diverge.
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

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2).max(1) as usize;

    // Build title: show focused teammate or staged attachments in the block title.
    let (title, title_style) = if let Some(ref focused_id) = app.focused_teammate {
        let name = app
            .team_members
            .iter()
            .find(|m| m.agent_id == *focused_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        (
            format!(" → {name} (focused) "),
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
        .border_style(Style::default().fg(if app.is_input_blocked() {
            Color::Red
        } else {
            Color::White
        }));

    if app.input.is_empty() {
        // Show "> " prompt with dimmed placeholder text so the line doesn't jump.
        let ghost = Line::from(vec![
            Span::raw("> "),
            Span::styled(INPUT_PLACEHOLDER, Style::default().fg(Color::DarkGray)),
        ]);
        let paragraph = Paragraph::new(ghost).block(block);
        frame.render_widget(paragraph, area);
        // Cursor sits right after the "> " prefix.
        frame.set_cursor_position((area.x + 1 + 2, area.y + 1));
    } else {
        let kb_sel = app.kb_selection_char_range();
        let wrapped_lines = input_lines_with_kb_selection(&app.input, inner_width, kb_sel);
        let paragraph = Paragraph::new(wrapped_lines).block(block);
        frame.render_widget(paragraph, area);

        // Position cursor accounting for wrapped lines.
        // Use the character index (not byte length) so unicode content behaves.
        let (cursor_line, cursor_col) =
            input_cursor_display_pos(&app.input, app.input_cursor, inner_width);
        let cursor_x = area.x + 1 + cursor_col as u16;
        let cursor_y = area.y + 1 + cursor_line as u16;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// All documented keybindings: (keys column, description column).
const KEYBINDINGS: &[(&str, &str)] = &[
    // ── Typing ──────────────────────────────────────────────────────────
    ("@", "Mention a file — opens file picker"),
    ("/", "Slash command — opens command menu"),
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
    ("Alt+T", "Toggle Tasks panel visibility"),
    ("Alt+O", "Toggle telemetry panel visibility"),
    ("Alt+X", "Toggle context panel visibility"),
    ("Alt+Y", "Toggle YOLO mode (bypass safety checks)"),
    // ── Sending ─────────────────────────────────────────────────────────
    ("Enter", "Send message / confirm"),
    ("Ctrl+C, Ctrl+D", "Quit application (guarded sequence)"),
    // ── Navigation ──────────────────────────────────────────────────────
    ("Shift+↑ / PageUp", "Scroll messages up"),
    ("Shift+↓ / PageDown", "Scroll messages down"),
    ("↑ / ↓", "Browse input history"),
    ("Ctrl+PageUp", "Scroll log panel up"),
    ("Ctrl+PageDown", "Scroll log panel down"),
    ("PageUp / PageDown", "Scroll opened output overlay"),
    ("Ctrl+PageUp/PageDown", "Output overlay: jump start/end"),
    // ── Agent ────────────────────────────────────────────────────────────
    ("Tab", "Cycle to next agent"),
    ("Esc / Ctrl+X", "Cancel running agent (while processing)"),
    // ── Teams ────────────────────────────────────────────────────────────
    ("Alt+↓", "Focus next teammate"),
    ("Alt+↑", "Focus previous teammate (or clear focus)"),
    // ── Dialogs ──────────────────────────────────────────────────────────
    ("Esc", "Close any open dialog or menu"),
    ("y / a / n", "Allow / Always / Deny permission request"),
];

fn render_shortcuts_panel(frame: &mut Frame) {
    let full = frame.area();
    // Responsive sizing: up to 80 wide, up to (rows+2) tall, capped at screen.
    let w = 80u16.min(full.width.saturating_sub(4));
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

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
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
        let question_lines = req.question.lines().count().max(1);
        let option_count = req.options.len();
        let total_lines = question_lines + option_count + 7;
        let height_percent = ((total_lines * 100) / frame.area().height as usize)
            .min(60)
            .max(20) as u16;
        let area = centered_rect(70, height_percent, frame.area());
        frame.render_widget(Clear, area);

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
            let prefix = if is_selected { "▶ " } else { "  " };
            text.push(Line::from(Span::styled(format!("{prefix}{option}"), style)));
        }

        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            "↑/↓ or j/k to navigate  Enter to select  Esc to dismiss",
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

    let input_display = format!("▶ {}_", app.pending_question_input);
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

/// Render the interactive MCP discovery dialog overlay.
fn render_mcp_discover_dialog(frame: &mut Frame, app: &App) {
    let Some(state) = app.mcp_discover.as_ref() else {
        return;
    };

    // Size the dialog: taller when there are more servers.
    let server_rows = state.servers.len().max(1) as u16;
    let dialog_height = (server_rows + 10).min(40); // header + rows + prompt + padding
    let area = {
        let full = frame.area();
        let h = dialog_height.min(full.height.saturating_sub(4));
        let w = full.width.min(90);
        ratatui::layout::Rect {
            x: (full.width.saturating_sub(w)) / 2,
            y: (full.height.saturating_sub(h)) / 2,
            width: w,
            height: h,
        }
    };
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "MCP Server Discovery",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if state.servers.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No MCP servers detected.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Install MCP servers via npm (e.g. @modelcontextprotocol/server-filesystem)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  or place configs in ~/.mcp/servers/ and retry.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Load current config once so we can flag already-enabled servers.
        let enabled_ids: std::collections::HashSet<String> = ragent_agent::Config::load()
            .map(|c| c.mcp.into_keys().collect())
            .unwrap_or_default();

        // Column header
        lines.push(Line::from(vec![Span::styled(
            format!("  {:<3}  {:<20}  {:<40}  {}", "#", "ID", "Name", "Source"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(Span::styled(
            format!("  {}", "─".repeat(80)),
            Style::default().fg(Color::DarkGray),
        )));

        for (i, srv) in state.servers.iter().enumerate() {
            let already_enabled = enabled_ids.contains(&srv.id);
            let num = format!("{}", i + 1);
            let name = ragent_types::truncate_bytes(&srv.name, 37);
            let source = match &srv.source {
                ragent_agent::mcp::McpDiscoverySource::SystemPath => "PATH".to_string(),
                ragent_agent::mcp::McpDiscoverySource::NpmGlobal { .. } => "npm global".to_string(),
                ragent_agent::mcp::McpDiscoverySource::McpRegistry { .. } => {
                    "MCP registry".to_string()
                }
            };
            let (num_color, id_color, name_color, source_color) = if already_enabled {
                // Yellow tones for already-configured servers
                (Color::Yellow, Color::Yellow, Color::Yellow, Color::Yellow)
            } else {
                (Color::Magenta, Color::White, Color::Green, Color::DarkGray)
            };
            let enabled_tag = if already_enabled { " ✓" } else { "" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<3}", num),
                    Style::default().fg(num_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {:<20}", format!("{}{}", srv.id, enabled_tag)),
                    Style::default().fg(id_color),
                ),
                Span::styled(format!("  {:<40}", name), Style::default().fg(name_color)),
                Span::styled(format!("  {}", source), Style::default().fg(source_color)),
            ]));
        }

        lines.push(Line::from(Span::styled(
            "  (yellow = already enabled in ragent.json)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));

    // Feedback line (error or success)
    if let Some(ref msg) = state.feedback {
        let color = if msg.starts_with('✓') {
            Color::Green
        } else {
            Color::Red
        };
        lines.push(Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(color),
        )));
        lines.push(Line::from(""));
    }

    // Input prompt
    if state.servers.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Press Esc to close",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  Enable server #: ", Style::default().fg(Color::White)),
            Span::styled(
                with_cursor_marker(state.number_input.as_str(), state.number_cursor),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            "  Enter to enable  Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /mcp discover ")
        .border_style(Style::default().fg(Color::Magenta));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
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
                    "{}…",
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
        " History ({} entries) — ↑/↓ navigate · Enter select · Esc close ",
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
                    "{}…",
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
        " Saved configurations ({} entries) — ↑/↓ navigate · Enter restore · Esc close ",
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
mod tests {
    use super::messages_to_lines;
    use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_messages_to_lines_renders_agent_notice_bright_yellow_one_line_per_item() {
        let message = Message::new(
            "s1",
            Role::Assistant,
            vec![MessagePart::Text {
                text: "📋 Agent Notice\nFirst item\nSecond item".to_string(),
            }],
        );

        let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
        let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

        // Header plus each list item gets its own line.
        assert!(rendered.iter().any(|line| line.contains("📋 Agent Notice")));
        assert!(rendered.iter().any(|line| line.contains("First item")));
        assert!(rendered.iter().any(|line| line.contains("Second item")));

        // All notice lines are styled bright yellow + bold.
        for line in &lines {
            for span in line.spans.iter() {
                assert_eq!(span.style.fg, Some(ratatui::style::Color::Yellow));
                assert!(
                    span.style
                        .add_modifier
                        .contains(ratatui::style::Modifier::BOLD)
                );
            }
        }
    }

    #[test]
    fn test_messages_to_lines_renders_full_thinktool_output_multiline() {
        let message = Message::new(
            "s1",
            Role::Assistant,
            vec![MessagePart::ToolCall {
                tool: "think".to_string(),
                call_id: "call-1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"thought": "First line.\nSecond line."}),
                    output: Some(json!({"thought": "First line.\nSecond line."})),
                    error: None,
                    duration_ms: Some(42),
                },
            }],
        );

        let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
        let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

        assert!(
            rendered.iter().any(|line| line.contains("Think")),
            "Expected tool header line in rendered output: {rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .filter(|line| line.contains("Think"))
                .all(|line| !line.contains("First line.")),
            "Expected think header to omit inline thought summary: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  💭 First line."),
            "Expected first thought line in rendered output: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|line| line == "  💭 Second line."),
            "Expected second thought line in rendered output: {rendered:?}"
        );
    }

    #[test]
    fn test_messages_to_lines_renders_full_agent_complete_output_multiline() {
        let message = Message::new(
            "s1",
            Role::Assistant,
            vec![MessagePart::ToolCall {
                tool: "agent_complete".to_string(),
                call_id: "call-1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: json!({"summary": "First line.\nSecond line."}),
                    output: Some(json!({
                        "agent_complete": true,
                        "summary": "First line.\nSecond line."
                    })),
                    error: None,
                    duration_ms: Some(42),
                },
            }],
        );

        let lines = messages_to_lines(&[message], &HashMap::new(), &HashMap::new(), "/project");
        let rendered: Vec<String> = lines.iter().map(ToString::to_string).collect();

        // Each summary line must be rendered as its own ratatui Line. A single
        // Line containing the whole summary would lose the '\n' graphemes
        // (ratatui filters them out of Spans), collapsing the summary into one
        // visual paragraph.
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("  └ [ok] First line.")),
            "Expected first summary line on its own rendered line: {rendered:?}"
        );
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("  └ [ok] Second line.")),
            "Expected second summary line on its own rendered line: {rendered:?}"
        );
        assert!(
            !rendered
                .iter()
                .any(|line| { line.contains("First line.") && line.contains("Second line.") }),
            "Summary lines must not be joined into a single Line: {rendered:?}"
        );
    }
}
