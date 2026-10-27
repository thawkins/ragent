//! Additional coverage for `ragent_tui::input::handle_key` modal/overlay and
//! editing branches not exercised by the other TUI suites.
//!
//! `test_input_field_extra.rs` covers `InputField` (a different module) and the
//! `is_global_quit_key` predicate. This file drives the real `handle_key`
//! dispatch through the dialog/overlay intercepts (question, permission,
//! pickers, menus, viewers, confirmations) and the remaining normal-edit key
//! matrix, asserting on the returned `InputAction` and the resulting state.
//!
//! Tests only - no production logic is touched.

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use ragent_agent::permission::PermissionRequest;
use ragent_tui::app::{
    App, BackendPanelRow, BackendPanelState, ContextAction, ContextMenuState, FileMenuEntry,
    FileMenuState, MemoryViewState, OutputViewLineCache, OutputViewState, OutputViewTarget,
    PendingForceCleanup, PendingMemoryDelete, PlanApprovalState, QuestionRequest, QueuedInput,
    ResearchViewState, RollbackOfferState, SelectionPane, SlashMenuEntry, SlashMenuState,
};
use ragent_tui::input::{InputAction, handle_key};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn kmod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn slash_entry(trigger: &str) -> SlashMenuEntry {
    SlashMenuEntry {
        trigger: trigger.to_string(),
        description: format!("{trigger} description"),
        is_skill: false,
        suggestions: Vec::new(),
        parameter_hint: None,
    }
}

fn queued(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

// ===========================================================================
// Global quit keys / event kinds
// ===========================================================================

#[tokio::test]
async fn release_event_is_ignored() {
    let mut app = support::make_app();
    let mut k = key(KeyCode::Enter);
    k.kind = KeyEventKind::Release;
    assert!(handle_key(&mut app, k).await.is_none());
}

#[tokio::test]
async fn ctrl_c_arms_quit_and_copies_with_selection() {
    let mut app = support::make_app();
    let action = handle_key(&mut app, kmod(KeyCode::Char('c'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::Quit)));

    app.kb_select_anchor = Some(0);
    let action = handle_key(&mut app, kmod(KeyCode::Char('c'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::CopyToClipboard)));
}

#[tokio::test]
async fn ctrl_d_confirms_quit() {
    let mut app = support::make_app();
    let action = handle_key(&mut app, kmod(KeyCode::Char('d'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::ConfirmQuit)));
}

// ===========================================================================
// Confirmation modals
// ===========================================================================

#[tokio::test]
async fn memory_delete_modal_enter_and_esc() {
    let mut app = support::make_app();
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id: 1,
        preview: "x".to_string(),
    });
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ConfirmMemoryDelete)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelMemoryDelete)));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('q')))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn router_save_modal_enter_and_esc() {
    let mut app = support::make_app();
    app.pending_router_save = Some(ragent_llm::providers::router_config::RouterConfig::default());
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ConfirmRouterSave)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelRouterSave)));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn stop_confirm_modal_enter_and_esc() {
    let mut app = support::make_app();
    app.pending_stop_confirm = true;
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ConfirmStopAgent)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelStopAgent)));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn force_cleanup_modal_enter_and_esc() {
    let mut app = support::make_app();
    app.pending_forcecleanup = Some(PendingForceCleanup {
        team_name: "alpha".to_string(),
        active_members: vec!["alice".to_string()],
    });
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ConfirmForceCleanup)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelForceCleanup)));
}

#[tokio::test]
async fn rollback_offer_modal_enter_and_esc() {
    let mut app = support::make_app();
    app.pending_rollback = Some(RollbackOfferState {
        session_id: "s1".to_string(),
        status: "completed".to_string(),
        iterations: 1,
        diffstat: "+1 -0".to_string(),
        files: vec!["a.rs".to_string()],
    });
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ConfirmRollback)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelRollback)));
}

#[tokio::test]
async fn queue_clear_confirm_modal() {
    let mut app = support::make_app();
    app.queue_clear_confirm_open = true;
    // Left/Right toggle the selection and request a redraw.
    let action = handle_key(&mut app, key(KeyCode::Left)).await;
    assert!(action.is_none());
    assert!(app.needs_redraw);
    // Enter resolves to confirm or cancel depending on the selection.
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(
        matches!(
            action,
            Some(InputAction::ConfirmQueueClear | InputAction::CancelQueueClear)
        ),
        "got: {action:?}"
    );
}

