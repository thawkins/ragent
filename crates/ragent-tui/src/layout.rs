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
use crate::utils::{ResponsiveBreakpoint, centered_rect, centered_rect_max, is_below_minimum_size};

use ragent_agent::message::{Message, MessagePart, Role, ToolCallStatus};

use crate::app::{
    App, ContextAction, LogLevel, OutputViewTarget, PROVIDER_LIST, ProviderSetupStep, SelectionPane,
};
use crate::widgets::message_widget::{
    canonical_tool_name, capitalize_tool_name, read_line_range, tool_inline_diff,
    tool_input_summary, tool_result_summary,
};

fn shorten_middle(s: &str, max_chars: usize) -> String {
    let total = s.chars().count();
    if total <= max_chars {
        return s.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let keep_left = (max_chars - 1) / 2;
    let keep_right = max_chars - 1 - keep_left;
    let left: String = s.chars().take(keep_left).collect();
    let right: String = s.chars().skip(total.saturating_sub(keep_right)).collect();
    format!("{left}…{right}")
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
    // History picker overlay — rendered on top of everything.
    if app.history_picker.is_some() {
        render_history_picker(frame, app);
    }
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

    let agents_enabled = !app.active_tasks.is_empty();
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

fn render_provider_setup_dialog(frame: &mut Frame, app: &App) {
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
        ProviderSetupStep::SelectProvider { selected } => {
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

            // Show masked key input
            let key_text = key_field.text();
            let masked = if key_text.is_empty() {
                String::new()
            } else {
                let char_count = key_text.chars().count();
                if char_count <= 8 {
                    "*".repeat(char_count)
                } else {
                    let first4: String = key_text.chars().take(4).collect();
                    let last4: String = key_text
                        .chars()
                        .rev()
                        .take(4)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    format!("{}…{}", first4, last4)
                }
            };
            let key_cursor_display = if *active_field == 0 {
                key_field.cursor()
            } else {
                masked.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 0 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(&masked, key_cursor_display),
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
            user_code,
            verification_uri,
        } => {
            let lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "GitHub Copilot Authorisation",
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
                .title(" Copilot Sign In ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::SelectModel {
            provider_name,
            models,
            selected,
            ..
        } => {
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
            let header = Row::new(vec!["Model", "Context", "Cost", "Thinking", "Features"]).style(
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

            let rows: Vec<Row> = models
                .iter()
                .enumerate()
                .skip(start)
                .take(end - start)
                .map(|(i, entry)| {
                    let is_selected = i == *selected;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    // Format context window
                    let ctx_str = if entry.context_window >= 1_000_000 {
                        format!("{}M", entry.context_window / 1_000_000)
                    } else if entry.context_window >= 1_000 {
                        format!("{}K", entry.context_window / 1_000)
                    } else {
                        entry.context_window.to_string()
                    };

                    // Format cost: display tier (Free, Low, Medium, etc.) and multiplier (0x, 1x, 3x, etc.)
                    let cost_str = format!("{} · {}", entry.cost_tier, entry.cost_multiplier);
                    let thinking_str = App::format_thinking_levels(&entry.thinking_levels);

                    // Format features
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

                    // Add selection indicator
                    let model_name = if is_selected {
                        format!("▸ {}", entry.name)
                    } else {
                        format!("  {}", entry.name)
                    };

                    Row::new(vec![
                        model_name,
                        ctx_str,
                        cost_str,
                        thinking_str,
                        features_str,
                    ])
                    .style(style)
                })
                .collect();

            let table = Table::new(
                rows,
                [
                    Constraint::Percentage(35), // Model name
                    Constraint::Percentage(15), // Context window
                    Constraint::Percentage(20), // Cost
                    Constraint::Percentage(18), // Thinking
                    Constraint::Percentage(12), // Features
                ],
            )
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

            // Token field (masked)
            let masked = if token_input.is_empty() {
                String::new()
            } else {
                let char_count = token_input.chars().count();
                if char_count <= 8 {
                    "*".repeat(char_count)
                } else {
                    let first4: String = token_input.chars().take(4).collect();
                    let last4: String = token_input
                        .chars()
                        .rev()
                        .take(4)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    format!("{}…{}", first4, last4)
                }
            };
            let tok_cursor_display = if *active_field == 1 {
                *token_cursor
            } else {
                masked.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if *active_field == 1 { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(&masked, tok_cursor_display),
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

            // ── Right pane: four bucket columns ──
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

            let bucket_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                    Constraint::Percentage(25),
                ])
                .split(right_inner);

            for (idx, tier) in ragent_llm::providers::router_config::Tier::all()
                .iter()
                .enumerate()
            {
                let is_active = *active_bucket == *tier;
                let tier_config = draft_config.tiers.get(&tier.to_string());
                let models = tier_config.map(|t| t.models.as_slice()).unwrap_or(&[]);

                let bucket_block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " {} {} ",
                        tier.initial(),
                        if is_active && !left_pane_focused {
                            "*"
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
                        let cost = app
                            .estimate_entry_cost(&entry.provider, &entry.model)
                            .unwrap_or_default();
                        let cost_span = if cost.is_empty() {
                            Span::styled(String::new(), style)
                        } else {
                            Span::styled(
                                format!("  {}", cost),
                                Style::default().fg(Color::DarkGray),
                            )
                        };
                        bucket_lines.push(Line::from(vec![
                            Span::styled(format!("{}", entry.provider), style),
                            Span::styled(
                                format!(" / {} ", entry.model),
                                Style::default().fg(Color::DarkGray),
                            ),
                            cost_span,
                        ]));
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
                    "Esc cancel | Tab switch pane | ↑↓ move | Space toggle | Enter assign | Ctrl+S save | Ctrl+↑↓ reorder — Error: {err}"
                )
            } else {
                "Esc cancel | Tab switch pane | ↑↓ move | Space toggle provider | Enter assign | Ctrl+S save | Ctrl+↑↓ reorder".to_string()
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
            let area = centered_rect(60, 70, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Assign model for {} → {} ",
                    provider_name, target_tier
                ))
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let mut lines: Vec<Line<'_>> = Vec::new();
            if models.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No models available.",
                    Style::default().fg(Color::Yellow),
                )));
            } else {
                for (i, m) in models.iter().enumerate() {
                    let is_selected = i == *selected;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    lines.push(Line::from(vec![
                        Span::styled(if is_selected { "▸ " } else { "  " }, style),
                        Span::styled(m.name.clone(), style),
                        Span::styled(
                            format!(" ({}) [{} tokens]", m.id, m.context_window),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Esc cancel | ↑↓ select | Enter assign",
                Style::default().fg(Color::DarkGray),
            )));

            let paragraph = Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, inner);
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
fn build_wrapped_content_lines(lines: &[Line<'_>], inner_width: usize) -> Vec<String> {
    let mut result = Vec::new();
    for line in lines {
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
    result
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

/// Render input text as styled ratatui `Line`s with keyboard-selection highlighting.
///
/// Characters within `selection` (a `[start, end)` char-index range) are
/// rendered with a blue background. Prefix characters (`"> "` / `"  "`) are
/// never considered part of the selection.
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
    // reachable via the `/log` + `/profile` slash commands. The TODO panel is a
    // third sibling (FR-004, FR-012) and is rendered alone in the side column
    // when `show_todo` is true.
    if app.show_log || app.show_profile || app.show_todo || app.show_memory {
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

        // The TODO panel is mutually exclusive with log/profile/memory
        // (FR-012), so it gets its own branch that renders alone in the side
        // column.
        if app.show_todo {
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.memory_area = Rect::default();
            app.todo_area = h_chunks[1];
            render_todo_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::Todo, h_chunks[1]);
        } else if app.show_memory {
            // The Memory panel is mutually exclusive with log/profile/todo
            // (FR-004), so it gets its own branch that renders alone in the
            // side column. Clearing the other side-panel areas ensures mouse
            // hit-testing and scrollbar drag dispatch never target a panel
            // that is not actually visible.
            app.log_area = Rect::default();
            app.profile_area = Rect::default();
            app.active_agents_area = Rect::default();
            app.teams_area = Rect::default();
            app.todo_area = Rect::default();
            app.memory_area = h_chunks[1];
            render_memory_panel(frame, app, h_chunks[1]);
            apply_selection_highlight(frame, app, SelectionPane::Memory, h_chunks[1]);
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
                    render_profile_panel(frame, app, h_chunks[1]);
                    apply_selection_highlight(frame, app, SelectionPane::Profile, h_chunks[1]);
                }
                (false, false) => {
                    app.memory_area = Rect::default();
                }
            }
        }
    } else {
        app.message_area = chunks[2];
        app.log_area = Rect::default();
        app.profile_area = Rect::default();
        app.todo_area = Rect::default();
        app.memory_area = Rect::default();
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

    // Memory browser overlay
    if app.memory_browser.is_some() {
        crate::panels::render_memory_browser(frame, app);
    } else {
        app.memory_browser_close_area = Rect::default();
        app.memory_browser_area = Rect::default();
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
    let lines = markdown_to_lines(
        &view.markdown,
        &base,
        frame.area().width.saturating_sub(4) as usize,
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
    let inner = block.inner(area);
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    let total = paragraph.line_count(inner.width) as u16;
    let visible = inner.height;
    view.max_scroll = total.saturating_sub(visible);
    view.scroll_offset = view.scroll_offset.min(view.max_scroll);

    frame.render_widget(paragraph.scroll((view.scroll_offset, 0)), area);

    if total > visible {
        let mut sb_state =
            ScrollbarState::new(view.max_scroll as usize).position(view.scroll_offset as usize);
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
                } else if link_state.is_some() {
                    link_state.as_mut().unwrap().1.push_str(&text);
                } else if image_state.is_some() {
                    image_state.as_mut().unwrap().1.push_str(&text);
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
            format!("Downloading {}", shorten_middle(&state.model_id, 32)),
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

    // Build lines from all log entries
    let lines: Vec<Line> = all_entries
        .iter()
        .map(|entry| {
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
                        let display_sid = app
                            .sid_to_display_name
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
            Line::from(spans)
        })
        .collect();

    // Cache plain-text content for text selection copy
    // Must match the word-wrapped display that Paragraph renders
    let log_inner_width = log_inner.width as usize;
    app.log_content_lines = build_wrapped_content_lines(&lines, log_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

    // Use the rendered (wrapped) line count so the scroll reaches the true
    // bottom. `line_count(width)` accounts for word-wrapping; `lines.len()`
    // only counts logical lines and under-scrolls when entries are long.
    let total_lines = paragraph.line_count(log_inner.width) as u16;
    let visible_height = log_inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.log_max_scroll = max_scroll;
    let scroll = app.log_scroll_offset.min(max_scroll);

    let paragraph = paragraph.scroll((max_scroll.saturating_sub(scroll), 0));

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

/// Render the TODO side panel.
///
/// Lists the TODO items belonging to the active session (`app.session_id`),
/// re-queried from `Storage::get_todos` on each frame so edits made via the
/// `todo_write` tool or `/todo` slash command are reflected without a toggle
/// (FR-014). Each row is rendered as `[<STATUS>] <title>` with the status
/// prefix coloured according to FR-007. When no rows are returned the panel
/// shows a `No TODO items` placeholder in dark gray (FR-005); if the storage
/// query fails the panel shows `Failed to load TODOs` in red and does not
/// panic (NFR-005). A vertical scrollbar is rendered when the row count
/// exceeds the visible height (FR-008).
///
/// # Arguments
/// - `frame` — the ratatui frame to render into.
/// - `app` — mutable `App` state; reads `session_id`, `todo_scroll_offset`,
///   and `storage`; writes `todo_area`, `todo_max_scroll`, and
///   `todo_content_lines`.
/// - `area` — the rect allocated to the panel by the side-panel split.
fn render_todo_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " TODO ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.todo_area = area;

    // Resolve the session to display TODOs for. Fall back to the primary
    // session id when no specific agent session is selected, mirroring the
    // log panel's session-resolution logic.
    let session_id = app
        .selected_agent_session_id
        .clone()
        .or_else(|| app.session_id.clone());

    let rows_result: anyhow::Result<Vec<ragent_storage::TodoRow>> = match &session_id {
        Some(sid) => app
            .storage
            .get_todos(sid, None)
            .map_err(|e| anyhow::anyhow!(e.to_string())),
        None => Ok(Vec::new()),
    };

    let lines: Vec<Line> = match rows_result {
        Ok(rows) if rows.is_empty() => {
            app.todo_max_scroll = 0;
            vec![Line::from(Span::styled(
                "No TODO items",
                Style::default().fg(Color::DarkGray),
            ))]
        }
        Ok(rows) => rows
            .iter()
            .map(|row| {
                let status_upper = row.status.to_uppercase();
                let status_color = match status_upper.as_str() {
                    "PENDING" => Color::Yellow,
                    "IN_PROGRESS" => Color::Cyan,
                    "DONE" => Color::Green,
                    "BLOCKED" => Color::Red,
                    _ => Color::DarkGray,
                };
                Line::from(vec![
                    Span::styled(
                        format!("[{status_upper}] "),
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(row.title.clone()),
                ])
            })
            .collect(),
        Err(_) => {
            app.todo_max_scroll = 0;
            vec![Line::from(Span::styled(
                "Failed to load TODOs",
                Style::default().fg(Color::Red),
            ))]
        }
    };

    // Cache plain-text content for text selection copy, matching the log
    // panel's wrapping behaviour.
    let todo_inner_width = inner.width as usize;
    app.todo_content_lines = build_wrapped_content_lines(&lines, todo_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.todo_max_scroll = max_scroll;
    let scroll = app.todo_scroll_offset.min(max_scroll);
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

    let snapshot = ragent_agent::session::profiler::agent_loop_profiler().snapshot();
    if !snapshot.enabled {
        app.profile_max_scroll = 0;
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

/// Append a memory source section (header + body) to `lines`.
///
/// Used by [`render_memory_panel`] to render each of the three memory sources
/// (project memory, project analysis, user memory) with a consistent property
/// header line and either the file body or a `(no memory file)` placeholder.
fn append_memory_source<'a>(
    lines: &mut Vec<Line<'a>>,
    title: &str,
    path: &std::path::Path,
    scope_label: &'a str,
    default_scope: ragent_agent::memory::BlockScope,
) {
    use ragent_agent::memory::MemoryBlock;
    use std::fs;

    // Property header (FR-006).
    let (exists, size, mtime, description, body_text) = match fs::read_to_string(path) {
        Ok(text) => {
            let block = MemoryBlock::from_markdown(&text, default_scope);
            let size = text.len();
            let mtime = fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "-".to_string());
            let desc = if block.description.is_empty() {
                String::new()
            } else {
                block.description.clone()
            };
            (true, size, mtime, desc, block.content.clone())
        }
        Err(_) => (false, 0usize, "-".to_string(), String::new(), String::new()),
    };

    // Header line 1: title + path.
    lines.push(Line::from(vec![
        Span::styled(
            format!("{title} "),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({})", path.display()),
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    // Header line 2: properties (scope, size, mtime, description).
    let desc_span = if description.is_empty() {
        Span::raw("")
    } else {
        Span::styled(
            format!("  desc: {description}"),
            Style::default().fg(Color::DarkGray),
        )
    };
    lines.push(Line::from(vec![
        Span::styled("scope ", Style::default().fg(Color::DarkGray)),
        Span::raw(scope_label),
        Span::raw("  "),
        Span::styled("size ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{size}B")),
        Span::raw("  "),
        Span::styled("mtime ", Style::default().fg(Color::DarkGray)),
        Span::raw(mtime),
        desc_span,
    ]));
    lines.push(Line::raw(""));

    // Body (FR-015: placeholder for missing files).
    if exists {
        for raw in body_text.lines() {
            lines.push(Line::raw(raw.to_string()));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "(no memory file)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::raw(""));
}

/// Render the Memory side panel (toggled via `Alt+M`).
///
/// Surfaces the same information as the `/memory show` slash command inside a
/// live, scrollable side panel. Three memory sources are displayed, each with
/// a property header line (path, scope, byte size, last-modified time, and
/// description extracted from YAML frontmatter when present):
///
/// 1. **Project Memory** — `<working_dir>/.ragent/memory/MEMORY.md` (FR-001)
/// 2. **Project Analysis** — `<working_dir>/.ragent/memory/PROJECT_ANALYSIS.md`
///    (rendered only when the file exists)
/// 3. **User Memory** — `~/.ragent/memory/MEMORY.md`
///
/// Missing or unreadable files render a `(no … memory)` placeholder instead of
/// aborting the whole panel (FR-015). Files are re-read on every render so
/// external edits are reflected without restarting the TUI (FR-010). A
/// structured-memory count summary line is rendered at the top when the
/// SQLite store is available (FR-007). Plain-text content is cached in
/// `memory_content_lines` for text selection / copy (FR-013), and a vertical
/// scrollbar is rendered when content overflows the visible height (FR-009).
///
/// # Arguments
/// - `frame` — the ratatui frame to render into.
/// - `app` — mutable `App` state; reads `memory_scroll_offset` and `storage`;
///   writes `memory_area`, `memory_max_scroll`, and `memory_content_lines`.
/// - `area` — the rect allocated to the panel by the side-panel split.
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

    use ragent_agent::memory::BlockScope;
    use std::path::PathBuf;

    // Resolve the three memory source paths (FR-001).
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_mem: PathBuf = cwd.join(".ragent").join("memory").join("MEMORY.md");
    let project_analysis: PathBuf = cwd
        .join(".ragent")
        .join("memory")
        .join("PROJECT_ANALYSIS.md");
    let user_mem: Option<PathBuf> =
        dirs::home_dir().map(|h| h.join(".ragent").join("memory").join("MEMORY.md"));

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Optional structured-memory count summary line at the top of the panel
    // (FR-007). Rendered only when the SQLite structured-memory store is
    // available and reports a non-zero count, so the panel stays compact for
    // projects that have not adopted structured memories yet.
    if let Ok(count) = app.storage.count_memories() {
        if count > 0 {
            lines.push(Line::from(Span::styled(
                format!("Structured memories: {count}"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
            lines.push(Line::raw(""));
        }
    }

    // Project memory.
    append_memory_source(
        &mut lines,
        "Project Memory",
        &project_mem,
        "project",
        BlockScope::Project,
    );

    // Project analysis (only if the file exists — FR-001).
    if project_analysis.exists() {
        append_memory_source(
            &mut lines,
            "Project Analysis",
            &project_analysis,
            "project",
            BlockScope::Project,
        );
    }

    // User memory.
    if let Some(path) = &user_mem {
        append_memory_source(&mut lines, "User Memory", path, "user", BlockScope::Global);
    } else {
        lines.push(Line::from(Span::styled(
            "User Memory (no home directory)",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "(no user memory)",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::raw(""));
    }

    // Cache plain-text content for text selection copy (FR-013), matching the
    // log / todo / profile panels' wrapping behaviour.
    let memory_inner_width = inner.width as usize;
    app.memory_content_lines = build_wrapped_content_lines(&lines, memory_inner_width);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.memory_max_scroll = max_scroll;
    let scroll = app.memory_scroll_offset.min(max_scroll);
    let paragraph = paragraph.scroll((scroll, 0));
    frame.render_widget(paragraph, inner);

    // Render scrollbar when content overflows (FR-009).
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

    let mut lines: Vec<Line<'_>> = Vec::new();

    if let Some(ref sid) = target_session {
        let session_messages = if app.session_id.as_deref() == Some(sid.as_str()) {
            app.messages.clone()
        } else {
            app.storage.get_messages(sid).unwrap_or_default()
        };
        lines = messages_to_lines(
            &session_messages,
            &app.tool_step_map,
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
                || (entry.session_id.is_none() && app.session_id.as_deref() == Some(sid.as_str()))
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
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    let total = paragraph.line_count(inner.width) as u16;
    let visible = inner.height;
    view.max_scroll = total.saturating_sub(visible);
    view.scroll_offset = view.scroll_offset.min(view.max_scroll);

    frame.render_widget(paragraph.scroll((view.scroll_offset, 0)), area);

    if total > visible {
        let mut sb_state =
            ScrollbarState::new(view.max_scroll as usize).position(view.scroll_offset as usize);
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
            MemberStatus::Suspended => ("⏸", Color::DarkGray),
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

/// Render a slice of messages into formatted lines using the rich format
/// from the primary Messages panel.  Both `render_messages` and
/// `render_output_view_overlay` delegate here so teammate output looks
/// identical to the lead agent's chat window.
fn messages_to_lines<'a>(
    messages: &[Message],
    tool_step_map: &std::collections::HashMap<String, (String, u32, u32)>,
    sid_to_display: &std::collections::HashMap<String, String>,
    cwd: &str,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line<'a>> = Vec::new();

    for msg in messages {
        for part in &msg.parts {
            match part {
                MessagePart::Text { text } => {
                    let (dot, dot_style, indent) = match msg.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        Role::Assistant => (
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
                    let step_tag = if let Some((sid, step, substep)) = tool_step_map.get(call_id) {
                        // Look up display name from app
                        let display = sid_to_display
                            .get(sid)
                            .cloned()
                            .unwrap_or_else(|| sid.clone());
                        format!("[{display}:{step}.{substep}] ")
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
                        format!("  📎 [image: {}]", name),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }

            lines.push(Line::from(""));
        }
    }

    lines
}

fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    // Determine which session to display messages for.
    // If a specific agent is selected, show its messages; otherwise show primary session.
    let _display_session = app
        .selected_agent_session_id
        .clone()
        .or_else(|| app.session_id.clone());

    // Filter messages to the selected agent's session.
    // For now, messages are still stored globally, so we match by session_id if available.
    // TODO: Implement proper multi-session message storage to filter by _display_session.
    // This is a placeholder for future multi-session message handling.
    let messages_to_show = &app.messages;

    let lines = messages_to_lines(
        messages_to_show,
        &app.tool_step_map,
        &app.sid_to_display_name,
        &app.cwd,
    );
    // Cache plain-text content for text selection copy
    // Must match the word-wrapped display that Paragraph renders
    let inner_width = area.width.saturating_sub(2) as usize;
    app.message_content_lines = build_wrapped_content_lines(&lines, inner_width);

    // Build the paragraph with wrapping so we can measure the true rendered height.
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

    let paragraph = Paragraph::new(lines)
        .block(messages_block)
        .wrap(Wrap { trim: false });

    // Use line_count() which accounts for word-wrap at the inner width
    // (area width minus left+right borders).
    let inner_width = area.width.saturating_sub(2);
    let total = paragraph.line_count(inner_width) as u16;
    let visible = area.height.saturating_sub(2);
    let max_scroll = total.saturating_sub(visible);
    // Clamp scroll_offset when content shrinks to prevent blank timeline
    // (C3 fix: Timeline no longer goes blank when content shrinks)
    app.scroll_offset = app.scroll_offset.min(max_scroll);
    app.message_max_scroll = max_scroll;
    let scroll = max_scroll.saturating_sub(app.scroll_offset);
    let paragraph = paragraph.scroll((scroll, 0));

    frame.render_widget(paragraph, area);

    // Render scrollbar when content overflows
    if total > visible {
        let scroll_position = scroll as usize;
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
                    .map(|s| format!("📎{s}"))
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
    ("Alt+T", "Toggle TODO panel visibility"),
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
            "⚠️  Permission Required {} ({} queued)",
            countdown_text, queue_depth
        )
    } else {
        format!("⚠️  Permission Required {}", countdown_text)
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

#[cfg(test)]
mod tests {
    use super::messages_to_lines;
    use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
    use serde_json::json;
    use std::collections::HashMap;

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
}
