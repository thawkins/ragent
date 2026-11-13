//! Project `Properties` dialog, opened from the sidebar's project heading
//! menu (spec SESSIONMENU).
//!
//! Three settings, each falling back to the global configuration when not
//! set: a description shown after the project's name in the sidebar, the
//! execution backend sessions created in the project use, and the provider
//! and model they start on. The dialog edits a copy; `Enter` saves all three
//! through the same [`ragent_agent::project::ProjectManager`] methods as
//! `/project describe`, `/project backend` and `/project model`, and `Esc`
//! discards.
//!
//! Keys: `Tab`/`Shift+Tab` (or `Up`/`Down`) move between fields; in the
//! description the usual editing keys apply; on a choice `Left`/`Right` step
//! through the enabled choices and `Space` opens the full list. A backend
//! that cannot run here (no runtime, no remote configured) is greyed out.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use ragent_agent::project::{MAX_DESCRIPTION_CHARS, parse_model_ref};
use ragent_config::ExecutionBackendKind;

use crate::app::App;

/// Width of the dialog, including its border.
const WIDTH: u16 = 66;
/// Most choices the open list shows at once.
const LIST_ROWS: usize = 10;

/// One entry of a choice field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Text shown for the entry.
    pub label: String,
    /// The stored value; `None` is the `Global (...)` entry.
    pub value: Option<String>,
    /// Whether it can be chosen; a disabled entry is greyed out.
    pub enabled: bool,
}

/// The field that has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The description text.
    Description,
    /// The execution backend choice.
    Backend,
    /// The provider/model choice.
    Model,
}

impl Field {
    /// The next field down (`forward`) or up, wrapping.
    fn step(self, forward: bool) -> Self {
        match (self, forward) {
            (Self::Description, true) | (Self::Model, false) => Self::Backend,
            (Self::Backend, true) | (Self::Description, false) => Self::Model,
            (Self::Model, true) | (Self::Backend, false) => Self::Description,
        }
    }
}

/// The open `Properties` dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProperties {
    /// The project being edited.
    pub project_id: String,
    /// Its name, for the title.
    pub project_name: String,
    /// The description being edited.
    pub description: String,
    /// Caret position in [`Self::description`], in characters.
    pub caret: usize,
    /// Backend choices: `Global (...)` first.
    pub backends: Vec<Choice>,
    /// Index of the chosen backend.
    pub backend: usize,
    /// Model choices: `Global (...)` first.
    pub models: Vec<Choice>,
    /// Index of the chosen model.
    pub model: usize,
    /// The field with the keyboard.
    pub focus: Field,
    /// The highlighted entry of the open choice list, if one is open.
    pub list: Option<usize>,
    /// Why the last save failed, shown at the bottom.
    pub error: Option<String>,
    /// Area painted last frame, for mouse hit-testing.
    pub area: Rect,
}

impl ProjectProperties {
    /// The choices and chosen index of the focused field, if it is a choice.
    fn focused_choices(&self) -> Option<(&[Choice], usize)> {
        match self.focus {
            Field::Description => None,
            Field::Backend => Some((&self.backends, self.backend)),
            Field::Model => Some((&self.models, self.model)),
        }
    }

    /// Sets the chosen index of the focused choice field.
    fn choose(&mut self, index: usize) {
        match self.focus {
            Field::Description => {}
            Field::Backend => self.backend = index,
            Field::Model => self.model = index,
        }
    }
}

/// Opens the dialog for project `project_id` with its stored settings.
///
/// # Errors
///
/// Returns a message when the project is not listed in the sidebar.
pub fn open(app: &mut App, project_id: &str) -> Result<(), String> {
    let project = app
        .sidebar
        .groups
        .iter()
        .find(|g| g.project_id == project_id)
        .map(|g| g.project.clone())
        .ok_or_else(|| "that project is no longer listed".to_string())?;
    let (backends, backend) = backend_choices(
        app,
        std::path::Path::new(&project.worktree),
        project.execution_backend.as_deref(),
    );
    let stored_model = match (&project.provider_id, &project.model_id) {
        (Some(provider), Some(model)) => Some(format!("{provider}/{model}")),
        _ => None,
    };
    let (models, model) = model_choices(app, stored_model.as_deref());
    let description = project.description.unwrap_or_default();
    app.sidebar.properties = Some(ProjectProperties {
        project_id: project.id,
        project_name: project.name,
        caret: description.chars().count(),
        description,
        backends,
        backend,
        models,
        model,
        focus: Field::Description,
        list: None,
        error: None,
        area: Rect::default(),
    });
    Ok(())
}