#[tokio::test]
async fn memory_clear_confirm_modal() {
    let mut app = support::make_app();
    app.memory_clear_confirm_open = true;
    let action = handle_key(&mut app, key(KeyCode::Right)).await;
    assert!(action.is_none());
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(
        matches!(
            action,
            Some(InputAction::ConfirmMemoryClear | InputAction::CancelMemoryClear)
        ),
        "got: {action:?}"
    );
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::CancelMemoryClear)));
}

// ===========================================================================
// Plan approval
// ===========================================================================

#[tokio::test]
async fn plan_approval_cursor_and_actions() {
    let mut app = support::make_app();
    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "the plan".to_string(),
        cursor_approve: true,
    });
    let action = handle_key(&mut app, key(KeyCode::Right)).await;
    assert!(matches!(action, Some(InputAction::TogglePlanCursor)));
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::ApprovePlan)));

    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "the plan".to_string(),
        cursor_approve: false,
    });
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::RejectPlan)));
    let action = handle_key(&mut app, key(KeyCode::Char('r'))).await;
    assert!(matches!(action, Some(InputAction::RejectPlan)));
    let action = handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(matches!(action, Some(InputAction::RejectPlan)));
}

// ===========================================================================
// Question dialog
// ===========================================================================

#[tokio::test]
async fn question_dialog_with_options() {
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "Pick".to_string(),
        options: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        default_index: Some(0),
        expire_at_ms: None,
    });
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.question_selected_index, 1);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('j')))
            .await
            .is_none()
    );
    assert_eq!(app.question_selected_index, 2);
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.question_selected_index, 1);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('k')))
            .await
            .is_none()
    );
    assert_eq!(app.question_selected_index, 0);
    // Clamp at the top.
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.question_selected_index, 0);

    // Enter answers the highlighted option and pops the queue.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.question_queue.is_empty());
    assert_eq!(app.question_selected_index, 0);
}

#[tokio::test]
async fn question_dialog_with_options_esc_dismisses() {
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q1".to_string(),
        session_id: "s1".to_string(),
        question: "Pick".to_string(),
        options: vec!["a".to_string()],
        default_index: None,
        expire_at_ms: None,
    });
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.question_queue.is_empty());
}

#[tokio::test]
async fn question_dialog_free_text_edit_and_submit() {
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q2".to_string(),
        session_id: "s1".to_string(),
        question: "Type".to_string(),
        options: Vec::new(),
        default_index: None,
        expire_at_ms: None,
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('h')))
            .await
            .is_none()
    );
    assert!(
        handle_key(&mut app, key(KeyCode::Char('i')))
            .await
            .is_none()
    );
    assert_eq!(app.pending_question_input, "hi");
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    assert_eq!(app.pending_question_input, "h");
    // Whitespace-only input does not submit; the dialog stays open.
    app.pending_question_input = "   ".to_string();
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert_eq!(app.question_queue.len(), 1);
    // A real answer submits and clears the input.
    app.pending_question_input = "yes".to_string();
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.question_queue.is_empty());
    assert_eq!(app.pending_question_input, "");
}

#[tokio::test]
async fn question_dialog_free_text_esc_dismisses() {
    let mut app = support::make_app();
    app.question_queue.push_back(QuestionRequest {
        id: "q3".to_string(),
        session_id: "s1".to_string(),
        question: "Type".to_string(),
        options: Vec::new(),
        default_index: None,
        expire_at_ms: None,
    });
    app.pending_question_input = "typed".to_string();
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.question_queue.is_empty());
    assert_eq!(app.pending_question_input, "");
}

// ===========================================================================
// Permission dialog
// ===========================================================================

fn permission_req() -> PermissionRequest {
    PermissionRequest {
        id: "p1".to_string(),
        session_id: "s1".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({"timeout_secs": 120u64}),
        tool_call_id: None,
    }
}

#[tokio::test]
async fn permission_dialog_y_a_n() {
    for c in ['y', 'a', 'n'] {
        let mut app = support::make_app();
        app.permission_queue.push_back(permission_req());
        assert!(handle_key(&mut app, key(KeyCode::Char(c))).await.is_none());
        assert_eq!(app.permission_queue.len(), 1);
    }
    // Any other key is swallowed and changes nothing.
    let mut app = support::make_app();
    app.permission_queue.push_back(permission_req());
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
}

