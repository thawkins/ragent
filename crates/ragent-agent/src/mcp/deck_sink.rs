//! PowerPoint export of a diff deck through an MCP server (spec `diffdeck` FR-030).
//!
//! `ragent-research` deliberately does not know how a presentation is written;
//! it exposes [`ragent_research::deck_pptx::DeckSlideSink`] and hands over a
//! [`ragent_research::deck_pptx::DeckSlidePlan`]. This module is the transport
//! that turns that plan into a `.pptx` by driving an MCP server that can create
//! presentation files.
//!
//! ## Server selection
//!
//! Nothing is hard-wired to one vendor. A connected server qualifies when it
//! advertises a tool that creates a presentation *and* has a way to write it to
//! a file. The check is a name/description match ([`looks_like_deck_server`]) so
//! a new PowerPoint-capable server works without a code change, and a workspace
//! with a spreadsheet server or a document server does not get mistaken for one.
//!
//! ## Why the export is best-effort
//!
//! The markdown deck is the canonical artifact and is already on disk before
//! this sink is called. Every failure here is reported as
//! [`DeckPptxOutcome::Failed`] or [`DeckPptxOutcome::Unavailable`] and never as
//! an error, so a broken or absent MCP server costs the user nothing.
//!
//! ## Drive protocol
//!
//! A deck server is driven through its own generic command channel when it has
//! one (DeckCraft's `run_command`, whose ids are the server's public API) and
//! through `presentation.create`-style tools otherwise. Both paths build the
//! same sequence: create the presentation, add one slide per planned slide with
//! its title and body, then save to `plan.path`.

use std::sync::Arc;

use async_trait::async_trait;
use ragent_research::deck::ItemStatus;
use ragent_research::deck_pptx::{
    DeckPptxOutcome, DeckSlide, DeckSlidePlan, DeckSlideSink, DeckTableRow, SlideKind,
};
use ragent_research::deck_progress::{self, DeckEvent, DeckProgress, DeckStage};
use serde_json::{Value, json};
use tokio::sync::RwLock;

use crate::mcp::{McpClient, McpServer, McpStatus, McpToolDef};

/// An MCP tool that qualifies a server as able to write a presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckTool {
    /// The server that advertises the tool.
    pub server_id: String,
    /// The tool that creates a presentation file.
    pub create_tool: String,
    /// The generic command tool, when the server has one (DeckCraft's
    /// `run_command`, `execute_command`, or `execute`).
    pub command_tool: Option<String>,
    /// The tool that adds a slide, when the server has no command tool.
    pub add_slide_tool: Option<String>,
    /// The tool that writes the file, when the server has no command tool.
    pub save_tool: Option<String>,
}

impl DeckTool {
    /// A label naming the server and tool, fit for a user-facing report.
    #[must_use]
    pub fn provider(&self) -> String {
        format!("{}/{}", self.server_id, self.create_tool)
    }
}

/// The presentation nouns that identify a deck-writing server.
///
/// A server is only a deck writer if its tool manifest names a presentation, so a
/// spreadsheet or document server is never mistaken for one. `deckcraft` is in the
/// list because that is the product noun the server calls its own output.
const DECK_NOUNS: [&str; 6] = [
    "presentation",
    "powerpoint",
    "pptx",
    "slide deck",
    "slide-deck",
    "deckcraft",
];

/// The creation verbs a deck-writing server advertises.
///
/// They are matched against vendor *descriptions*, not tool names, so they are
/// English verbs rather than an alias table - but `write` and `save` are retired
/// tool names (spec `uniformtools` FR-026), and a bare `"write"` literal here is
/// exactly the erosion FR-027 forbids. Keeping the set in one named constant,
/// spelled with the verbs a presentation server actually publishes, is what stops
/// the next edit from reaching for the retired spelling by accident.
const DECK_VERBS: [&str; 6] = ["create", "new", "add", "save", "export", "generate"];

/// The verbs that mean "write the file" in a save-tool predicate.
///
/// `write` is absent for the same reason: a save tool says `save`, `export` or
/// `write_file`, and none of those is a retired tool name.
const SAVE_VERBS: [&str; 3] = ["save", "export", "write_file"];

