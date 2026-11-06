//! Drive-protocol regression tests for the deck's PowerPoint export (spec
//! `diffdeck` FR-030).
//!
//! [`McpDeckSink`] drives a *remote* presentation server, so the protocol it
//! speaks - which tool creates the file, whether the default slide is reused or
//! appended to, which slide index every later command targets - is invisible to
//! the compiler and is only proven by watching what the server is asked to do.
//! A wrong index here is not a crash: it silently writes the plan's slides beside
//! an empty one, which is how a 20-slide file with a blank lead slide came out of
//! a 19-slide plan.
//!
//! These cases run the sink against an in-process fake of a deck server over the
//! HTTP MCP transport, record every command in arrival order, and assert the
//! sequence. The fake answers from its own slide list, so a command naming an
//! index it does not have is a test failure rather than a silently ignored write.
//!
//! [`McpDeckSink`]: ragent_agent::mcp::deck_sink::McpDeckSink

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use ragent_agent::mcp::deck_sink::McpDeckSink;
use ragent_agent::mcp::{McpClient, McpStatus};
use ragent_config::{McpServerConfig, McpTransport};
use ragent_research::deck::ItemStatus;
use ragent_research::deck_pptx::{
    DeckPptxOutcome, DeckSlide, DeckSlidePlan, DeckSlideSink, SlideKind,
};
use serde_json::{Value, json};
use tokio::sync::RwLock;

/// One command the sink sent to the fake server.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sent {
    /// The MCP tool that carried it (`new_presentation`, `run_command`, ...).
    tool: String,
    /// The command id, for a `run_command` call.
    command: Option<String>,
    /// The command's parameters.
    params: Value,
}

/// The state one fake server run records.
#[derive(Debug, Clone, Default)]
struct Recorded {
    /// Every command, in arrival order.
    sent: Vec<Sent>,
    /// How many slides the presentation holds.
    slides: usize,
}

/// A fake deck server: a presentation with one default slide, a command channel,
/// and the three wrapper tools the sink prefers.
///
/// It answers from its own state rather than echoing, so an out-of-range index is
/// refused and the sink's error path is exercised instead of hidden.
async fn handler(State(state): State<Arc<Mutex<Recorded>>>, body: String) -> impl IntoResponse {
    let request: Value = serde_json::from_str(&body).unwrap_or_default();
    let id = request.get("id").cloned().unwrap_or(json!(1));
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let result = match method {
        "initialize" => json!({ "protocolVersion": "2024-11-05" }),
        "tools/list" => json!({ "tools": tool_manifest() }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_default();
            let tool = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            call_tool(&state, &tool, arguments)
        }
        other => panic!("unexpected MCP method {other}"),
    };

    let frame = json!({ "jsonrpc": "2.0", "id": id, "result": result });
    (
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        format!("event: message\ndata: {frame}\n\n"),
    )
}

/// The tool manifest a DeckCraft-shaped server advertises: the three wrappers the
/// sink prefers and the generic command channel it styles through.
fn tool_manifest() -> Vec<Value> {
    vec![
        json!({ "name": "new_presentation", "description": "Start a new presentation", "inputSchema": { "type": "object" } }),
        json!({ "name": "add_slide", "description": "Insert a slide", "inputSchema": { "type": "object" } }),
        json!({ "name": "save_presentation", "description": "Save the presentation to a path", "inputSchema": { "type": "object" } }),
        json!({ "name": "run_command", "description": "Run any command by id with JSON parameters", "inputSchema": { "type": "object" } }),
    ]
}