// ===========================================================================
// Menus / pickers / viewers
// ===========================================================================

#[tokio::test]
async fn shortcuts_panel_esc_dismisses() {
    let mut app = support::make_app();
    app.show_shortcuts = true;
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(!app.show_shortcuts);

    app.show_shortcuts = true;
    assert!(
        handle_key(&mut app, key(KeyCode::Char('?')))
            .await
            .is_none()
    );
    assert!(!app.show_shortcuts);

    app.show_shortcuts = true;
    assert!(
        handle_key(&mut app, key(KeyCode::Char('x')))
            .await
            .is_none()
    );
    assert!(app.show_shortcuts, "other keys keep the panel open");
}

#[tokio::test]
async fn context_menu_navigation_and_actions() {
    let mut app = support::make_app();
    app.context_menu = Some(ContextMenuState {
        x: 0,
        y: 0,
        pane: SelectionPane::Messages,
        selected: 1,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, false),
            (ContextAction::Paste, true),
        ],
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(
        app.context_menu.as_ref().expect("open").selected,
        2,
        "Up skips the disabled items and wraps to Paste"
    );
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.context_menu.as_ref().expect("open").selected, 2);
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.context_menu.is_none(), "Enter activates and closes");

    app.context_menu = Some(ContextMenuState {
        x: 0,
        y: 0,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Cut, false)],
    });
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.context_menu.is_none(), "disabled item closes the menu");

    let mut app = support::make_app();
    app.context_menu = Some(ContextMenuState {
        x: 0,
        y: 0,
        pane: SelectionPane::Messages,
        selected: 0,
        items: vec![(ContextAction::Copy, true)],
    });
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.context_menu.is_none());
}

#[tokio::test]
async fn slash_menu_navigation_select_and_escape() {
    let mut app = support::make_app();
    app.input = "/".to_string();
    app.slash_menu = Some(SlashMenuState {
        matches: vec![slash_entry("help"), slash_entry("history")],
        selected: 0,
        filter: String::new(),
    });
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.slash_menu.as_ref().expect("open").selected, 1);
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.slash_menu.as_ref().expect("open").selected, 0);
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.slash_menu.as_ref().expect("open").selected, 1);

    // Enter emits the highlighted command while the menu is open.
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    match action {
        Some(InputAction::SlashCommand(cmd)) => assert!(cmd.starts_with('/')),
        other => panic!("expected SlashCommand, got {other:?}"),
    }

    // Typing routes into the input buffer through the menu branch.
    let mut app = support::make_app();
    app.input = "/".to_string();
    app.input_cursor = 1;
    app.slash_menu = Some(SlashMenuState {
        matches: vec![slash_entry("help")],
        selected: 0,
        filter: String::new(),
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('x')))
            .await
            .is_none()
    );
    assert!(app.input.contains('x'));

    // Esc closes the menu.
    let mut app = support::make_app();
    app.slash_menu = Some(SlashMenuState {
        matches: vec![slash_entry("help")],
        selected: 0,
        filter: String::new(),
    });
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.slash_menu.is_none());
}