/// Whether a server's tool manifest makes it a presentation writer.
///
/// The match is on lowercased tool names and descriptions, so it covers the
/// common naming shapes (`create_presentation`, `presentation_create`,
/// `pptx_create`, `new_presentation`, `save_presentation`) without pinning one
/// vendor. A tool that only *reads* presentations never qualifies, and a
/// spreadsheet or document server is excluded by the presentation noun.
fn looks_like_deck_server(server: &McpServer) -> Option<DeckTool> {
    if !matches!(server.status, McpStatus::Connected) {
        return None;
    }
    let connected = &server.tools;
    if connected.is_empty() {
        return None;
    }

    let find = |predicate: &dyn Fn(&McpToolDef) -> bool| -> Option<&McpToolDef> {
        connected.iter().find(|tool| predicate(tool))
    };
    // The "is this a deck tool" test: a creation verb plus a presentation noun.
    let mentions_deck = |tool: &McpToolDef| {
        let haystack = format!("{} {}", tool.name, tool.description).to_lowercase();
        DECK_NOUNS.iter().any(|noun| haystack.contains(noun))
            && DECK_VERBS.iter().any(|verb| haystack.contains(verb))
    };

    let create_tool = find(&mentions_deck)?;
    let command_tool = find(&|tool| {
        let name = tool.name.to_lowercase();
        matches!(
            name.as_str(),
            "run_command" | "execute_command" | "execute" | "batch" | "deckcraft_batch"
        )
    });
    // Without a command channel the plan needs an explicit add-slide and save.
    let add_slide_tool = find(&|tool| {
        let haystack = format!("{} {}", tool.name, tool.description).to_lowercase();
        haystack.contains("slide")
            && (haystack.contains("add") || haystack.contains("insert") || haystack.contains("new"))
    });
    let save_tool = find(&|tool| {
        let haystack = format!("{} {}", tool.name, tool.description).to_lowercase();
        SAVE_VERBS.iter().any(|verb| haystack.contains(verb))
            && DECK_NOUNS.iter().any(|noun| haystack.contains(noun))
    });

    // A command channel carries add-slide and save inside it; without one both
    // must exist as tools or the plan could never be written.
    if command_tool.is_none() && (add_slide_tool.is_none() || save_tool.is_none()) {
        return None;
    }

    Some(DeckTool {
        server_id: server.id.clone(),
        create_tool: create_tool.name.clone(),
        command_tool: command_tool.map(|tool| tool.name.clone()),
        add_slide_tool: add_slide_tool.map(|tool| tool.name.clone()),
        save_tool: save_tool.map(|tool| tool.name.clone()),
    })
}

/// Pick the connected server that can write a presentation.
///
/// Servers are considered in id order so the choice is stable for a given set
/// of connected servers, and the first qualifying one wins.
///
/// A server that also publishes the wrapper tools the drive protocol prefers
/// (`add_slide`, `save_presentation`) has them recorded in `DeckTool` alongside
/// (`add_slide_tool`/`save_tool`), so the command path uses them instead of the
/// raw command ids; see [`McpDeckSink::write_via_command`].
#[must_use]
pub fn select_deck_tool(servers: &[McpServer]) -> Option<DeckTool> {
    let mut ordered: Vec<&McpServer> = servers.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    ordered.into_iter().find_map(looks_like_deck_server)
}

/// A [`DeckSlideSink`] that writes the presentation through an MCP server.
///
/// The client is shared rather than owned: it is the same `McpClient` the
/// session uses for every other MCP call, so a server the user already
/// connected is used as-is and no second connection is spawned.
pub struct McpDeckSink {
    client: Arc<RwLock<McpClient>>,
}

impl McpDeckSink {
    /// Build a sink over an existing MCP client.
    #[must_use]
    pub fn new(client: Arc<RwLock<McpClient>>) -> Self {
        Self { client }
    }

    /// The tool this sink would use, or `None` when no server qualifies.
    ///
    /// Public so a surface can say *why* no presentation was produced instead of
    /// reporting a bare absence.
    pub async fn available_tool(&self) -> Option<DeckTool> {
        let guard = self.client.read().await;
        select_deck_tool(guard.servers())
    }