/// Answer one tool call from the fake's slide list, recording it.
fn call_tool(state: &Arc<Mutex<Recorded>>, tool: &str, arguments: Value) -> Value {
    let mut recorded = state.lock().expect("the fake's state is not poisoned");
    let payload = match tool {
        "new_presentation" => {
            // A new presentation arrives with one blank title slide, which is the
            // behaviour the sink's "reuse the default slide" branch exists for.
            recorded.slides = 1;
            json!({ "document": 1, "slides": recorded.slides })
        }
        "add_slide" => {
            recorded.slides += 1;
            recorded.sent.push(Sent {
                tool: tool.to_string(),
                command: None,
                params: arguments,
            });
            json!({ "index": recorded.slides - 1 })
        }
        "save_presentation" => {
            // The sink confirms the file exists after the save, so the fake writes
            // it: a save that only answered would fail that check, not the protocol.
            if let Some(path) = arguments.get("path").and_then(Value::as_str) {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    std::fs::create_dir_all(parent).expect("create the save directory");
                }
                std::fs::write(path, b"fake presentation").expect("write the saved file");
            }
            recorded.sent.push(Sent {
                tool: tool.to_string(),
                command: None,
                params: arguments,
            });
            json!({ "saved": true })
        }
        "run_command" => {
            let command = arguments
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let params = arguments
                .get("params")
                .cloned()
                .unwrap_or_else(|| json!({}));
            // A command that names a slide must name one that exists; the sink
            // treats a rejection as a failed export, so this turns a wrong index
            // into a named failure instead of a slide nobody wrote to.
            if let Some(index) = params.get("index").and_then(Value::as_u64) {
                assert!(
                    (index as usize) < recorded.slides.max(1),
                    "`{command}` targeted slide {index}, but the presentation has {} slide(s)",
                    recorded.slides
                );
            }
            recorded.sent.push(Sent {
                tool: tool.to_string(),
                command: Some(command.clone()),
                params: params.clone(),
            });
            match command.as_str() {
                "slide.layout" => json!({ "layout": 18 }),
                // The placeholders a re-laid-out `titleAndContent` slide carries.
                "slide.inspect" => json!({
                    "shapes": [
                        { "id": 101, "placeholder": "title" },
                        { "id": 102, "placeholder": "obj" }
                    ]
                }),
                _ => Value::Null,
            }
        }
        other => panic!("unexpected tool {other}"),
    };
    json!({
        "content": [{ "type": "text", "text": payload.to_string() }],
        "isError": false
    })
}

/// Boot the fake deck server and return its address.
async fn spawn_server() -> (SocketAddr, Arc<Mutex<Recorded>>) {
    let state = Arc::new(Mutex::new(Recorded::default()));
    let router = Router::new()
        .route("/mcp", post(handler))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind the fake deck server");
    let addr = listener.local_addr().expect("the fake's address");
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    (addr, state)
}

/// A three-slide plan: a cover, a section, and a summary.
///
/// The cover is a [`SlideKind::Cover`] because that is what triggers the
/// default-slide reuse; every other kind is appended.
fn plan(path: PathBuf) -> DeckSlidePlan {
    DeckSlidePlan {
        item: "aisurvey".to_string(),
        title: Some("A deck title".to_string()),
        from: "2026-11-02".to_string(),
        to: "2026-11-03".to_string(),
        generated_on: "2026-11-04".to_string(),
        elements: Vec::new(),
        slides: vec![
            DeckSlide::new(SlideKind::Cover, "Cover title")
                .with_lead("33 items across 3 elements")
                .with_bullets(vec!["31 changes".to_string()]),
            DeckSlide::new(SlideKind::Section, "Section title")
                .with_status(ItemStatus::Added)
                .with_lead("A definition"),
            DeckSlide::new(SlideKind::Summary, "What changed")
                .with_lead("33 items compared")
                .with_bullets(vec!["NEW - 2".to_string()]),
        ],
        path,
        force: true,
    }
}

/// A sink connected to the fake, and the fake's own record.
async fn connected_sink() -> (McpDeckSink, Arc<Mutex<Recorded>>, PathBuf) {
    let (addr, state) = spawn_server().await;
    let config = McpServerConfig {
        type_: McpTransport::Http,
        url: Some(format!("http://{addr}/mcp")),
        ..McpServerConfig::default()
    };
    let mut client = McpClient::new();
    client
        .connect("deckcraft", config)
        .await
        .expect("the fake deck server connects");
    let server = client
        .servers()
        .iter()
        .find(|server| server.id == "deckcraft")
        .expect("the fake is registered");
    assert_eq!(server.status, McpStatus::Connected);
    let target = PathBuf::from("target/temp/test_deck_sink_drive_protocol.pptx");
    (
        McpDeckSink::new(Arc::new(RwLock::new(client))),
        state,
        target,
    )
}

