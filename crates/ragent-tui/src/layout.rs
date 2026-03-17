//! TUI layout and rendering.
//!
//! Builds the three-row layout (status bar, messages, input) and draws each
//! section plus an optional permission dialog overlay. On first launch the
//! home screen shows a centered logo, prompt, tips, and provider status.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Flex, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use ragent_core::message::{MessagePart, Role, ToolCallStatus};

use crate::app::{
    App, LogLevel, PROVIDER_LIST, ProviderSetupStep, ScreenMode, SelectionPane,
};
use crate::logo;
use crate::widgets::message_widget::{
    capitalize_tool_name, tool_inline_diff, tool_input_summary,
    tool_result_summary,
};

/// The version string shown on the home screen.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Render the full TUI, dispatching to the Home or Chat screen.
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
    match app.current_screen {
        ScreenMode::Home => render_home(frame, app),
        ScreenMode::Chat => render_chat(frame, app),
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

// ---------------------------------------------------------------------------
// Home screen
// ---------------------------------------------------------------------------

fn render_home(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Compute input height based on wrapped text length
    let max_width = 88u16.min(area.width.saturating_sub(4));
    let inner_width = max_width.saturating_sub(2).max(1) as usize; // inside borders
    let input_text_len = app.input.len() + 2; // "> " prefix
    let num_lines = ((input_text_len as f32) / (inner_width as f32))
        .ceil()
        .max(1.0) as u16;
    let input_height = num_lines + 2; // +2 for top and bottom borders

    // Vertical layout: flex-grow top | logo | gap | prompt | provider | tip | flex-grow bottom | status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),               // top spacer
            Constraint::Length(4),            // logo (4 lines)
            Constraint::Length(1),            // gap
            Constraint::Length(input_height), // prompt input (dynamic)
            Constraint::Length(2),            // provider status
            Constraint::Length(2),            // tip
            Constraint::Min(1),               // bottom spacer
            Constraint::Length(1),            // status bar
        ])
        .flex(Flex::Center)
        .split(area);

    // Logo — centered
    render_logo(frame, chunks[1]);

    // Prompt — centered input
    let home_input_area = centered_horizontal(max_width, chunks[3]);
    app.home_input_area = home_input_area;
    render_home_input(frame, app, chunks[3]);
    apply_selection_highlight(frame, app, SelectionPane::HomeInput, home_input_area);

    // Slash menu dropdown (above the input, if active)
    if app.slash_menu.is_some() {
        let input_area = centered_horizontal(max_width, chunks[3]);
        render_slash_menu(frame, app, input_area);
    }

    // File menu dropdown (above the input, if active)
    if app.file_menu.is_some() {
        let input_area = centered_horizontal(max_width, chunks[3]);
        render_file_menu(frame, app, input_area);
    }

    // Provider status
    render_provider_status(frame, app, chunks[4]);

    // Tip — centered below prompt
    render_tip(frame, app, chunks[5]);

    // Bottom status bar
    render_home_status_bar(frame, app, chunks[7]);

    // Provider setup dialog overlay (if active)
    if app.provider_setup.is_some() {
        render_provider_setup_dialog(frame, app);
    }
}