    /// Call a wrapper tool and return the payload it answered with.
    ///
    /// The create tool is a tool, not a command id, so it goes through
    /// [`McpClient::call_tool`] directly; its answer carries the slide count the
    /// drive protocol needs to know whether the presentation arrived with a
    /// default slide.
    async fn call_tool_value(
        &self,
        client: &McpClient,
        server_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, String> {
        let result = client
            .call_tool(server_id, tool_name, arguments)
            .await
            .map_err(|e| format!("`{tool_name}` failed: {e}"))?;
        if result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!("`{tool_name}` rejected: {}", result_text(&result)));
        }
        let payload = result
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| blocks.first())
            .and_then(|block| block.get("text"))
            .and_then(Value::as_str)
            .map(|text| serde_json::from_str(text).unwrap_or(Value::Null))
            .unwrap_or(Value::Null);
        Ok(payload)
    }

    /// Drive the server through its generic command channel.
    ///
    /// The commands are the deck server's own public API: create a
    /// presentation, then build each planned slide, then save to the target.
    /// Every command also has a dedicated MCP wrapper tool of the same
    /// identity (`new_presentation` for `file.new`, `add_slide` for `slide.new`),
    /// and the wrappers are what a server actually implements: DeckCraft answers
    /// `unknown command \`presentation.new\`` because its command ids are
    /// `file.new` and `file.save` while the wrapper names are the stable,
    /// vendor-independent surface. The wrappers are therefore preferred whenever
    /// the server advertises them, and the raw command ids are only the fallback
    /// for a command-channel server that has none.
    ///
    /// A server that rejects a tool reports `is_error`, which becomes an `Err`
    /// naming the tool, so a protocol change surfaces as a readable failure
    /// rather than a half-written file.
    async fn write_via_command(
        &self,
        tool: &DeckTool,
        plan: &DeckSlidePlan,
        command_tool: &str,
        progress: Option<&dyn DeckProgress>,
    ) -> Result<(), String> {
        let client = self.client.read().await;

        // The wrapper tools this server advertises, if any. A server that
        // publishes them is driven through them; the raw command ids are only a
        // fallback for a command-channel server that publishes none.
        let has_tool = |name: &str| {
            client.servers().iter().any(|server| {
                server.id == tool.server_id && server.tools.iter().any(|t| t.name == name)
            })
        };
        let add_wrapper = has_tool("add_slide").then_some("add_slide");
        let save_wrapper = has_tool("save_presentation").then_some("save_presentation");
        // The individual slide-building commands. A command channel without them
        // still gets a readable deck (title and body), just without the layout
        // refinements; `slide_builder` reports whether they are usable.
        let builder = has_tool("run_command").then(|| SlideBuilder {
            tool: command_tool,
            can_style: self.supports_styling(&tool.server_id),
        });

        // A new presentation arrives with one blank title slide already in it.
        // The plan's first slide is that cover, so the slide is reused rather
        // than drawn beside: creating it here and appending every planned slide
        // is what left an empty `ctrTitle`/`subTitle` cover at the front of the
        // deck and made the file one slide longer than the plan.
        let created = self
            .call_tool_value(&client, &tool.server_id, &tool.create_tool, json!({}))
            .await?;
        // Only a styled run reuses the default slide: without a builder the slide
        // cannot be re-laid-out or written, so a fresh one is added instead.
        let keep_default =
            builder.is_some() && created.get("slides").and_then(Value::as_u64) != Some(0);

        if let (true, Some(builder), Some(cover)) = (
            keep_default,
            builder.as_ref(),
            plan.slides
                .first()
                .filter(|slide| slide.kind == SlideKind::Cover),
        ) {
            // Make the cover the planned cover before anything is drawn on it: the
            // default slide is the `title` layout, whose `ctrTitle`/`subTitle`
            // placeholders the builder does not style.
            builder
                .command(
                    &client,
                    &tool.server_id,
                    "slide.layout",
                    json!({ "layout": cover.layout(), "index": 0 }),
                )
                .await?;
        }

        let total_slides = plan.slides.len();
        for (index, slide) in plan.slides.iter().enumerate() {
            // FR-031: report before drawing, so a slow server is visible while the
            // slide is being worked on rather than only once it is written.
            deck_progress::report(progress, deck_slide_event(slide, index, total_slides));
            // Slide indices are 0-based throughout the deck server's API, so the
            // plan's first slide is index 0 - not the 1 that `slide.new` answers
            // with, which is the 0-based index of the slide it just added.
            let reuse_default = keep_default && index == 0;
            let arguments = json!({
                "layout": slide.layout(),
                "title": slide.title,
                "body": slide.plain_body(),
            });
            if reuse_default {
                // The default slide is already the cover: it needs the plan's text,
                // not a second slide beside it.
                builder
                    .as_ref()
                    .ok_or("a reused cover needs the slide builder")?
                    .set_text(
                        &client,
                        &tool.server_id,
                        index,
                        &slide.title,
                        &slide.plain_body(),
                    )
                    .await?;
            } else {
                match add_wrapper {
                    Some(wrapper) => {
                        self.call_checked(&client, &tool.server_id, wrapper, arguments)
                            .await?;
                    }
                    None => {
                        self.call_checked(
                            &client,
                            &tool.server_id,
                            command_tool,
                            json!({ "command": "slide.new", "params": arguments }),
                        )
                        .await?;
                    }
                }
            }
            if let Some(builder) = &builder {
                // The slide must be made *current* before anything is drawn on
                // it. `shape.insert` and its siblings act on the selected slide,
                // and a fresh `slide.new` does not select what it created, so
                // dressing without navigating painted every accent onto the
                // previous slide - which is exactly how the first deck came out
                // plain. Indices are 0-based, the same numbering `slide.inspect`
                // and `slide.go` take.
                //
                // [`SlideBuilder::dress`] navigates itself (it must, before its
                // `edit.select`s resolve), so the navigation is only taken here
                // when dressing is skipped - otherwise every slide is navigated
                // to twice, and the deck server is asked for a slide change it
                // has already performed.
                builder
                    .dress(&client, &tool.server_id, index, slide)
                    .await?;
                if !builder.can_style {
                    self.call_checked(
                        &client,
                        &tool.server_id,
                        command_tool,
                        json!({ "command": "slide.go", "params": { "index": index } }),
                    )
                    .await?;
                }
            }
        }

        let path = plan.path.display().to_string();
        match save_wrapper {
            Some(wrapper) => {
                self.call_checked(&client, &tool.server_id, wrapper, json!({ "path": path }))
                    .await
            }
            None => {
                self.call_checked(
                    &client,
                    &tool.server_id,
                    command_tool,
                    json!({ "command": "file.save", "params": { "path": path } }),
                )
                .await
            }
        }
    }

    /// Whether the named server has a command channel the builder can drive.
    ///
    /// A command channel is a *verb* the server exposes once (`run_command`),
    /// carrying command ids (`shape.insert`, `format.fontDialog`, ...) that are
    /// not tools at all. Requiring each command id to also appear as a tool name
    /// matched nothing, so the builder never ran and every slide stayed a plain
    /// heading over a paragraph. What actually decides whether a command works is
    /// the server's answer to it, and [`SlideBuilder::command`] reports a refusal
    /// precisely so the first unsupported command is visible. A channel that
    /// refuses everything therefore costs a readable deck rather than a failed
    /// export, which is what keeps the export additive.
    fn supports_styling(&self, server_id: &str) -> bool {
        let Ok(client) = self.client.try_read() else {
            return false;
        };
        client.servers().iter().any(|server| {
            server.id == server_id
                && server.tools.iter().any(|tool| {
                    matches!(
                        tool.name.to_lowercase().as_str(),
                        "run_command" | "execute_command" | "execute" | "batch"
                    )
                })
        })
    }

    /// Drive the server through per-operation tools.
    async fn write_via_tools(
        &self,
        tool: &DeckTool,
        plan: &DeckSlidePlan,
        add_slide_tool: &str,
        save_tool: &str,
        progress: Option<&dyn DeckProgress>,
    ) -> Result<(), String> {
        let client = self.client.read().await;
        self.call_checked(&client, &tool.server_id, &tool.create_tool, json!({}))
            .await?;
        let total_slides = plan.slides.len();
        for (index, slide) in plan.slides.iter().enumerate() {
            deck_progress::report(progress, deck_slide_event(slide, index, total_slides));
            let body = slide.plain_body();
            self.call_checked(
                &client,
                &tool.server_id,
                add_slide_tool,
                json!({
                    "layout": slide.layout(),
                    "title": slide.title,
                    "body": body,
                    "text": body,
                }),
            )
            .await?;
        }
        self.call_checked(
            &client,
            &tool.server_id,
            save_tool,
            json!({ "path": plan.path.display().to_string() }),
        )
        .await
    }
}