/// The backend choices for the project at `dir` (its own configuration
/// decides the global kind and what can run) and the index of `stored`
/// (`Global` when unset).
fn backend_choices(app: &App, dir: &std::path::Path, stored: Option<&str>) -> (Vec<Choice>, usize) {
    let global = app
        .session_processor
        .load_config_for(dir)
        .effective_execution_backend();
    let mut choices = vec![Choice {
        label: format!("Global ({})", global.as_str()),
        value: None,
        enabled: true,
    }];
    choices.extend(
        app.project_backend_choices(dir)
            .into_iter()
            .map(|(kind, why)| Choice {
                label: match &why {
                    Some(why) => format!("{} ({why})", kind.as_str()),
                    None => kind.as_str().to_string(),
                },
                value: Some(kind.as_str().to_string()),
                enabled: why.is_none(),
            }),
    );
    let chosen = stored
        .and_then(ExecutionBackendKind::from_label)
        .and_then(|kind| {
            choices
                .iter()
                .position(|c| c.value.as_deref() == Some(kind.as_str()))
        })
        .unwrap_or(0);
    (choices, chosen)
}

/// The model choices (every model of every configured provider) and the
/// index of `stored` (`Global` when unset). A stored model no provider
/// lists any more is kept as a choice so saving does not drop it.
fn model_choices(app: &App, stored: Option<&str>) -> (Vec<Choice>, usize) {
    let global = match app.storage.get_setting("selected_model") {
        Ok(Some(model)) if !model.is_empty() => model,
        Ok(_) => "none".to_string(),
        Err(e) => {
            tracing::warn!(error = %e, "global model not read");
            "none".to_string()
        }
    };
    let mut choices = vec![Choice {
        label: format!("Global ({global})"),
        value: None,
        enabled: true,
    }];
    for provider in App::get_configured_providers_for_router(&app.storage) {
        choices.extend(app.models_for_provider(&provider.id).into_iter().map(|m| {
            let value = format!("{}/{}", provider.id, m.id);
            Choice {
                label: value.clone(),
                value: Some(value),
                enabled: true,
            }
        }));
    }
    let chosen = match stored {
        None => 0,
        Some(stored) => match choices
            .iter()
            .position(|c| c.value.as_deref() == Some(stored))
        {
            Some(at) => at,
            None => {
                choices.insert(
                    1,
                    Choice {
                        label: stored.to_string(),
                        value: Some(stored.to_string()),
                        enabled: true,
                    },
                );
                1
            }
        },
    };
    (choices, chosen)
}

/// Saves the dialog's settings and closes it; on failure it stays open
/// with the reason.
fn save(app: &mut App) {
    let Some(dialog) = app.sidebar.properties.clone() else {
        return;
    };
    let manager = app.project_manager();
    let id = dialog.project_id.as_str();
    let backend = dialog.backends[dialog.backend]
        .value
        .as_deref()
        .and_then(ExecutionBackendKind::from_label);
    let model = dialog.models[dialog.model].value.clone();
    let result = (|| -> Result<(), String> {
        let model = model
            .as_deref()
            .map(parse_model_ref)
            .transpose()
            .map_err(|e| e.to_string())?;
        manager
            .set_description(id, Some(&dialog.description))
            .map_err(|e| e.to_string())?;
        manager
            .set_backend(id, backend)
            .map_err(|e| e.to_string())?;
        manager
            .set_base_model(id, model.as_ref())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            app.sidebar.properties = None;
            app.sidebar.dirty = true;
            app.status = format!(
                "{}: backend {}, model {}",
                dialog.project_name,
                dialog.backends[dialog.backend].label,
                dialog.models[dialog.model].label
            );
        }
        Err(e) => {
            if let Some(open) = app.sidebar.properties.as_mut() {
                open.error = Some(e);
            }
        }
    }
}

/// Handles a key while the dialog is open.
///
/// # Returns
///
/// `true` when the dialog is open (it consumes every key), `false` otherwise.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return false;
    };
    if let Some(cursor) = dialog.list {
        list_key(dialog, cursor, key);
        return true;
    }
    match key.code {
        KeyCode::Esc => app.sidebar.properties = None,
        KeyCode::Enter => save(app),
        KeyCode::Tab | KeyCode::Down => dialog.focus = dialog.focus.step(true),
        KeyCode::BackTab | KeyCode::Up => dialog.focus = dialog.focus.step(false),
        _ => match dialog.focus {
            Field::Description => edit_key(dialog, key),
            Field::Backend | Field::Model => choice_key(dialog, key),
        },
    }
    true
}