/// The plan's first slide reuses the presentation's default slide at index 0.
///
/// A fresh presentation already holds one blank slide. Appending the cover beside
/// it leaves an empty slide at the front and makes the file one longer than the
/// plan - the defect this case pins. The default slide is re-laid-out to the
/// planned layout and then written, rather than drawn beside.
#[tokio::test]
async fn test_the_cover_reuses_the_default_slide_at_index_zero() {
    let (sink, state, target) = connected_sink().await;
    let plan = plan(target);

    let outcome = sink.write_deck(&plan).await;
    assert!(
        matches!(outcome, DeckPptxOutcome::Written { .. }),
        "the fake answers every command, so the export must succeed: {outcome:?}"
    );

    let recorded = state.lock().expect("state").clone();
    assert_eq!(
        recorded.slides, 3,
        "the plan's 3 slides must be 3 slides: the cover reuses the default slide \
         instead of being appended beside it"
    );

    // The first slide command must be the re-layout of the reused slide.
    let first = recorded
        .sent
        .first()
        .expect("the sink sends at least one command");
    assert_eq!(first.tool, "run_command");
    assert_eq!(first.command.as_deref(), Some("slide.layout"));
    assert_eq!(
        first.params.get("index").and_then(Value::as_u64),
        Some(0),
        "the default slide is index 0, not the 1-based number `slide.new` answers with"
    );
}

/// Every slide command targets its 0-based plan index.
///
/// `slide.go` and `slide.inspect` share one numbering with the plan, so slide `n`
/// of the plan must be index `n` of the presentation even though the reused cover
/// was never created by `slide.new`.
#[tokio::test]
async fn test_every_slide_command_targets_its_zero_based_plan_index() {
    let (sink, state, target) = connected_sink().await;
    let plan = plan(target);
    let expected: Vec<u64> = (0..plan.slide_count() as u64).collect();

    let outcome = sink.write_deck(&plan).await;
    assert!(
        matches!(outcome, DeckPptxOutcome::Written { .. }),
        "the export must succeed: {outcome:?}"
    );

    let recorded = state.lock().expect("state").clone();
    let visited: Vec<u64> = recorded
        .sent
        .iter()
        .filter(|sent| sent.command.as_deref() == Some("slide.go"))
        .filter_map(|sent| sent.params.get("index").and_then(Value::as_u64))
        .collect();
    assert_eq!(
        visited, expected,
        "slides must be visited once each, in plan order, by 0-based index"
    );

    // Dressing a slide draws on it, and a draw acts on the *current* slide, so the
    // navigation that makes slide `n` current must come before the first shape
    // drawn on it. Both indices are the plan's own, so this is also the assertion
    // that the two numberings agree.
    for index in expected {
        let first = |command: &str| {
            recorded
                .sent
                .iter()
                .position(|sent| {
                    sent.command.as_deref() == Some(command)
                        && sent
                            .params
                            .get("index")
                            .and_then(Value::as_u64)
                            .map(|target| target == index)
                            .unwrap_or(false)
                })
                .unwrap_or_else(|| panic!("`{command}` must target slide {index}"))
        };
        let navigated = first("slide.go");
        let drawn = recorded
            .sent
            .iter()
            .skip(navigated)
            .position(|sent| sent.command.as_deref() == Some("shape.insert"))
            .map(|offset| navigated + offset)
            .unwrap_or_else(|| panic!("slide {index} must be dressed after being navigated to"));
        assert!(
            navigated < drawn,
            "slide {index} must be navigated to before anything is drawn on it"
        );
    }
}

/// The appended slides carry the plan's layout, title, and body.
///
/// Only the reused cover goes through `text.set`; every other slide is created by
/// the `add_slide` wrapper with its text in the call, so a wrapper that lost the
/// body would produce a titled but blank slide.
#[tokio::test]
async fn test_appended_slides_carry_the_planned_title_and_body() {
    let (sink, state, target) = connected_sink().await;
    let plan = plan(target);

    let outcome = sink.write_deck(&plan).await;
    assert!(
        matches!(outcome, DeckPptxOutcome::Written { .. }),
        "the export must succeed: {outcome:?}"
    );

    let recorded = state.lock().expect("state").clone();
    let added: Vec<&Sent> = recorded
        .sent
        .iter()
        .filter(|sent| sent.tool == "add_slide")
        .collect();
    assert_eq!(
        added.len(),
        plan.slide_count() - 1,
        "every slide after the reused cover is appended"
    );
    for (sent, slide) in added.iter().zip(plan.slides.iter().skip(1)) {
        assert_eq!(
            sent.params.get("layout").and_then(Value::as_str),
            Some("titleAndContent"),
            "`{}` must use the layout the builder can style",
            slide.title
        );
        assert_eq!(
            sent.params.get("title").and_then(Value::as_str),
            Some(slide.title.as_str()),
            "the appended slide carries the plan's title"
        );
        let body = sent
            .params
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !body.trim().is_empty(),
            "`{}` must carry a body, not just a title",
            slide.title
        );
    }
}