/// The presentation helpers this module renders a planned slide with.
trait SlideRender {
    fn layout(&self) -> &'static str;
    fn plain_body(&self) -> String;
    fn presenter_notes(&self) -> String;
}

impl McpDeckSink {
    /// Call one MCP tool and turn a server-side rejection into an error.
    ///
    /// [`McpClient::call_tool`] only fails when the *transport* fails; a tool
    /// the server refuses comes back as a successful call whose `is_error` is
    /// `true` and whose text carries the reason. Ignoring that flag is what let
    /// an export report success for a presentation that was never created, so
    /// every call this module makes goes through here.
    async fn call_checked(
        &self,
        client: &McpClient,
        server_id: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<(), String> {
        let result = client
            .call_tool(server_id, tool_name, arguments)
            .await
            .map_err(|e| format!("`{tool_name}` failed: {e}"))?;
        if result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!("`{tool_name}` rejected: {}", result_text(&result)));
        }
        Ok(())
    }
}

impl SlideRender for DeckSlide {
    /// The DeckCraft layout each slide kind is built on.
    ///
    /// A cover takes `titleAndContent` rather than `title`: the `title` layout
    /// carries a `ctrTitle` and a `subTitle` placeholder, which
    /// [`SlideBuilder::shape_id`] does not recognise - so the cover's counts and
    /// coverage line were set nowhere and the title slide came out blank. The
    /// `titleAndContent` placeholders are the ones the builder already styles.
    fn layout(&self) -> &'static str {
        "titleAndContent"
    }

    fn plain_body(&self) -> String {
        // The body is a bullet list, not a paragraph: a slide's text is read at a
        // glance, and a block of prose is skimmed past. The lead opens as its own
        // line, and the items follow as bullets so each one is a separate glance.
        let mut lines = Vec::new();
        if !self.lead.is_empty() {
            lines.push(self.lead.clone());
        }
        for bullet in &self.bullets {
            lines.push(format!("\u{2022} {bullet}"));
        }
        if let Some(rows) = &self.table {
            for row in rows {
                let value = if row.value.is_empty() {
                    String::new()
                } else {
                    format!("  -  {}", row.value)
                };
                lines.push(format!("\u{2022} {}{value}", row.label));
            }
        }
        lines.join("\n")
    }

    fn presenter_notes(&self) -> String {
        let mut notes = Vec::new();
        if !self.lead.is_empty() {
            notes.push(self.lead.clone());
        }
        for bullet in &self.bullets {
            notes.push(format!("- {bullet}"));
        }
        notes.join("\n")
    }
}