#[tokio::test]
async fn file_menu_navigation_and_editing_keys() {
    let mut app = support::make_app();
    app.input = "@/x".to_string();
    app.file_menu = Some(FileMenuState {
        matches: (0..12)
            .map(|i| FileMenuEntry {
                display: format!("f{i}"),
                path: std::path::PathBuf::from(format!("f{i}")),
                is_dir: false,
            })
            .collect(),
        selected: 0,
        scroll_offset: 0,
        query: "x".to_string(),
        current_dir: None,
    });
    // Down past the visible window scrolls the menu.
    for _ in 0..10 {
        assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    }
    assert!(app.file_menu.as_ref().expect("open").scroll_offset > 0);
    // Up back to the top resets the offset.
    for _ in 0..10 {
        assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    }
    assert_eq!(app.file_menu.as_ref().expect("open").selected, 0);
    // Navigation/edit keys are routed through the menu.
    for k in [
        kmod(KeyCode::Char('a'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('e'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('b'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('f'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('w'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('k'), KeyModifiers::CONTROL),
        kmod(KeyCode::Char('u'), KeyModifiers::CONTROL),
        kmod(KeyCode::Left, KeyModifiers::CONTROL),
        kmod(KeyCode::Right, KeyModifiers::CONTROL),
        kmod(KeyCode::Home, KeyModifiers::CONTROL),
        kmod(KeyCode::End, KeyModifiers::CONTROL),
        key(KeyCode::Left),
        key(KeyCode::Right),
        key(KeyCode::Home),
        key(KeyCode::End),
        key(KeyCode::Delete),
        key(KeyCode::Char('z')),
        key(KeyCode::Backspace),
        kmod(KeyCode::Char('\\'), KeyModifiers::CONTROL),
    ] {
        let _ = handle_key(&mut app, k).await;
    }
    // Esc closes the menu.
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.file_menu.is_none());
}

#[tokio::test]
async fn research_view_scroll_and_close() {
    let mut app = support::make_app();
    app.research_view = Some(ResearchViewState {
        name: "t".to_string(),
        path: std::path::PathBuf::from("/r/RESEARCH.md"),
        base_dir: std::path::PathBuf::from("/r"),
        markdown: "# T".to_string(),
        scroll_offset: 0,
        max_scroll: 100,
        line_cache: OutputViewLineCache::default(),
    });
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageUp, KeyModifiers::CONTROL)).await,
        Some(InputAction::ResearchViewToStart)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageDown, KeyModifiers::CONTROL)).await,
        Some(InputAction::ResearchViewToEnd)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageUp)).await,
        Some(InputAction::ResearchViewPageUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageDown)).await,
        Some(InputAction::ResearchViewPageDown)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Up)).await,
        Some(InputAction::ResearchViewLineUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Down)).await,
        Some(InputAction::ResearchViewLineDown)
    ));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.research_view.is_none());
}

#[tokio::test]
async fn memory_view_scroll_and_close() {
    let row = ragent_storage::storage::MemoryRow {
        id: 1,
        content: "c".to_string(),
        category: "fact".to_string(),
        source: "s".to_string(),
        confidence: 0.9,
        project: "p".to_string(),
        session_id: "s".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        access_count: 0,
        last_accessed: None,
    };
    let mut app = support::make_app();
    app.memory_view = Some(MemoryViewState {
        row,
        scroll_offset: 0,
        max_scroll: 50,
        line_cache: OutputViewLineCache::default(),
    });
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageUp, KeyModifiers::CONTROL)).await,
        Some(InputAction::MemoryViewToStart)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageDown, KeyModifiers::CONTROL)).await,
        Some(InputAction::MemoryViewToEnd)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageUp)).await,
        Some(InputAction::MemoryViewPageUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageDown)).await,
        Some(InputAction::MemoryViewPageDown)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Up)).await,
        Some(InputAction::MemoryViewLineUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Down)).await,
        Some(InputAction::MemoryViewLineDown)
    ));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.memory_view.is_none());
}

#[tokio::test]
async fn output_view_scroll_and_close() {
    let mut app = support::make_app();
    app.output_view = Some(OutputViewState {
        target: OutputViewTarget::Session {
            session_id: "s1".to_string(),
            label: "primary".to_string(),
        },
        scroll_offset: 0,
        max_scroll: 20,
        line_cache: OutputViewLineCache::default(),
    });
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageUp, KeyModifiers::CONTROL)).await,
        Some(InputAction::OutputViewToStart)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageDown, KeyModifiers::CONTROL)).await,
        Some(InputAction::OutputViewToEnd)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageUp)).await,
        Some(InputAction::OutputViewPageUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageDown)).await,
        Some(InputAction::OutputViewPageDown)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Down, KeyModifiers::ALT)).await,
        Some(InputAction::FocusNextTeammate)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Up, KeyModifiers::ALT)).await,
        Some(InputAction::FocusPrevTeammate)
    ));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.output_view.is_none());
}

// ===========================================================================
// Overlay panels routing
// ===========================================================================