/// A key in the description field.
fn edit_key(dialog: &mut ProjectProperties, key: KeyEvent) {
    let len = dialog.description.chars().count();
    let byte = |text: &str, at: usize| text.char_indices().nth(at).map_or(text.len(), |(i, _)| i);
    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if len < MAX_DESCRIPTION_CHARS {
                let at = byte(&dialog.description, dialog.caret);
                dialog.description.insert(at, c);
                dialog.caret += 1;
            }
        }
        KeyCode::Backspace if dialog.caret > 0 => {
            dialog.caret -= 1;
            let at = byte(&dialog.description, dialog.caret);
            dialog.description.remove(at);
        }
        KeyCode::Delete if dialog.caret < len => {
            let at = byte(&dialog.description, dialog.caret);
            dialog.description.remove(at);
        }
        KeyCode::Left => dialog.caret = dialog.caret.saturating_sub(1),
        KeyCode::Right => dialog.caret = (dialog.caret + 1).min(len),
        KeyCode::Home => dialog.caret = 0,
        KeyCode::End => dialog.caret = len,
        _ => {}
    }
}

/// A key on a choice field: `Left`/`Right` step to the previous or next
/// enabled choice, `Space` opens the list.
fn choice_key(dialog: &mut ProjectProperties, key: KeyEvent) {
    let Some((choices, chosen)) = dialog.focused_choices() else {
        return;
    };
    let step = |forward: bool| {
        let n = choices.len();
        (1..n)
            .map(|k| {
                if forward {
                    (chosen + k) % n
                } else {
                    (chosen + n - k) % n
                }
            })
            .find(|&i| choices[i].enabled)
    };
    match key.code {
        KeyCode::Left => {
            if let Some(i) = step(false) {
                dialog.choose(i);
            }
        }
        KeyCode::Right => {
            if let Some(i) = step(true) {
                dialog.choose(i);
            }
        }
        KeyCode::Char(' ') => dialog.list = Some(chosen),
        _ => {}
    }
}

/// A key while a choice list is open: move, `Enter` picks an enabled
/// entry, `Esc` closes the list.
fn list_key(dialog: &mut ProjectProperties, cursor: usize, key: KeyEvent) {
    let Some((choices, _)) = dialog.focused_choices() else {
        dialog.list = None;
        return;
    };
    let last = choices.len().saturating_sub(1);
    let enabled = choices.get(cursor).is_some_and(|c| c.enabled);
    dialog.list = match key.code {
        KeyCode::Up => Some(cursor.saturating_sub(1)),
        KeyCode::Down => Some((cursor + 1).min(last)),
        KeyCode::PageUp => Some(cursor.saturating_sub(LIST_ROWS)),
        KeyCode::PageDown => Some((cursor + LIST_ROWS).min(last)),
        KeyCode::Home => Some(0),
        KeyCode::End => Some(last),
        KeyCode::Enter if enabled => {
            dialog.choose(cursor);
            None
        }
        KeyCode::Esc => None,
        _ => Some(cursor),
    };
}

/// Handles pasted `text` while the dialog is open: into the description at
/// the caret (line breaks become spaces, cut at the length limit), and
/// ignored on a choice field.
///
/// # Returns
///
/// `true` when the dialog is open (it takes every paste), `false` otherwise.
pub fn handle_paste(app: &mut App, text: &str) -> bool {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return false;
    };
    if dialog.focus == Field::Description && dialog.list.is_none() {
        let room = MAX_DESCRIPTION_CHARS.saturating_sub(dialog.description.chars().count());
        let pasted: String = text
            .chars()
            .filter(|&c| c != '\r')
            .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
            .take(room)
            .collect();
        let at = dialog
            .description
            .char_indices()
            .nth(dialog.caret)
            .map_or(dialog.description.len(), |(i, _)| i);
        dialog.description.insert_str(at, &pasted);
        dialog.caret += pasted.chars().count();
    }
    true
}

/// Handles a mouse event while the dialog is open: it consumes every event,
/// and a click outside it closes it.
///
/// # Returns
///
/// `true` when the dialog is open, `false` otherwise.
pub fn handle_mouse(app: &mut App, event: MouseEvent) -> bool {
    let Some(dialog) = app.sidebar.properties.as_ref() else {
        return false;
    };
    let area = dialog.area;
    let inside = event.column >= area.x
        && event.column < area.x + area.width
        && event.row >= area.y
        && event.row < area.y + area.height;
    if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) && !inside {
        app.sidebar.properties = None;
    }
    true
}

