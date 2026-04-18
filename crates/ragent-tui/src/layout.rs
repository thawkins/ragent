//! TUI layout and rendering.
//!
//! Builds the main layout with a 2-line status bar at the top, messages in the
//! middle, and an input area at the bottom.
//!
//! The status bar is organized into 2 lines for better readability:
//! - Line 1: Session, agent, working directory, git branch, and status message
//! - Line 2: Provider, token usage, active tasks, LSP status, code index, and log indicator

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap,
    },
};

use crate::layout_active_agents::render_active_agents_subpanel;

use crate::utils::{ResponsiveBreakpoint, centered_rect, is_below_minimum_size};

use ragent_core::message::{Message, MessagePart, Role, ToolCallStatus};

use crate::app::{
    App, ContextAction, LogLevel, OutputViewTarget, PROVIDER_LIST, ProviderSetupStep, SelectionPane,
};
use crate::widgets::message_widget::{
    capitalize_tool_name, read_line_range, tool_inline_diff, tool_input_summary,
    tool_result_summary,
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
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let Some(step) = app.provider_setup.as_ref() else {
        return;
    };
    match step {
        ProviderSetupStep::SelectProvider { selected } => {
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    "Select a Provider",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            for (i, (_pid, pname)) in PROVIDER_LIST.iter().enumerate() {
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
                    Span::styled(*pname, style),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter select  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Provider Setup ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
        }
        ProviderSetupStep::EnterKey {
            provider_id,
            provider_name,
            key_input,
            key_cursor,
            endpoint_input,
            endpoint_cursor,
            editing_endpoint,
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
            let masked = if key_input.is_empty() {
                String::new()
            } else {
                let char_count = key_input.chars().count();
                if char_count <= 8 {
                    "*".repeat(char_count)
                } else {
                    let first4: String = key_input.chars().take(4).collect();
                    let last4: String = key_input
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
            let key_cursor_display = if !*editing_endpoint {
                *key_cursor
            } else {
                masked.chars().count()
            };
            lines.push(Line::from(vec![
                Span::styled(
                    if !*editing_endpoint { "> " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    with_cursor_marker(&masked, key_cursor_display),
                    Style::default().fg(Color::White),
                ),
            ]));

            if provider_id == "generic_openai" {
                lines.push(Line::from(""));
                lines.push(Line::from(
                    "Endpoint URL (optional, e.g. http://localhost:11434/v1):",
                ));
                let endpoint_cursor_display = if *editing_endpoint {
                    *endpoint_cursor
                } else {
                    endpoint_input.chars().count()
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        if *editing_endpoint { "> " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        if endpoint_input.is_empty() {
                            "(use default/env)".to_string()
                        } else {
                            with_cursor_marker(endpoint_input, endpoint_cursor_display)
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
            // Create header row
            let header = Row::new(vec!["Model", "Context", "Cost", "Features"])
                .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan));

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

                    // Format cost
                    let cost_str = if entry.cost_input == 0.0 && entry.cost_output == 0.0 {
                        "Free".to_string()
                    } else {
                        format!("${:.2}/${:.2}", entry.cost_input, entry.cost_output)
                    };

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

                    Row::new(vec![model_name, ctx_str, cost_str, features_str]).style(style)
                })
                .collect();

            let table = Table::new(rows, [
                Constraint::Percentage(45), // Model name
                Constraint::Percentage(15), // Context window
                Constraint::Percentage(25), // Cost
                Constraint::Percentage(15), // Features
            ])
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
                frame.render_widget(Paragraph::new(showing_line).alignment(Alignment::Right), showing_area);
            }
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
                lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(*pname, style),
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

#[allow(dead_code)]
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
            Constraint::Length(team_strip_h), // teammate strip (0 when hidden)
            Constraint::Min(3),               // messages + optional log
            Constraint::Length(input_height), // input (dynamic)
        ])
        .split(chat_area);

    render_status_bar(frame, app, chunks[0]);

    if team_strip {
        render_teammate_strip(frame, app, chunks[1]);
    }

    // Split the middle area horizontally when the log panel is visible.
    // Use responsive log split based on terminal width.
    if app.show_log {
        let (msg_pct, log_pct) = breakpoint.log_split();
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(msg_pct), // messages (responsive)
                Constraint::Percentage(log_pct), // log (responsive)
            ])
            .split(chunks[2]);

        app.message_area = h_chunks[0];
        app.log_area = h_chunks[1];
        render_messages(frame, app, h_chunks[0]);
        apply_selection_highlight(frame, app, SelectionPane::Messages, h_chunks[0]);
        render_log_panel(frame, app, h_chunks[1]);
        apply_selection_highlight(frame, app, SelectionPane::Log, h_chunks[1]);
    } else {
        app.message_area = chunks[2];
        app.log_area = Rect::default();
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

    // LSP discover dialog overlay
    if app.lsp_discover.is_some() {
        render_lsp_discover_dialog(frame, app);
    }

    // LSP edit dialog overlay
    if app.lsp_edit.is_some() {
        render_lsp_edit_dialog(frame, app);
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

    // Journal viewer overlay
    if app.journal_viewer.is_some() {
        crate::panels::render_journal_viewer(frame, app);
    } else {
        app.journal_viewer_close_area = Rect::default();
        app.journal_viewer_area = Rect::default();
    }

    // Render output overlay last so it always appears above Teams/Agents popups.
    if app.output_view.is_some() {
        render_output_view_overlay(frame, app);
    } else {
        app.output_view_area = Rect::default();
    }
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
        frame.render_stateful_widget(scrollbar, log_inner, &mut scrollbar_state);
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
    use ragent_core::team::MemberStatus;

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

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    // Split area into 2 lines
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Row 1: Context information (folder, branch, shell cwd, status)
    let mut row1_left: Vec<Span<'_>> = Vec::new();

    // ragent indicator with version
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    row1_left.push(Span::styled(
        format!("● Ragent: {} ", version),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));

    // Current directory
    row1_left.push(Span::styled(
        format!("│ {} ", app.cwd),
        Style::default().fg(Color::White),
    ));
    // Shell cwd if different
    if let Some(ref shell_cwd) = app.shell_cwd {
        if *shell_cwd != app.cwd {
            row1_left.push(Span::styled(
                format!("→ {} ", shell_cwd),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    // Git branch
    if let Some(ref branch) = app.git_branch {
        row1_left.push(Span::styled(
            format!("│ ⎇ {} ", branch),
            Style::default().fg(Color::Green),
        ));
    }

    // Status message on the right
    let mut row1_right: Vec<Span<'_>> = Vec::new();
    if !app.status.is_empty() && app.status != "Ready" {
        row1_right.push(Span::styled(
            format!("{} ", app.status),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let left_len: usize = row1_left.iter().map(|s| s.content.len()).sum();
    let right_len: usize = row1_right.iter().map(|s| s.content.len()).sum();
    let gap = rows[0]
        .width
        .saturating_sub(left_len as u16 + right_len as u16);
    let gap_span = Span::raw(" ".repeat(gap as usize));

    row1_left.push(gap_span);
    row1_left.extend(row1_right);

    let line1 = Line::from(row1_left);
    frame.render_widget(Paragraph::new(line1), rows[0]);

    // Row 2: Resources and system state (provider, tokens, tasks, LSP, log)
    let mut row2_left: Vec<Span<'_>> = Vec::new();

    // Provider with health indicator
    if let Some(label) = app.provider_model_label() {
        let (icon, health_color) = match app.provider_health_status() {
            Some(true) => ("●", Color::Green),
            Some(false) => ("✗", Color::Red),
            None => ("●", Color::Yellow),
        };
        row2_left.push(Span::styled(
            format!("{} ", icon),
            Style::default()
                .fg(health_color)
                .add_modifier(Modifier::BOLD),
        ));
        row2_left.push(Span::styled(
            format!("{} ", label),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Token usage
    row2_left.push(Span::styled(
        format!("│ tokens: {}/{} ", app.token_usage.0, app.token_usage.1),
        Style::default().fg(Color::Cyan),
    ));

    // Usage quota indicator
    {
        let (usage_text, is_unknown) = app.usage_display();
        // Extract percentage from text (e.g., "ctx: 45%" or "Pro quota: 85.5%")
        let pct_in_text = usage_text
            .split_whitespace()
            .last()
            .and_then(|s| s.trim_end_matches('%').parse::<f32>().ok());
        let fg = if is_unknown {
            Color::White
        } else if let Some(q) = app.quota_percent {
            if q >= 95.0 {
                Color::Red
            } else if q >= 80.0 {
                Color::Yellow
            } else {
                Color::Green
            }
        } else if let Some(p) = pct_in_text {
            // Color based on context window percentage
            if p >= 95.0 {
                Color::Red
            } else if p >= 80.0 {
                Color::Yellow
            } else {
                Color::Green
            }
        } else {
            Color::Green
        };
        row2_left.push(Span::styled(
            format!("[{}] ", usage_text),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));
    }

    // Stream bytes received counter
    if app.stream_bytes > 0 {
        let bytes_text = if app.stream_bytes >= 1_048_576 {
            format!("↓ {:.1}M", app.stream_bytes as f64 / 1_048_576.0)
        } else if app.stream_bytes >= 1024 {
            format!("↓ {:.1}K", app.stream_bytes as f64 / 1024.0)
        } else {
            format!("↓ {}B", app.stream_bytes)
        };
        row2_left.push(Span::styled(
            format!("[{}] ", bytes_text),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Active tasks
    if !app.active_tasks.is_empty() {
        let running = app
            .active_tasks
            .iter()
            .filter(|t| t.status == ragent_core::task::TaskStatus::Running)
            .count();
        row2_left.push(Span::styled(
            format!("│ ⚙ {}/{} tasks ", running, app.active_tasks.len()),
            Style::default().fg(Color::Magenta),
        ));
    }

    // LSP servers
    {
        use ragent_core::lsp::LspStatus;
        let connected = app
            .lsp_servers
            .iter()
            .filter(|s| s.status == LspStatus::Connected)
            .count();
        let total = app.lsp_servers.len();
        if total > 0 {
            let (icon, color) = if connected == total {
                ("⬡", Color::Cyan)
            } else if connected > 0 {
                ("⬡", Color::Yellow)
            } else {
                ("⬡", Color::DarkGray)
            };
            row2_left.push(Span::styled(
                format!("│ {} LSP {}/{} ", icon, connected, total),
                Style::default().fg(color),
            ));
        }
    }

    // Code index indicator (uses cached stats to avoid per-frame SQL/FTS queries)
    {
        let enabled = app.code_index_enabled;
        let (tick, tick_color) = if enabled {
            ("✓", Color::Green)
        } else {
            ("✗", Color::Red)
        };
        row2_left.push(Span::styled(
            "│ Codeindex: ",
            Style::default().fg(Color::DarkGray),
        ));
        row2_left.push(Span::styled(
            format!("{tick} "),
            Style::default().fg(tick_color).add_modifier(Modifier::BOLD),
        ));
        if enabled {
            if let Some(ref stats) = app.code_index_stats_cache {
                let label = if stats.files_indexed > 0 {
                    format!("{} files/{} syms", stats.files_indexed, stats.total_symbols)
                } else {
                    "empty".to_string()
                };
                row2_left.push(Span::styled(label, Style::default().fg(Color::Cyan)));
            }
            if app.code_index_busy {
                let pct_label = if let Some(ref idx) = app.code_index {
                    let (done, total) = idx.reindex_progress();
                    if total > 0 {
                        let pct = (done as f64 / total as f64 * 100.0).min(100.0);
                        format!(" ⟳indexing {pct:.0}%")
                    } else {
                        " ⟳indexing".to_string()
                    }
                } else {
                    " ⟳indexing".to_string()
                };
                row2_left.push(Span::styled(
                    pct_label,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            row2_left.push(Span::raw(" "));
        }
    }

    // ── Memory indicator ────────────────────────────────────────────────────
    {
        let block_count = app.memory_block_count;
        let entry_count = app.memory_entry_count;
        let journal_count = app.journal_entry_count;
        if block_count > 0 || entry_count > 0 || journal_count > 0 {
            let mem_label = if entry_count > 0 {
                format!("│ MEM: {}blk, {}mem", block_count, entry_count)
            } else {
                format!("│ MEM: {}blk", block_count)
            };
            row2_left.push(Span::styled(mem_label, Style::default().fg(Color::Magenta)));
            if journal_count > 0 {
                row2_left.push(Span::styled(
                    format!(", {}j", journal_count),
                    Style::default().fg(Color::Magenta),
                ));
            }
            row2_left.push(Span::raw(" "));

            // Relative time of last update
            if let Some(updated) = app.memory_last_updated {
                let elapsed = updated.elapsed();
                let time_str = if elapsed.as_secs() < 60 {
                    format!("{}s ago", elapsed.as_secs())
                } else if elapsed.as_secs() < 3600 {
                    format!("{}m ago", elapsed.as_secs() / 60)
                } else {
                    format!("{}h ago", elapsed.as_secs() / 3600)
                };
                row2_left.push(Span::styled(time_str, Style::default().fg(Color::DarkGray)));
                row2_left.push(Span::raw(" "));
            }
        }
    }

    let line2 = Line::from(row2_left);
    frame.render_widget(Paragraph::new(line2), rows[1]);
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
                    if summary.is_empty() {
                        spans.push(Span::styled(display_name, name_style));
                    } else {
                        // Extract icon (emoji + space) from the beginning of summary
                        let mut parts = summary.splitn(2, ' ');
                        let icon = parts.next().unwrap_or("");
                        let rest = parts.next().unwrap_or("");
                        if !icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{} ", icon),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
                        spans.push(Span::styled(format!("{} ", display_name), name_style));
                        if !rest.is_empty() {
                            spans.push(Span::styled(
                                rest.to_string(),
                                Style::default().fg(Color::DarkGray),
                            ));
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
                    lines.push(Line::from(spans));

                    if state.status == ToolCallStatus::Completed && tool != "edit" {
                        // Skip result summary for edit tool (already shows inline diff)
                        if let Some(result) =
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
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        )));
                    }
                }
                MessagePart::Image { path, .. } => {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
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
        .title(Span::styled(title, title_style));

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
    ("Esc", "Cancel running agent (while processing)"),
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

    if req.permission == "question" {
        // Question-type: show a text-input dialog so the user can type a response.
        let area = centered_rect(70, 40, frame.area());
        frame.render_widget(Clear, area);

        let question_text = req.patterns.first().map(|s| s.as_str()).unwrap_or("");
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
                question_text,
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
        return;
    }

          // Standard permission dialog: y/a/n.
          let area = centered_rect(60, 40, frame.area()); // Increased height for better visibility
          frame.render_widget(Clear, area);
    
          // Wrap the dialog in a block with strong styling to make it prominent
          let text = vec![
              Line::from(Span::styled(
                  "⚠️  Permission Required",
                  Style::default()
                      .fg(Color::Yellow)
                      .add_modifier(Modifier::BOLD),
              )),
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
              format!(" Permission: {} ({} queued) ", req.permission, queue_depth)
          } else {
              format!(" Permission: {} ", req.permission)
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

/// Render the interactive LSP discovery dialog overlay.
fn render_lsp_discover_dialog(frame: &mut Frame, app: &App) {
    let Some(state) = app.lsp_discover.as_ref() else {
        return;
    };

    // Fixed dialog height — scrollable content fits inside.
    let dialog_height = 24u16;
    let area = {
        let full = frame.area();
        let h = dialog_height.min(full.height.saturating_sub(4));
        let w = full.width.min(82);
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
            "LSP Server Discovery",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if state.servers.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No language servers detected.",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  Install a server on PATH (e.g. rust-analyzer, gopls) and retry.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        // Load current config once so we can flag already-enabled servers.
        let enabled_ids: std::collections::HashSet<String> = ragent_core::config::Config::load()
            .map(|c| c.lsp.into_keys().collect())
            .unwrap_or_default();

        // Column header
        lines.push(Line::from(vec![Span::styled(
            format!(
                "  {:<3}  {:<18}  {:<10}  {:<20}  {}",
                "#", "Name", "Version", "Extensions", "Executable"
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(Span::styled(
            format!("  {}", "─".repeat(74)),
            Style::default().fg(Color::DarkGray),
        )));

        for (i, srv) in state.servers.iter().enumerate() {
            let already_enabled = enabled_ids.contains(&srv.id);
            let num = format!("{}", i + 1);
            let exts = srv.extensions.join(", ");
            let version = srv.version.as_deref().unwrap_or("—");
            let exe = {
                let s = srv.executable.to_string_lossy();
                if s.len() > 22 {
                    format!("…{}", &s[s.len().saturating_sub(21)..])
                } else {
                    s.into_owned()
                }
            };
            let (num_color, name_color, ext_color, exe_color) = if already_enabled {
                (Color::Yellow, Color::Yellow, Color::Yellow, Color::Yellow)
            } else {
                (Color::Cyan, Color::White, Color::Green, Color::DarkGray)
            };
            let enabled_tag = if already_enabled { " ✓" } else { "" };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<3}", num),
                    Style::default().fg(num_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {:<18}", format!("{}{}", srv.id, enabled_tag)),
                    Style::default().fg(name_color),
                ),
                Span::styled(
                    format!("  {:<10}", version),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(format!("  {:<20}", exts), Style::default().fg(ext_color)),
                Span::styled(format!("  {}", exe), Style::default().fg(exe_color)),
            ]));
        }

        lines.push(Line::from(Span::styled(
            "  (yellow = already enabled in ragent.json)",
            Style::default().fg(Color::DarkGray),
        )));

        // Scroll hint if list overflows visible area
        let fixed_rows = 7u16; // header + sep + legend + blank + feedback(2) + prompt lines
        let visible_rows = area.height.saturating_sub(fixed_rows + 2); // +2 for border
        if state.servers.len() as u16 > visible_rows {
            lines.push(Line::from(Span::styled(
                "  ↑/↓ PgUp/PgDn to scroll",
                Style::default().fg(Color::DarkGray),
            )));
        }
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
                    .fg(Color::Cyan)
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
        .title(" /lsp discover ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left)
        .scroll((state.scroll_offset, 0));
    frame.render_widget(paragraph, area);
}

/// Render the interactive LSP edit dialog overlay.
///
/// Shows all configured LSP servers. ↑/↓ moves the cursor; Space/Enter toggles
/// enabled/disabled; Esc closes the dialog.
fn render_lsp_edit_dialog(frame: &mut Frame, app: &App) {
    let Some(state) = app.lsp_edit.as_ref() else {
        return;
    };

    let dialog_height = 24u16;
    let area = {
        let full = frame.area();
        let h = dialog_height.min(full.height.saturating_sub(4));
        let w = full.width.min(72);
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
            "LSP Server Configuration",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    // Column header
    lines.push(Line::from(vec![Span::styled(
        format!("  {:<20}  {}", "Server ID", "Status"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(Span::styled(
        format!("  {}", "─".repeat(46)),
        Style::default().fg(Color::DarkGray),
    )));

    // Compute visible window for scrolling (account for header/footer rows)
    let fixed_rows = 8u16; // title + blank + header + sep + blank + feedback + hint + border
    let visible_rows = area.height.saturating_sub(fixed_rows) as usize;
    // Clamp scroll so selected row is always visible
    let scroll = state.scroll_offset as usize;

    for (i, (id, disabled)) in state.servers.iter().enumerate() {
        let is_selected = i == state.selected;
        let (status_str, status_color) = if *disabled {
            ("⚪ disabled", Color::DarkGray)
        } else {
            ("🟢 enabled ", Color::Green)
        };

        let row_style = if is_selected {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let cursor = if is_selected { "▶ " } else { "  " };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}{:<20}", cursor, id),
                row_style.fg(if is_selected {
                    Color::White
                } else {
                    Color::White
                }),
            ),
            Span::styled(format!("  {}", status_str), row_style.fg(status_color)),
        ]));
    }

    lines.push(Line::from(""));

    // Feedback line
    if let Some(ref msg) = state.feedback {
        let color = if msg.starts_with('✗') {
            Color::Red
        } else {
            Color::Green
        };
        lines.push(Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(color),
        )));
        lines.push(Line::from(""));
    }

    // Hint row
    let scroll_hint = if state.servers.len() > visible_rows {
        "  ↑/↓ scroll  Space/Enter toggle  Esc close"
    } else {
        "  ↑/↓ move  Space/Enter toggle  Esc close"
    };
    lines.push(Line::from(Span::styled(
        scroll_hint,
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" /lsp edit ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left)
        .scroll((scroll as u16, 0));
    frame.render_widget(paragraph, area);
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
        let enabled_ids: std::collections::HashSet<String> = ragent_core::config::Config::load()
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
            let name = if srv.name.len() > 38 {
                format!("{}…", &srv.name[..37])
            } else {
                srv.name.clone()
            };
            let source = match &srv.source {
                ragent_core::mcp::McpDiscoverySource::SystemPath => "PATH".to_string(),
                ragent_core::mcp::McpDiscoverySource::NpmGlobal { .. } => "npm global".to_string(),
                ragent_core::mcp::McpDiscoverySource::McpRegistry { .. } => {
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

/// Render the plan approval dialog overlay.
///
/// Shows the plan text (scrollable) with Approve / Reject buttons. The user
/// presses Enter to approve or `r`/Esc to reject.
#[allow(dead_code)]
fn render_plan_approval_dialog(frame: &mut Frame, app: &App) {
    let Some(ref state) = app.plan_approval_pending else {
        return;
    };

    let full = frame.area();
    let w = full.width.min(90);
    let h = full.height.saturating_sub(4).min(30);
    let area = Rect {
        x: (full.width.saturating_sub(w)) / 2,
        y: (full.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, area);

    // Plan text lines (truncated to fit)
    let inner_width = area.width.saturating_sub(4) as usize;
    let text_height = area.height.saturating_sub(6) as usize;
    let mut plan_lines: Vec<Line<'_>> = state
        .plan_text
        .lines()
        .flat_map(|l| {
            if l.len() <= inner_width {
                vec![Line::from(l.to_string())]
            } else {
                l.chars()
                    .collect::<Vec<_>>()
                    .chunks(inner_width)
                    .map(|c| Line::from(c.iter().collect::<String>()))
                    .collect()
            }
        })
        .take(text_height)
        .collect();
    if state.plan_text.lines().count() > text_height {
        plan_lines.push(Line::from(Span::styled(
            "  … (scroll not yet wired — approve/reject below) …",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Approve/Reject buttons
    let (approve_style, reject_style) = if state.cursor_approve {
        (
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
    };

    plan_lines.push(Line::from(""));
    plan_lines.push(Line::from(vec![
        Span::styled(" ✓ Approve ", approve_style),
        Span::raw("   "),
        Span::styled(" ✗ Reject ", reject_style),
        Span::styled(
            "       ←/→ toggle  Enter confirm  Esc cancel",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " 📋 Plan Review ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(plan_lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(paragraph, area);
}