#[tokio::test]
async fn backend_panel_keys_route() {
    let mut app = support::make_app();
    app.backend_panel = Some(BackendPanelState {
        rows: vec![
            BackendPanelRow {
                id: "local".to_string(),
                name: "Local".to_string(),
                kind: "local",
                active: true,
                health: "ok",
                health_ok: true,
                detail: None,
                connection: "host".to_string(),
            },
            BackendPanelRow {
                id: "sandbox".to_string(),
                name: "Sandbox".to_string(),
                kind: "podman",
                active: false,
                health: "ok",
                health_ok: true,
                detail: None,
                connection: "vol://x".to_string(),
            },
        ],
        cursor: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.backend_panel.as_ref().expect("open").cursor, 1);
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.backend_panel.as_ref().expect("open").cursor, 0);
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.backend_panel.is_none());
}

#[tokio::test]
async fn queue_show_panel_keys_route() {
    let mut app = support::make_app();
    app.queue_show_open = true;
    app.input_queue.push_back(queued("one"));
    app.input_queue.push_back(queued("two"));
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.queue_show_selected, 1);
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.queue_show_selected, 0);
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Delete)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(!app.queue_show_open);
}

#[tokio::test]
async fn queue_menu_keys_route() {
    let mut app = support::make_app();
    app.queue_menu_open = true;
    app.input_queue.push_back(queued("one"));
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    // Esc closes the menu.
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(!app.queue_menu_open);
}

#[tokio::test]
async fn plugin_store_panel_keys_route() {
    use ragent_plugins::StoreKind;
    let mut app = support::make_app();
    app.plugin_store = Some(ragent_tui::app::PluginStoreBrowser::new(
        StoreKind::Codex,
        "",
        false,
    ));
    for k in [
        key(KeyCode::Tab),
        key(KeyCode::BackTab),
        key(KeyCode::Up),
        key(KeyCode::Down),
        key(KeyCode::Enter),
        key(KeyCode::Char('c')),
        key(KeyCode::Backspace),
        key(KeyCode::Char('q')),
    ] {
        assert!(handle_key(&mut app, k).await.is_none());
    }
}

#[tokio::test]
async fn connector_store_panel_keys_route() {
    let mut app = support::make_app();
    app.connector_store = Some(ragent_connectors::CatalogueBrowser::new("", false));
    // Esc on an *empty* query closes the panel.
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.connector_store.is_none());

    // With a multi-char query typed, Esc edits the query instead of closing.
    let mut app = support::make_app();
    app.connector_store = Some(ragent_connectors::CatalogueBrowser::new("", false));
    for k in [
        key(KeyCode::Tab),
        key(KeyCode::Up),
        key(KeyCode::Down),
        key(KeyCode::Enter),
        key(KeyCode::Char('c')),
        key(KeyCode::Char('q')),
        key(KeyCode::Char('z')),
    ] {
        assert!(handle_key(&mut app, k).await.is_none());
    }
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(
        app.connector_store.is_some(),
        "Esc edits a non-empty query rather than closing"
    );
}

// ===========================================================================
// Normal editing matrix
// ===========================================================================

#[tokio::test]
async fn enter_on_empty_input_is_noop() {
    let mut app = support::make_app();
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
}

#[tokio::test]
async fn shift_enter_inserts_newline() {
    let mut app = support::make_app();
    app.input = "abc".to_string();
    app.input_cursor = 3;
    assert!(
        handle_key(&mut app, kmod(KeyCode::Enter, KeyModifiers::SHIFT))
            .await
            .is_none()
    );
    assert_eq!(app.input, "abc\n");
}

#[tokio::test]
async fn alt_enter_inserts_newline() {
    let mut app = support::make_app();
    app.input = "xy".to_string();
    app.input_cursor = 2;
    assert!(
        handle_key(&mut app, kmod(KeyCode::Enter, KeyModifiers::ALT))
            .await
            .is_none()
    );
    assert_eq!(app.input, "xy\n");
}

#[tokio::test]
async fn enter_sends_and_bang_and_slash() {
    let mut app = support::make_app();
    app.input = "hello".to_string();
    app.input_cursor = 5;
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::SendMessage(ref s)) if s == "hello"));

    let mut app = support::make_app();
    app.input = "! ls".to_string();
    app.input_cursor = 4;
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::BangCommand(_))));

    let mut app = support::make_app();
    app.input = "/help".to_string();
    app.input_cursor = 5;
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;
    assert!(matches!(action, Some(InputAction::SlashCommand(_))));
}