fn render_logo(frame: &mut Frame, area: Rect) {
    let logo_width = logo::LOGO.iter().map(|l| l.len()).max().unwrap_or(0) as u16;

    // Centre the logo horizontally
    let centered = centered_horizontal(logo_width, area);

    let lines: Vec<Line<'_>> = logo::LOGO
        .iter()
        .map(|line| {
            Line::from(Span::styled(
                *line,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, centered);
}

fn render_home_input(frame: &mut Frame, app: &App, area: Rect) {
    let max_width = 88u16.min(area.width.saturating_sub(4));
    let centered = centered_horizontal(max_width, area);

    let input_text = format!("> {}", app.input);
    let inner_width = centered.width.saturating_sub(2).max(1) as usize;

    // Character-wrap the text so cursor math (pos / width) stays correct
    let wrapped_lines = char_wrap(&input_text, inner_width);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Ask anything… ")
        .title_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(wrapped_lines).block(block);
    frame.render_widget(paragraph, centered);

    // Position cursor accounting for wrapped lines
    let cursor_pos = app.input.len() + 2; // "> " prefix
    let cursor_line = cursor_pos / inner_width;
    let cursor_col = cursor_pos % inner_width;
    let cursor_x = centered.x + 1 + cursor_col as u16;
    let cursor_y = centered.y + 1 + cursor_line as u16;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_tip(frame: &mut Frame, app: &App, area: Rect) {
    let max_width = 88u16.min(area.width.saturating_sub(4));
    let centered = centered_horizontal(max_width, area);

    let tip_line = Line::from(vec![
        Span::styled(
            "● Tip  ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(app.tip, Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(tip_line).alignment(Alignment::Left);
    frame.render_widget(paragraph, centered);
}

fn render_provider_status(frame: &mut Frame, app: &App, area: Rect) {
    let max_width = 88u16.min(area.width.saturating_sub(4));
    let centered = centered_horizontal(max_width, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    if let Some(ref prov) = app.configured_provider {
        let source_label = match prov.source {
            crate::app::ProviderSource::EnvVar => " (env)",
            crate::app::ProviderSource::Database => " (saved)",
            crate::app::ProviderSource::AutoDiscovered => " (auto)",
        };

        // Health indicator: green dot, red cross, or yellow dot while checking
        let (health_icon, health_color) = match app.provider_health_status() {
            Some(true) => ("● ", Color::Green),
            Some(false) => ("✗ ", Color::Red),
            None => ("● ", Color::Yellow),
        };

        let mut spans = vec![
            Span::styled(health_icon, Style::default().fg(health_color)),
            Span::styled(&prov.name, Style::default().fg(Color::Green)),
            Span::styled(source_label, Style::default().fg(Color::DarkGray)),
        ];

        if let Some(label) = app.provider_model_label() {
            let model_id = label.split(" / ").nth(1).unwrap_or(&label);
            spans.push(Span::styled(
                format!("  model: {}", model_id),
                Style::default().fg(Color::Cyan),
            ));
        }

        spans.push(Span::styled(
            "  — use /provider to change",
            Style::default().fg(Color::DarkGray),
        ));

        lines.push(Line::from(spans));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                "⚠ No provider configured",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  — use /provider to set up",
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
    frame.render_widget(paragraph, centered);
}

fn render_provider_setup_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let step = app.provider_setup.as_ref().unwrap();
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
            provider_name,
            key_input,
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
                "".to_string()
            } else {
                let char_count = key_input.chars().count();
                if char_count <= 8 {
                    "*".repeat(char_count)
                } else {
                    let first4: String = key_input.chars().take(4).collect();
                    let last4: String = key_input.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
                    format!("{}…{}", first4, last4)
                }
            };
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::styled(masked, Style::default().fg(Color::White)),
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
            let mut lines: Vec<Line<'_>> = vec![
                Line::from(Span::styled(
                    format!("Select a Model ({})", provider_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            if models.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No models available",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for (i, (_mid, mname)) in models.iter().enumerate() {
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
                        Span::styled(mname.as_str(), style),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "↑/↓ navigate  Enter select  Esc cancel",
                Style::default().fg(Color::DarkGray),
            )));

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Select Model ")
                .border_style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, area);
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

            for (i, (name, desc)) in agents.iter().enumerate() {
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
                lines.push(Line::from(vec![
                    Span::styled(indicator, style),
                    Span::styled(name.as_str(), style),
                    Span::styled(
                        format!("  {}{}", desc, current_marker),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
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

    let item_count = menu.matches.len() as u16;
    // +2 for the border
    let height = (item_count + 2).min(input_area.y);
    let width = input_area.width.min(50);

    let popup = Rect::new(
        input_area.x,
        input_area.y.saturating_sub(height),
        width,
        height,
    );

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    for (i, entry) in menu.matches.iter().enumerate() {
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup);
}

/// Render the `@` file reference autocomplete popup above the input area.
fn render_file_menu(frame: &mut Frame, app: &App, input_area: Rect) {
    let menu = match &app.file_menu {
        Some(m) => m,
        None => return,
    };

    if menu.matches.is_empty() {
        return;
    }

    let item_count = menu.matches.len() as u16;
    let height = (item_count + 2).min(input_area.y);
    let width = input_area.width.min(60);

    let popup = Rect::new(
        input_area.x,
        input_area.y.saturating_sub(height),
        width,
        height,
    );

    frame.render_widget(Clear, popup);

    let mut lines: Vec<Line<'_>> = Vec::new();
    for (i, entry) in menu.matches.iter().enumerate() {
        let is_selected = i == menu.selected;
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
        lines.push(Line::from(vec![
            Span::styled(indicator, path_style),
            Span::raw(icon),
            Span::styled(&entry.display, path_style),
        ]));
    }

    let title = format!(" @{} ", menu.query);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup);
}

fn render_home_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let version = format!("v{}", VERSION);

    // Left side: agent name + path + git branch
    let mut left_spans: Vec<Span<'_>> = vec![
        Span::styled(
            format!(" agent: {}", app.agent_name),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", app.cwd),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    if let Some(ref branch) = app.git_branch {
        left_spans.push(Span::styled(
            format!("  ⎇ {}", branch),
            Style::default().fg(Color::Cyan),
        ));
    }

    if app.show_log {
        left_spans.push(Span::styled(
            "  ▪ log:on",
            Style::default().fg(Color::Yellow),
        ));
    } else {
        left_spans.push(Span::styled(
            "  ▪ log:off",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let right_span = Span::styled(
        format!("{}  ", version),
        Style::default().fg(Color::DarkGray),
    );

    let left_len: usize = left_spans.iter().map(|s| s.content.len()).sum();
    let right_len = right_span.content.len();
    let gap = area
        .width
        .saturating_sub(left_len as u16 + right_len as u16);
    let gap_span = Span::raw(" ".repeat(gap as usize));

    let mut spans = left_spans;
    spans.push(gap_span);
    spans.push(right_span);

    let line = Line::from(spans);
    let bar = Paragraph::new(line);
    frame.render_widget(bar, area);
}

/// Centre a block of `width` within the given `area` horizontally.
fn centered_horizontal(width: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    Rect::new(x, area.y, w, area.height)
}

/// Split `text` into fixed-width character-wrapped lines.
///
/// Unlike word wrapping, this breaks at exact character boundaries so that
/// cursor positioning via `pos / width` and `pos % width` is always correct.
fn char_wrap<'a>(text: &'a str, width: usize) -> Vec<Line<'a>> {
    if width == 0 {
        return vec![Line::from(text)];
    }
    let mut lines = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + width).min(text.len());
        lines.push(Line::from(&text[start..end]));
        start = end;
    }
    if lines.is_empty() {
        lines.push(Line::from(""));
    }
    lines
}

// ---------------------------------------------------------------------------
// Chat screen (existing three-panel layout)
// ---------------------------------------------------------------------------

fn render_chat(frame: &mut Frame, app: &mut App) {
    // Compute chat input height based on wrapped text
    let chat_area = frame.area();
    let input_inner_width = chat_area.width.saturating_sub(2).max(1) as usize;
    let input_text_len = app.input.len() + 2; // "> " prefix
    let input_lines = ((input_text_len as f32) / (input_inner_width as f32))
        .ceil()
        .max(1.0) as u16;
    let input_height = input_lines + 2; // +2 for borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // status bar
            Constraint::Min(3),               // messages + optional log
            Constraint::Length(input_height), // input (dynamic)
        ])
        .split(chat_area);

    render_status_bar(frame, app, chunks[0]);

    // Split the middle area horizontally when the log panel is visible.
    if app.show_log {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // messages
                Constraint::Percentage(40), // log
            ])
            .split(chunks[1]);

        app.message_area = h_chunks[0];
        app.log_area = h_chunks[1];
        render_messages(frame, app, h_chunks[0]);
        apply_selection_highlight(frame, app, SelectionPane::Messages, h_chunks[0]);
        render_log_panel(frame, app, h_chunks[1]);
        apply_selection_highlight(frame, app, SelectionPane::Log, h_chunks[1]);
    } else {
        app.message_area = chunks[1];
        app.log_area = Rect::default();
        render_messages(frame, app, chunks[1]);
        apply_selection_highlight(frame, app, SelectionPane::Messages, chunks[1]);
    }

    app.input_area = chunks[2];
    render_input(frame, app, chunks[2]);
    apply_selection_highlight(frame, app, SelectionPane::Input, chunks[2]);

    // Slash menu dropdown (above the chat input, if active)
    if app.slash_menu.is_some() {
        render_slash_menu(frame, app, chunks[2]);
    }

    // File menu dropdown (above the chat input, if active)
    if app.file_menu.is_some() {
        render_file_menu(frame, app, chunks[2]);
    }

    if app.permission_pending.is_some() {
        render_permission_dialog(frame, app);
    }

    // Provider setup dialog overlay (if active, e.g. via /provider command)
    if app.provider_setup.is_some() {
        render_provider_setup_dialog(frame, app);
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

    if app.log_entries.is_empty() {
        app.log_max_scroll = 0;
        let empty = Paragraph::new(Line::from(Span::styled(
            "No log entries yet",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, inner);
        return;
    }

    // Build lines from log entries
    let lines: Vec<Line> = app
        .log_entries
        .iter()
        .map(|entry| {
            let ts = entry.timestamp.format("%H:%M:%S");
            let (level_str, level_color) = match entry.level {
                LogLevel::Info => ("INF", Color::Blue),
                LogLevel::Tool => ("TUL", Color::Cyan),
                LogLevel::Warn => ("WRN", Color::Yellow),
                LogLevel::Error => ("ERR", Color::Red),
            };
            Line::from(vec![
                Span::styled(format!("{ts} "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{level_str} "),
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&entry.message),
            ])
        })
        .collect();

    // Cache plain-text content for text selection copy
    app.log_content_lines = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false });

    // Use the rendered (wrapped) line count so the scroll reaches the true
    // bottom. `line_count(width)` accounts for word-wrapping; `lines.len()`
    // only counts logical lines and under-scrolls when entries are long.
    let total_lines = paragraph.line_count(inner.width) as u16;
    let visible_height = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_height);
    app.log_max_scroll = max_scroll;
    let scroll = app.log_scroll_offset.min(max_scroll);

    let paragraph = paragraph.scroll((max_scroll.saturating_sub(scroll), 0));

    frame.render_widget(paragraph, inner);

    // Render scrollbar when content overflows
    if total_lines > visible_height {
        let scroll_position = max_scroll.saturating_sub(scroll) as usize;
        let mut scrollbar_state =
            ScrollbarState::new(max_scroll as usize).position(scroll_position);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let session_display = app
        .session_id
        .as_deref()
        .map(|s| &s[..8.min(s.len())])
        .unwrap_or("none");

    let bar_style = Style::default()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    // Left side: ragent info + folder + branch (adjacent)
    let left_info = format!(
        " ● ragent  session: {}  agent: {}  tokens: {}/{}  [{}]",
        session_display, app.agent_name, app.token_usage.0, app.token_usage.1, app.status,
    );

    let folder_name = app.cwd.clone();

    let mut left_spans: Vec<Span<'_>> = vec![
        Span::styled(left_info.clone(), bar_style),
        Span::styled(
            format!("  {}", folder_name),
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(ref branch) = app.git_branch {
        left_spans.push(Span::styled(
            format!(" ⎇ {}", branch),
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Right side: log indicator + provider/model with health indicator
    let mut right_parts: Vec<Span<'_>> = Vec::new();

    if app.show_log {
        right_parts.push(Span::styled(
            "▪ log:on  ",
            Style::default().fg(Color::Yellow).bg(Color::DarkGray),
        ));
    } else {
        right_parts.push(Span::styled(
            "▪ log:off  ",
            Style::default().fg(Color::DarkGray).bg(Color::DarkGray),
        ));
    }

    if let Some(label) = app.provider_model_label() {
        let (icon, health_color) = match app.provider_health_status() {
            Some(true) => ("● ", Color::Green),
            Some(false) => ("✗ ", Color::Red),
            None => ("● ", Color::Yellow),
        };

        right_parts.push(Span::styled(
            icon.to_string(),
            Style::default()
                .fg(health_color)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ));
        right_parts.push(Span::styled(label, bar_style));
    }

    // Add background task status
    if !app.active_tasks.is_empty() {
        let running = app
            .active_tasks
            .iter()
            .filter(|t| t.status == ragent_core::task::TaskStatus::Running)
            .count();
        let task_status = format!("  ⚙️ {}/{}tasks", running, app.active_tasks.len());
        right_parts.push(Span::styled(
            task_status,
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ));
    }

    right_parts.push(Span::styled("  ", Style::default().bg(Color::DarkGray)));

    let left_len: usize = left_spans.iter().map(|s| s.content.len()).sum();
    let right_len: usize = right_parts.iter().map(|s| s.content.len()).sum();
    let gap = area
        .width
        .saturating_sub(left_len as u16 + right_len as u16);
    let gap_str = " ".repeat(gap as usize);

    let mut spans = left_spans;
    spans.push(Span::styled(gap_str, Style::default().bg(Color::DarkGray)));
    spans.extend(right_parts);

    let line = Line::from(spans);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

fn render_messages(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line<'_>> = Vec::new();

    for msg in &app.messages {
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
                                Span::raw(line),
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
                MessagePart::ToolCall { tool, call_id, state } => {
                    let step_tag = app
                        .tool_step_map
                        .get(call_id)
                        .map(|s| format!("[#{}] ", s))
                        .unwrap_or_default();
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
                            Style::default()
                                .fg(Color::Red)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "● ",
                            Style::default().fg(Color::DarkGray),
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, &app.cwd);

                    // Build the inline diff stats for edit tool (e.g. "(+25 -5)")
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
                        spans.push(Span::styled(format!("{} ", display_name), name_style));
                        spans.push(Span::styled(summary, Style::default().fg(Color::DarkGray)));
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

                    if state.status == ToolCallStatus::Completed {
                        if let Some(result) =
                            tool_result_summary(tool, &state.output, &state.input, &app.cwd)
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
            }

            lines.push(Line::from(""));
        }
    }

    // Cache plain-text content for text selection copy
    app.message_content_lines = lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect();

    // Build the paragraph with wrapping so we can measure the true rendered height.
    let messages_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .title(" Messages ");

    let paragraph = Paragraph::new(lines)
        .block(messages_block)
        .wrap(Wrap { trim: false });

    // Use line_count() which accounts for word-wrap at the inner width
    // (area width minus left+right borders).
    let inner_width = area.width.saturating_sub(2);
    let total = paragraph.line_count(inner_width) as u16;
    let visible = area.height.saturating_sub(2);
    let max_scroll = total.saturating_sub(visible);
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
    let input_text = format!("> {}", app.input);
    let inner_width = area.width.saturating_sub(2).max(1) as usize;

    // Character-wrap the text so cursor math (pos / width) stays correct
    let wrapped_lines = char_wrap(&input_text, inner_width);

    let block = Block::default().borders(Borders::ALL).title(" Input ");
    let paragraph = Paragraph::new(wrapped_lines).block(block);
    frame.render_widget(paragraph, area);

    // Position cursor accounting for wrapped lines
    let cursor_pos = app.input.len() + 2; // "> " prefix
    let cursor_line = cursor_pos / inner_width;
    let cursor_col = cursor_pos % inner_width;
    let cursor_x = area.x + 1 + cursor_col as u16;
    let cursor_y = area.y + 1 + cursor_line as u16;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_permission_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, frame.area());
    frame.render_widget(Clear, area);

    if let Some(ref req) = app.permission_pending {
        let text = vec![
            Line::from(Span::styled(
                "Permission Required",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(format!("Permission: {}", req.permission)),
            Line::from(format!(
                "Details: {}",
                req.patterns.first().map(|s| s.as_str()).unwrap_or("")
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[y]es  [a]lways  [n]o",
                Style::default().fg(Color::Cyan),
            )),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Permission ")
            .style(Style::default().fg(Color::Yellow));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
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