/// Paints the dialog, centred, when it is open.
pub fn render(frame: &mut Frame, app: &mut App) {
    let Some(dialog) = app.sidebar.properties.as_mut() else {
        return;
    };
    let screen = frame.area();
    let width = WIDTH.min(screen.width);
    let height = 14u16.min(screen.height);
    let area = Rect {
        x: screen.x + (screen.width - width) / 2,
        y: screen.y + (screen.height - height) / 2,
        width,
        height,
    };
    dialog.area = area;
    let inner = usize::from(width.saturating_sub(4));
    let label = |text: &str, field: Field| {
        let style = if dialog.focus == field {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Span::styled(format!(" {text:<12}"), style)
    };
    let value_width = inner.saturating_sub(13);
    let boxed = |text: &str, focused: bool| {
        let shown = crate::sessions_sidebar::fit(text, value_width.saturating_sub(2));
        let style = if focused {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };
        Span::styled(
            format!("[{shown:<w$}]", w = value_width.saturating_sub(2)),
            style,
        )
    };
    let description = if dialog.focus == Field::Description {
        caret_window(
            &dialog.description,
            dialog.caret,
            value_width.saturating_sub(2),
        )
    } else {
        dialog.description.clone()
    };
    let dim = Style::default().fg(Color::DarkGray);
    let shown_as = if dialog.description.trim().is_empty() {
        format!("shown as: {}", dialog.project_name)
    } else {
        format!(
            "shown as: {} ({})",
            dialog.project_name,
            dialog.description.trim()
        )
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            label("Description", Field::Description),
            boxed(&description, dialog.focus == Field::Description),
        ]),
        Line::from(Span::styled(
            format!(
                "{:13}{}",
                "",
                crate::sessions_sidebar::fit(&shown_as, value_width)
            ),
            dim,
        )),
        Line::from(""),
        Line::from(vec![
            label("Backend", Field::Backend),
            boxed(
                &format!("{} v", dialog.backends[dialog.backend].label),
                dialog.focus == Field::Backend,
            ),
        ]),
        Line::from(Span::styled(
            format!("{:13}new sessions only; existing ones keep theirs", ""),
            dim,
        )),
        Line::from(""),
        Line::from(vec![
            label("Model", Field::Model),
            boxed(
                &format!("{} v", dialog.models[dialog.model].label),
                dialog.focus == Field::Model,
            ),
        ]),
        Line::from(""),
    ];
    match &dialog.error {
        Some(error) => lines.push(Line::from(Span::styled(
            format!(" {}", crate::sessions_sidebar::fit(error, inner)),
            Style::default().fg(Color::Red),
        ))),
        None => lines.push(Line::from("")),
    }
    lines.push(Line::from(Span::styled(
        " Tab move  Left/Right change  Space list  Enter save  Esc cancel",
        dim,
    )));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" Properties: {} ", dialog.project_name));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).block(block), area);
    if let Some(cursor) = dialog.list {
        paint_list(frame, dialog, cursor, area);
    }
}

/// `text` with a `_` caret at `caret`, cut to the `width` characters
/// around the caret so the caret and the text next to it stay visible
/// while the rest scrolls out of view.
fn caret_window(text: &str, caret: usize, width: usize) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    chars.insert(caret, '_');
    let start = (caret + 1).saturating_sub(width);
    chars.into_iter().skip(start).take(width).collect()
}

/// Paints the open choice list below its field.
fn paint_list(frame: &mut Frame, dialog: &ProjectProperties, cursor: usize, dialog_area: Rect) {
    let Some((choices, chosen)) = dialog.focused_choices() else {
        return;
    };
    let row = match dialog.focus {
        Field::Backend => 5,
        Field::Description | Field::Model => 8,
    };
    let rows = choices.len().min(LIST_ROWS);
    let screen = frame.area();
    let height = u16::try_from(rows + 2).unwrap_or(u16::MAX);
    let width = dialog_area.width.saturating_sub(14);
    let area = Rect {
        x: dialog_area.x + 13,
        y: (dialog_area.y + row).min(screen.y + screen.height.saturating_sub(height)),
        width,
        height: height.min(screen.height),
    };
    let top = cursor.saturating_sub(rows.saturating_sub(1));
    let text_width = usize::from(width.saturating_sub(2));
    let lines: Vec<Line> = choices
        .iter()
        .enumerate()
        .skip(top)
        .take(rows)
        .map(|(i, choice)| {
            let mark = if i == chosen { '*' } else { ' ' };
            let text = format!(
                "{mark} {}",
                crate::sessions_sidebar::fit(&choice.label, text_width.saturating_sub(2))
            );
            let style = if i == cursor {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if choice.enabled {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(format!("{text:<text_width$}"), style))
        })
        .collect();
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        area,
    );
}