#[tokio::test]
async fn ctrl_x_cancels_while_processing_else_cuts() {
    let mut app = support::make_app();
    app.is_processing = true;
    let action = handle_key(&mut app, kmod(KeyCode::Char('x'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::CancelAgent)));

    let mut app = support::make_app();
    let action = handle_key(&mut app, kmod(KeyCode::Char('x'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::CutToClipboard)));
}

#[tokio::test]
async fn ctrl_v_pastes_alt_v_pastes_image() {
    let mut app = support::make_app();
    let action = handle_key(&mut app, kmod(KeyCode::Char('v'), KeyModifiers::CONTROL)).await;
    assert!(matches!(action, Some(InputAction::PasteFromClipboard)));

    let mut app = support::make_app();
    // Alt+V routes to the image-paste path (no clipboard image -> no-op).
    assert!(
        handle_key(&mut app, kmod(KeyCode::Char('v'), KeyModifiers::ALT))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn question_mark_on_empty_input_opens_shortcuts() {
    let mut app = support::make_app();
    assert!(
        handle_key(&mut app, key(KeyCode::Char('?')))
            .await
            .is_none()
    );
    assert!(app.show_shortcuts);
}

#[tokio::test]
async fn control_editing_keys_map_to_actions() {
    let mut app = support::make_app();
    for (code, expected) in [
        ('a', "SelectAll"),
        ('e', "MoveCursorEnd"),
        ('b', "MoveCursorLeft"),
        ('f', "MoveCursorRight"),
        ('w', "DeletePrevWord"),
        ('k', "DeleteToLineEnd"),
        ('u', "ClearLine"),
    ] {
        let action = handle_key(&mut app, kmod(KeyCode::Char(code), KeyModifiers::CONTROL)).await;
        assert!(action.is_some(), "Ctrl+{code} should map to {expected}");
    }
}

#[tokio::test]
async fn alt_toggle_keys_map_to_actions() {
    let mut app = support::make_app();
    for (c, name) in [
        ('l', "ToggleLog"),
        ('p', "ToggleProfile"),
        ('i', "ToggleTasksPanel"),
        ('m', "ToggleMemory"),
        ('o', "ToggleTelemetry"),
        ('c', "ToggleContextPanel"),
        ('y', "ToggleYolo"),
        ('e', "ToggleEditLog"),
        ('g', "ToggleGcf"),
        ('q', "OpenQueueMenu"),
    ] {
        let action = handle_key(&mut app, kmod(KeyCode::Char(c), KeyModifiers::ALT)).await;
        assert!(action.is_some(), "Alt+{c} should map to {name}");
    }
}

#[tokio::test]
async fn alt_x_opens_stop_confirm_only_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    assert!(
        handle_key(&mut app, kmod(KeyCode::Char('x'), KeyModifiers::ALT))
            .await
            .is_none()
    );
    assert!(app.pending_stop_confirm);

    let mut app = support::make_app();
    assert!(
        handle_key(&mut app, kmod(KeyCode::Char('x'), KeyModifiers::ALT))
            .await
            .is_none()
    );
    assert!(!app.pending_stop_confirm);
}

#[tokio::test]
async fn arrow_and_motion_keys_map_to_actions() {
    let mut app = support::make_app();
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Left)).await,
        Some(InputAction::MoveCursorLeft)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Right)).await,
        Some(InputAction::MoveCursorRight)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Home)).await,
        Some(InputAction::MoveCursorHome)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::End)).await,
        Some(InputAction::MoveCursorEnd)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Left, KeyModifiers::CONTROL)).await,
        Some(InputAction::MoveCursorWordLeft)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Right, KeyModifiers::CONTROL)).await,
        Some(InputAction::MoveCursorWordRight)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Left, KeyModifiers::SHIFT)).await,
        Some(InputAction::SelectCharLeft)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Right, KeyModifiers::SHIFT)).await,
        Some(InputAction::SelectCharRight)
    ));
    assert!(matches!(
        handle_key(
            &mut app,
            kmod(KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        )
        .await,
        Some(InputAction::SelectWordLeft)
    ));
    assert!(matches!(
        handle_key(
            &mut app,
            kmod(KeyCode::Right, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        )
        .await,
        Some(InputAction::SelectWordRight)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Up, KeyModifiers::SHIFT)).await,
        Some(InputAction::ScrollUp)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Down, KeyModifiers::SHIFT)).await,
        Some(InputAction::ScrollDown)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Up, KeyModifiers::ALT)).await,
        Some(InputAction::FocusPrevTeammate)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::Down, KeyModifiers::ALT)).await,
        Some(InputAction::FocusNextTeammate)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Up)).await,
        Some(InputAction::HistoryUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Down)).await,
        Some(InputAction::HistoryDown)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageUp, KeyModifiers::CONTROL)).await,
        Some(InputAction::LogScrollUp)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageDown, KeyModifiers::CONTROL)).await,
        Some(InputAction::LogScrollDown)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageUp)).await,
        Some(InputAction::ScrollUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageDown)).await,
        Some(InputAction::ScrollDown)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Tab)).await,
        Some(InputAction::SwitchAgent)
    ));
}