/// The geometry and palette one deck is drawn in.
///
/// A 960x540 pt 16:9 canvas, which is what DeckCraft's default theme is sized
/// for. Everything the builder places is expressed in these points, so the layout
/// is one coherent grid rather than a set of numbers scattered through the code.
mod style {
    /// Slide width in points.
    pub const WIDTH: f64 = 960.0;
    /// Slide height in points.
    pub const HEIGHT: f64 = 540.0;
    /// Left content margin.
    pub const MARGIN: f64 = 64.0;
    /// Right content edge.
    pub const RIGHT: f64 = WIDTH - MARGIN;
    /// Text column width.
    pub const COLUMN: f64 = RIGHT - MARGIN;
    /// The status accent bar down the left edge.
    pub const ACCENT_X: f64 = 40.0;
    /// The accent bar's top edge.
    pub const ACCENT_Y: f64 = 96.0;
    /// The accent bar's width.
    pub const ACCENT_W: f64 = 8.0;
    /// The accent bar's height.
    pub const ACCENT_H: f64 = 96.0;
    /// The status badge's left edge.
    pub const BADGE_X: f64 = WIDTH - MARGIN - 112.0;
    /// The status badge's top edge.
    pub const BADGE_Y: f64 = 36.0;
    /// The status badge's width.
    pub const BADGE_W: f64 = 112.0;
    /// The status badge's height.
    pub const BADGE_H: f64 = 26.0;
    /// The body text box's top edge, clear of the title and badge.
    pub const BODY_Y: f64 = 150.0;
    /// The body text box's height.
    pub const BODY_H: f64 = HEIGHT - BODY_Y - 56.0;
    /// Title font size in points.
    pub const TITLE_SIZE: f64 = 30.0;
    /// Body font size in points.
    pub const BODY_SIZE: f64 = 16.0;
    /// Body line spacing, as a multiple of the font size.
    pub const BODY_LEADING: f64 = 1.25;

    /// Ink: titles and body text.
    pub const INK: &str = "#1F2A44";
    /// Table header band.
    pub const BAND: &str = "#EEF1F6";
    /// Hairline rules and table borders.
    pub const RULE: &str = "#D8DCE4";

    /// The colour a status badge is drawn in.
    #[must_use]
    pub fn status_colour(tag: &str) -> &'static str {
        match tag {
            "NEW" => "#1D6FB8",
            "ADDED" => "#2E8B57",
            "UPDATE" => "#B8860B",
            _ => "#8A8F99",
        }
    }
}

/// The per-slide command sequence that turns a planned slide into a laid-out one.
///
/// The plain `add_slide` call only knows a title and a body, so every deck it
/// produced was a heading above a paragraph. This builder drives the server's
/// own shape, table and type commands to place a status accent, a title, a
/// lead, bullets, and tables on the slide, which is what separates a deck from a
/// text dump. It is best-effort per command: a server that lacks one command
/// skips it rather than failing the deck.
struct SlideBuilder<'a> {
    tool: &'a str,
    can_style: bool,
}