/// The save names the plan's own path, and it comes last.
///
/// A save before the slides are built writes a partial file, and a save to any
/// other path leaves the caller's expected artifact missing.
#[tokio::test]
async fn test_the_save_targets_the_plan_path_and_comes_last() {
    let (sink, state, target) = connected_sink().await;
    let plan = plan(target);
    let expected_path = plan.path.display().to_string();

    let outcome = sink.write_deck(&plan).await;
    assert!(
        matches!(outcome, DeckPptxOutcome::Written { .. }),
        "the export must succeed: {outcome:?}"
    );

    let recorded = state.lock().expect("state").clone();
    let last = recorded.sent.last().expect("at least one command");
    assert_eq!(
        last.tool, "save_presentation",
        "the save must come after every slide command"
    );
    assert_eq!(
        last.params.get("path").and_then(Value::as_str),
        Some(expected_path.as_str()),
        "the save must name the plan's path"
    );
}

/// A sink with no connected deck server reports `Unavailable`, never a file.
///
/// The export is additive, so an absent server costs the user nothing - but
/// reporting it as written would claim a deck that is not there.
#[tokio::test]
async fn test_no_connected_deck_server_reports_unavailable() {
    // A sink with no connected server cannot select a deck tool at all, which is
    // the `Unavailable` contract rather than a failure.
    let sink = McpDeckSink::new(Arc::new(RwLock::new(McpClient::new())));
    let outcome = sink
        .write_deck(&plan(PathBuf::from("target/temp/unused.pptx")))
        .await;
    assert_eq!(
        outcome,
        DeckPptxOutcome::Unavailable,
        "no connected deck server is `Unavailable`, not a failure"
    );
}

/// Every planned slide is reported before it is drawn (spec `diffdeck` FR-031).
///
/// The export is one server round trip per slide, so it is the longest silent
/// stretch of a deck run. The report must arrive *before* the slide is drawn,
/// carrying the plan's own 1-based position and total, so a surface can render
/// "slide 3 of 12" while the server is working on slide 3 - a report emitted
/// afterwards would only ever name a slide that is already done.
#[tokio::test]
async fn test_every_slide_is_reported_before_it_is_drawn() {
    use ragent_research::deck_progress::{DeckEvent, DeckProgress, DeckStage};

    /// Records each report beside the command count at the moment it arrived.
    struct Recorder {
        events: Mutex<Vec<(DeckEvent, usize)>>,
        recorded: Arc<Mutex<Recorded>>,
    }
    impl DeckProgress for Recorder {
        fn report(&self, event: &DeckEvent) {
            let commands = self
                .recorded
                .lock()
                .map(|state| state.sent.len())
                .unwrap_or_default();
            if let Ok(mut events) = self.events.lock() {
                events.push((event.clone(), commands));
            }
        }
    }

    let (sink, state, target) = connected_sink().await;
    let plan = plan(target);
    let planned = plan.slides.len();
    let recorder = Recorder {
        events: Mutex::new(Vec::new()),
        recorded: state.clone(),
    };

    let outcome = sink.write_deck_with_progress(&plan, Some(&recorder)).await;
    assert!(
        matches!(outcome, DeckPptxOutcome::Written { .. }),
        "the fake answers every command: {outcome:?}"
    );

    let events = recorder.events.lock().expect("events").clone();
    let slide_events: Vec<&DeckEvent> = events
        .iter()
        .filter(|(event, _)| event.stage == DeckStage::Export)
        .map(|(event, _)| event)
        .collect();
    assert_eq!(
        slide_events.len(),
        planned,
        "one report per planned slide, no more and no fewer"
    );
    for (index, event) in slide_events.iter().enumerate() {
        assert_eq!(
            event.unit(),
            Some((index, planned)),
            "report {index} must carry the completed/total pair for its position"
        );
        assert!(
            event
                .detail
                .contains(&format!("slide {} of {planned}", index + 1)),
            "the report names its slide: {}",
            event.detail
        );
    }

    // The last report arrives before the save, so the final slide is visible
    // while it is drawn rather than only once the file is on disk.
    let save_position = state
        .lock()
        .expect("state")
        .sent
        .iter()
        .position(|sent| sent.tool == "save_presentation")
        .expect("the sink saves the file");
    let last_report = events
        .iter()
        .filter(|(event, _)| event.stage == DeckStage::Export)
        .map(|(_, commands)| *commands)
        .next_back()
        .expect("the export reported");
    assert!(
        last_report <= save_position,
        "the last slide report ({last_report}) must precede the save ({save_position})"
    );
}