#[tokio::test]
async fn esc_cancels_only_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Esc)).await,
        Some(InputAction::CancelAgent)
    ));

    let mut app = support::make_app();
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
}

#[tokio::test]
async fn memory_panel_up_down_routes_to_cursor() {
    let mut app = support::make_app();
    app.show_memory = true;
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Up)).await,
        Some(InputAction::MemoryCursorUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Down)).await,
        Some(InputAction::MemoryCursorDown)
    ));
}

#[tokio::test]
async fn delete_with_active_selection_removes_range() {
    let mut app = support::make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.kb_select_anchor = Some(0);
    assert!(handle_key(&mut app, key(KeyCode::Delete)).await.is_none());
    assert!(app.input.is_empty(), "got: {:?}", app.input);
}

#[tokio::test]
async fn backspace_with_active_selection_removes_range() {
    let mut app = support::make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.kb_select_anchor = Some(0);
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    assert!(app.input.is_empty(), "got: {:?}", app.input);
}

#[tokio::test]
async fn typing_replaces_active_selection() {
    let mut app = support::make_app();
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.kb_select_anchor = Some(0);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('Z')))
            .await
            .is_none()
    );
    assert_eq!(app.input, "Z");
}

// ===========================================================================
// Router setup key handling
// ===========================================================================

fn router_app() -> App {
    use ragent_tui::app::{ConfiguredProvider, ProviderSource};
    let mut app = support::make_app();
    app.provider_setup = Some(ragent_tui::app::ProviderSetupStep::SetupRouter {
        providers: vec![ConfiguredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic".to_string(),
            source: ProviderSource::Database,
        }],
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: ragent_llm::providers::router_config::RouterConfig::default(),
        active_bucket: ragent_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    app
}

#[tokio::test]
async fn router_setup_tab_and_bucket_navigation() {
    let mut app = router_app();
    // Tab flips focus to the bucket pane and clears the bucket cursor.
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ragent_tui::app::ProviderSetupStep::SetupRouter {
            left_pane_focused, ..
        } => assert!(!left_pane_focused),
        _ => panic!("expected SetupRouter"),
    }
    // Left/Right cycle the active bucket while the bucket pane is focused.
    assert!(handle_key(&mut app, key(KeyCode::Right)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Left)).await.is_none());
    // Up/Down move within the pane (no models -> index stays 0).
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    // Esc closes the dialog.
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn router_setup_space_toggles_provider_selection() {
    let mut app = router_app();
    // Space on the highlighted provider removes it from the selection.
    assert!(
        handle_key(&mut app, key(KeyCode::Char(' ')))
            .await
            .is_none()
    );
    match app.provider_setup.as_ref().expect("setup") {
        ragent_tui::app::ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            ..
        } => assert_eq!(selected_provider_ids.len(), 0),
        _ => panic!("expected SetupRouter"),
    }
    // Space again re-adds it.
    assert!(
        handle_key(&mut app, key(KeyCode::Char(' ')))
            .await
            .is_none()
    );
    match app.provider_setup.as_ref().expect("setup") {
        ragent_tui::app::ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            ..
        } => assert_eq!(selected_provider_ids, &vec!["anthropic".to_string()]),
        _ => panic!("expected SetupRouter"),
    }
}

#[tokio::test]
async fn router_setup_ctrl_s_without_models_sets_error() {
    let mut app = router_app();
    assert!(
        handle_key(&mut app, kmod(KeyCode::Char('s'), KeyModifiers::CONTROL))
            .await
            .is_none()
    );
    assert!(app.pending_router_save.is_none(), "no model -> no save");
}