impl SlideBuilder<'_> {
    /// Apply the layout for one slide, styling each shape by its own id.
    ///
    /// DeckCraft splits its commands in two. A *shape* command (`shape.fill`,
    /// `shape.line`, `shape.setBounds`) takes `ids` and acts on those shapes. A
    /// *format* command (`format.fontDialog`, `format.anchor`, `format.margins`,
    /// `format.bullets`, `format.paragraphDialog`, `format.align`) takes no `ids`
    /// at all: it acts on the current selection and silently ignores an `ids` it
    /// is handed. Passing `ids` to both was the bug that emptied every body box -
    /// the badge, inserted last, stayed selected, so a body's `format.fontDialog`
    /// restyled the badge and the body was left with the plain 18pt ink the
    /// paragraph came in with. Every format command is therefore preceded by an
    /// `edit.select` of the one shape it is meant to restyle.
    async fn dress(
        &self,
        client: &McpClient,
        server_id: &str,
        index: usize,
        slide: &DeckSlide,
    ) -> Result<(), String> {
        if !self.can_style {
            return Ok(());
        }
        // Make the slide current first: the id-based commands still resolve
        // against the open slide.
        self.command(client, server_id, "slide.go", json!({ "index": index }))
            .await?;

        // The accent bar is the one mark every slide shares: it carries the status
        // colour on a finding slide and the ink colour otherwise, so the deck reads
        // as one system across its kinds.
        let accent = slide
            .status
            .map(|status| style::status_colour(status.tag()))
            .unwrap_or(style::INK);
        if let Some(id) = self
            .command_id(
                client,
                server_id,
                "shape.insert",
                json!({
                    "preset": "rect",
                    "rect": [style::ACCENT_X, style::ACCENT_Y, style::ACCENT_W, style::ACCENT_H],
                    "text": "",
                }),
            )
            .await?
        {
            self.command(
                client,
                server_id,
                "shape.fill",
                json!({ "color": accent, "ids": [id] }),
            )
            .await?;
            self.command(
                client,
                server_id,
                "shape.line",
                json!({ "none": true, "ids": [id] }),
            )
            .await?;
        }

        if let Some(status) = slide.status {
            self.badge(client, server_id, status).await?;
        }

        // The title is pulled down clear of the badge and set in the deck's ink.
        if let Some(id) = self.shape_id(client, server_id, "title", index).await? {
            self.command(
                client,
                server_id,
                "shape.setBounds",
                json!({ "x": style::MARGIN, "y": 28.0, "w": style::COLUMN - 130.0, "h": 84.0, "ids": [id] }),
            )
            .await?;
            self.select(client, server_id, id).await?;
            self.command(
                client,
                server_id,
                "format.fontDialog",
                json!({ "bold": true, "color": style::INK, "size": style::TITLE_SIZE }),
            )
            .await?;
            self.command(
                client,
                server_id,
                "format.anchor",
                json!({ "anchor": "bottom" }),
            )
            .await?;
        }

        // The body placeholder is a text box: give it its own type size, leading
        // and bullets so a finding reads as a lead plus scannable points rather
        // than one dense paragraph. It is restyled before the table is placed,
        // because placing a table moves the selection off it.
        if let Some(id) = self.shape_id(client, server_id, "body", index).await? {
            self.command(
                client,
                server_id,
                "shape.setBounds",
                json!({ "x": style::MARGIN, "y": style::BODY_Y, "w": style::COLUMN, "h": style::BODY_H, "ids": [id] }),
            )
            .await?;
            self.select(client, server_id, id).await?;
            self.command(
                client,
                server_id,
                "format.fontDialog",
                json!({ "bold": false, "color": style::INK, "size": style::BODY_SIZE }),
            )
            .await?;
            self.command(
                client,
                server_id,
                "format.anchor",
                json!({ "anchor": "top" }),
            )
            .await?;
            self.command(
                client,
                server_id,
                "format.margins",
                json!({ "left": 8.0, "right": 8.0, "top": 6.0, "bottom": 6.0 }),
            )
            .await?;
            if !slide.bullets.is_empty() || slide.table.is_some() {
                self.command(
                    client,
                    server_id,
                    "format.bullets",
                    json!({ "on": true, "char": "\u{2022}" }),
                )
                .await?;
            }
            self.command(
                client,
                server_id,
                "format.paragraphDialog",
                json!({ "lineSpacing": style::BODY_LEADING, "spaceAfter": 8 }),
            )
            .await?;
        }

        if let Some(rows) = &slide.table {
            self.table(client, server_id, rows).await?;
        }

        // Notes carry the text the slide itself does not show, so a presenter
        // still has the full item to speak from.
        let notes = slide.presenter_notes();
        if !notes.is_empty() {
            self.command(
                client,
                server_id,
                "slide.notes",
                json!({ "index": index, "text": notes }),
            )
            .await?;
        }
        Ok(())
    }

    /// Write a planned slide's title and body into a slide that already exists.
    ///
    /// `slide.new` populates the placeholders it creates; a presentation that
    /// arrives with one blank slide does not, and adding a second slide beside it
    /// is what left an empty cover at the front of the deck. The text is written
    /// through the same two placeholders `slide.new` would have filled, so a
    /// reused slide comes out identical to a created one.
    ///
    /// The title and body are resolved before either is written: setting one
    /// placeholder's text re-runs the shape lookup, so a lookup taken afterwards
    /// would be reading the slide the write had just changed.
    async fn set_text(
        &self,
        client: &McpClient,
        server_id: &str,
        index: usize,
        title: &str,
        body: &str,
    ) -> Result<(), String> {
        let mut placeholders: Vec<(&str, u64)> = Vec::with_capacity(2);
        for (role, key, text) in [("title", "title", title), ("body", "obj", body)] {
            if text.is_empty() {
                continue;
            }
            if let Some(id) = self.shape_id(client, server_id, role, index).await? {
                placeholders.push((text, id));
            } else {
                // A layout whose placeholder name is not the one the builder
                // styles would silently lose this text, so say so.
                tracing::debug!(
                    target: "ragent::mcp",
                    slide = index,
                    role,
                    key,
                    "slide has no {key} placeholder to reuse"
                );
            }
        }
        for (text, id) in placeholders {
            self.command(
                client,
                server_id,
                "text.set",
                json!({ "id": id, "text": text }),
            )
            .await?;
        }
        Ok(())
    }

    /// Make one shape the current selection, for the format commands that follow.
    ///
    /// A `format.*` command names no shape: it restyles the selection. Without this
    /// the selection is whatever was placed or inserted last, so a body's type
    /// settings landed on the status badge and the body kept the plain paragraph
    /// formatting it arrived with.
    async fn select(&self, client: &McpClient, server_id: &str, id: u64) -> Result<(), String> {
        self.command(client, server_id, "edit.select", json!({ "ids": [id] }))
            .await
    }

    /// The id of a slide's title or body placeholder.
    ///
    /// The ids are read from the slide rather than guessed: a placeholder's id
    /// differs per slide, and styling by position (the first shape) is what let a
    /// later command act on the wrong one.
    async fn shape_id(
        &self,
        client: &McpClient,
        server_id: &str,
        role: &str,
        index: usize,
    ) -> Result<Option<u64>, String> {
        let payload = self
            .call_command(
                client,
                server_id,
                "slide.inspect",
                json!({ "index": index }),
                "slide.inspect",
            )
            .await?;
        let key = if role == "title" { "title" } else { "obj" };
        Ok(payload
            .get("shapes")
            .and_then(Value::as_array)
            .and_then(|shapes| {
                shapes.iter().find(|shape| {
                    shape
                        .get("placeholder")
                        .and_then(Value::as_str)
                        .map(|placeholder| placeholder == key)
                        .unwrap_or(false)
                })
            })
            .and_then(|shape| shape.get("id"))
            .and_then(Value::as_u64))
    }

    /// Draw the status badge in the top-right corner.
    async fn badge(
        &self,
        client: &McpClient,
        server_id: &str,
        status: ItemStatus,
    ) -> Result<(), String> {
        // A filled rounded rectangle in the status colour, with the tag reversed
        // out of it. The tag is the only uppercase token on a slide, so the
        // status is readable at a glance from across a room.
        let Some(id) = self
            .command_id(
                client,
                server_id,
                "shape.insert",
                json!({
                    "preset": "roundRect",
                    "rect": [style::BADGE_X, style::BADGE_Y, style::BADGE_W, style::BADGE_H],
                    "text": status.tag(),
                }),
            )
            .await?
        else {
            return Ok(());
        };
        self.command(
            client,
            server_id,
            "shape.fill",
            json!({ "color": style::status_colour(status.tag()), "ids": [id] }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "shape.line",
            json!({ "none": true, "ids": [id] }),
        )
        .await?;
        self.select(client, server_id, id).await?;
        self.command(
            client,
            server_id,
            "format.fontDialog",
            json!({ "bold": true, "color": "#FFFFFF", "size": 11 }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "format.align",
            json!({ "align": "center" }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "format.anchor",
            json!({ "anchor": "middle" }),
        )
        .await
    }

    /// Place the slide's table and style it as a designed table, not a grid.
    async fn table(
        &self,
        client: &McpClient,
        server_id: &str,
        rows: &[DeckTableRow],
    ) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }
        // A table sits under the lead: two columns (the item, then its value) in
        // the body column, sized to its row count.
        let height = (rows.len() as f64).mul_add(32.0, 44.0).min(style::BODY_H);
        let Some(id) = self
            .command_id(
                client,
                server_id,
                "insert.table",
                json!({
                    "rows": rows.len() + 1,
                    "cols": 2,
                    "rect": [style::MARGIN, style::BODY_Y, style::COLUMN, height],
                }),
            )
            .await?
        else {
            return Ok(());
        };
        // The stock themes draw a heavy frame; the deck's look is a header band
        // and hairlines, so the theme is cleared and the design drawn explicitly.
        self.command(
            client,
            server_id,
            "table.style",
            json!({ "style": "none", "id": id }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "table.cellFill",
            json!({
                "cells": [[0, 0], [0, 1]],
                "color": style::BAND,
                "id": id,
            }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "table.cellMargins",
            json!({ "preset": "normal", "id": id }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "table.borders",
            json!({ "which": "insideH", "color": style::RULE, "width": 0.75, "id": id }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "table.cellAlign",
            json!({ "anchor": "middle", "align": "left", "id": id }),
        )
        .await?;
        // Column widths: the label takes most of the width, the value column the
        // rest, so a count sits at the right edge where the eye expects it.
        self.command(
            client,
            server_id,
            "table.columnWidth",
            json!({ "col": 0, "width": style::COLUMN - 140.0, "id": id }),
        )
        .await?;
        self.command(
            client,
            server_id,
            "table.columnWidth",
            json!({ "col": 1, "width": 140.0, "id": id }),
        )
        .await
    }

    /// Issue one command and return the shape id it reports, when it reports one.
    ///
    /// `shape.insert` answers `{"id": N}`. The shape commands take that id as an
    /// explicit `ids`, so a shape placed here and then styled by
    /// `shape.setBounds` / `shape.fill` is styled whatever the selection has
    /// drifted to. The `format.*` commands are the exception - they name no shape
    /// and restyle only the selection - so a caller that follows with one of
    /// those goes through [`Self::select`] first.
    async fn command_id(
        &self,
        client: &McpClient,
        server_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<Option<u64>, String> {
        let result = self
            .call_command(client, server_id, command, params, command)
            .await?;
        Ok(result.get("id").and_then(Value::as_u64))
    }

    /// Issue one command on the deck server's command channel.
    async fn command(
        &self,
        client: &McpClient,
        server_id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<(), String> {
        self.call_command(client, server_id, command, params, command)
            .await
            .map(|_| ())
    }

    /// Call one command and return the payload the server answered with.
    ///
    /// `what` names the thing being called in an error ("`new_presentation`
    /// failed"), because the command id is not always the tool name: a caller
    /// driving a wrapper tool passes the tool's own name.
    async fn call_command(
        &self,
        client: &McpClient,
        server_id: &str,
        command: &str,
        params: serde_json::Value,
        what: &str,
    ) -> Result<Value, String> {
        let result = client
            .call_tool(
                server_id,
                self.tool,
                json!({ "command": command, "params": params }),
            )
            .await
            .map_err(|e| format!("`{what}` failed: {e}"))?;
        if result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            // A styling command is an enhancement: a server that does not know it
            // still produces a readable slide, so the caller logs it and the deck
            // keeps the plain title-and-body shape.
            return Err(format!("`{what}` rejected: {}", result_text(&result)));
        }
        // The payload is the first text block, JSON-decoded when it parses.
        let payload = result
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| blocks.first())
            .and_then(|block| block.get("text"))
            .and_then(Value::as_str)
            .map(|text| serde_json::from_str(text).unwrap_or(Value::Null))
            .unwrap_or(Value::Null);
        Ok(payload)
    }
}

/// The text a tool result carries, for an error message.
///
/// [`McpClient::call_tool`] reports content as `{"content": [{"type": "text",
/// "text": "..."}, ...]}`; joining the text blocks is what makes the server's
/// own wording ("unknown command `presentation.new`") reach the user verbatim
/// instead of being replaced by a guessed cause.
fn result_text(result: &Value) -> String {
    let text: Vec<&str> = result
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if text.is_empty() {
        result.to_string()
    } else {
        text.join("; ")
    }
}

/// The FR-031 progress report for one planned slide.
///
/// The detail names the slide's kind and title, which is what identifies it to a
/// reader watching the export: "slide 3 of 12 (finding) - Async cancellation
/// leaks memory". A title is capped so a very long headline cannot flood the
/// message window, and `completed` is the count of slides finished *before* this
/// one, because the report is emitted before the slide is drawn.
fn deck_slide_event(slide: &DeckSlide, index: usize, total: usize) -> DeckEvent {
    let kind = match slide.kind {
        SlideKind::Cover => "cover",
        SlideKind::Section => "section",
        SlideKind::Finding => "finding",
        SlideKind::Table => "table",
        SlideKind::Summary => "summary",
    };
    let title = slide.title.trim();
    let title = if title.chars().count() > 48 {
        let truncated: String = title.chars().take(48).collect();
        format!("{truncated}...")
    } else {
        title.to_string()
    };
    DeckEvent::progress(
        DeckStage::Export,
        index,
        total,
        format!("slide {} of {total} ({kind}) - {title}", index + 1),
    )
}

#[async_trait]
impl DeckSlideSink for McpDeckSink {
    async fn write_deck(&self, plan: &DeckSlidePlan) -> DeckPptxOutcome {
        self.write_deck_with_progress(plan, None).await
    }

    /// Write the presentation, reporting each slide as the server accepts it
    /// (spec `diffdeck` FR-031).
    ///
    /// Every slide is one or more round trips over the MCP channel, so the export
    /// is the longest silent stretch of a deck run. Each slide is reported before
    /// it is drawn - the position the server is about to work on - and the count
    /// is the plan's own slide count, so the surface shows `N of M` against the
    /// deck that will actually be on disk.
    async fn write_deck_with_progress(
        &self,
        plan: &DeckSlidePlan,
        progress: Option<&dyn DeckProgress>,
    ) -> DeckPptxOutcome {
        let Some(tool) = self.available_tool().await else {
            return DeckPptxOutcome::Unavailable;
        };

        // An existing presentation is replaced so a rerun cannot silently keep
        // an older deck beside a newer markdown one.
        if plan.path.exists() && !plan.force {
            return DeckPptxOutcome::Failed {
                reason: format!("{} already exists", plan.path.display()),
            };
        }
        if let Some(parent) = plan.path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return DeckPptxOutcome::Failed {
                    reason: format!("cannot create {}: {e}", parent.display()),
                };
            }
        }

        let written = match (&tool.command_tool, &tool.add_slide_tool, &tool.save_tool) {
            (Some(command_tool), _, _) => {
                self.write_via_command(&tool, plan, command_tool, progress)
                    .await
            }
            (None, Some(add_slide_tool), Some(save_tool)) => {
                self.write_via_tools(&tool, plan, add_slide_tool, save_tool, progress)
                    .await
            }
            // `select_deck_tool` guarantees one of the two shapes.
            (None, _, _) => {
                return DeckPptxOutcome::Failed {
                    reason: format!(
                        "server '{}' has no way to add slides and save a file",
                        tool.server_id
                    ),
                };
            }
        };

        if let Err(reason) = written {
            return DeckPptxOutcome::Failed { reason };
        }

        // The server wrote it; confirm the file is actually there so a silent
        // protocol mismatch is reported rather than claimed as success.
        if !plan.path.exists() {
            return DeckPptxOutcome::Failed {
                reason: format!(
                    "server '{}' reported success but {} was not created",
                    tool.server_id,
                    plan.path.display()
                ),
            };
        }

        DeckPptxOutcome::Written {
            path: plan.path.clone(),
            slides: plan.slide_count(),
            provider: tool.provider(),
        }
    }
}
