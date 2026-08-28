//! Criterion benchmarks for the three main TUI panels:
//! - message list (`render_messages`)
//! - active-agents subpanel (`render_active_agents_subpanel`)
//! - teams subpanel (`render_teams_subpanel`)
//!
//! Covers spec tuiopt T-012 (FR-001, FR-003):
//! Each panel is benchmarked at varying message/agent/team counts in both
//! **cold** (cache empty — first render) and **warm** (cache populated —
//! subsequent render with unchanged state) modes, demonstrating the
//! per-frame cost reduction delivered by the FR-003 derived-data caches.

#![allow(missing_docs)]

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ratatui::{Terminal, backend::TestBackend};

use ragent_agent::event::EventBus;
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::task::{TaskEntry, TaskStatus};
use ragent_agent::trigger::TriggerRuntime;
use ragent_team::team::{MemberStatus, TeamConfig, TeamMember};

use ragent_tui::layout::render_messages;
use ragent_tui::layout_active_agents::render_active_agents_subpanel;
use ragent_tui::layout_teams::render_teams_subpanel;

#[path = "../tests/support/mod.rs"]
mod support;

use support::make_app;

// =========================================================================
// Helpers: message generation
// =========================================================================

/// Generate a realistic assistant message with mixed text and a tool call.
fn make_assistant_message(idx: usize, session_id: &str) -> Message {
    let text = format!(
        "This is assistant response #{idx} with **markdown** and some code:\n\
         ```rust\nfn example_{idx}() -> u32 {{ {idx} * 7 }}\n```\n\
         The answer is {}.",
        idx * 7
    );
    Message::new(
        session_id,
        Role::Assistant,
        vec![MessagePart::Text { text }],
    )
}

/// Generate a short user message.
fn make_user_message(idx: usize, session_id: &str) -> Message {
    Message::new(
        session_id,
        Role::User,
        vec![MessagePart::Text {
            text: format!("User question #{idx}: explain the concept of ownership in Rust."),
        }],
    )
}

/// Build an interleaved user/assistant message list of `count` messages.
fn build_messages(count: usize, session_id: &str) -> Vec<Message> {
    let mut msgs = Vec::with_capacity(count);
    for i in 0..count {
        if i % 2 == 0 {
            msgs.push(make_user_message(i, session_id));
        } else {
            msgs.push(make_assistant_message(i, session_id));
        }
    }
    msgs
}

// =========================================================================
// Helpers: active-agent task generation
// =========================================================================

/// Build `count` sub-agent TaskEntries attached to `parent_session`.
fn build_active_tasks(count: usize, parent_session: &str) -> Vec<TaskEntry> {
    let now = chrono::Utc::now();
    (0..count)
        .map(|i| TaskEntry {
            id: format!("task-{i:04}"),
            parent_session_id: parent_session.to_string(),
            child_session_id: format!("child-{i:04}"),
            agent_name: format!("explore-{i}"),
            task_prompt: format!("Explore directory #{i}"),
            background: i % 3 == 0,
            status: if i % 4 == 0 {
                TaskStatus::Running
            } else {
                TaskStatus::Completed
            },
            result: Some(Arc::from(format!("result {i}"))),
            error: None,
            created_at: now,
            completed_at: Some(now),
            reported: false,
            waiter_count: 0,
            output_file: None,
            report_status: ragent_agent::task::ReportStatus::Complete,
        })
        .collect()
}

// =========================================================================
// Helpers: team member generation
// =========================================================================

/// Build `count` TeamMember records for a team.
fn build_team_members(count: usize) -> Vec<TeamMember> {
    (0..count)
        .map(|i| {
            let mut m = TeamMember::new(format!("reviewer-{i}"), format!("tm-{i:03}"), "general");
            m.session_id = Some(format!("tm-session-{i:04}"));
            m.status = if i % 3 == 0 {
                MemberStatus::Working
            } else {
                MemberStatus::Idle
            };
            m
        })
        .collect()
}

// =========================================================================
// Bench: message list panel (render_messages)
// =========================================================================

fn bench_message_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_messages");

    for &count in &[10, 100, 500] {
        let session_id = "bench-session-0001";

        // Cold: clear the per-message line cache before each iteration so
        // every message must be rendered from scratch (FR-003 cache miss).
        group.bench_with_input(BenchmarkId::new("cold", count), &count, |b, &n| {
            let messages = build_messages(n, session_id);
            b.iter(|| {
                let mut app = make_app();
                app.session_id = Some(session_id.to_string());
                app.messages = messages.clone();
                app.message_line_cache.clear();
                app.message_cache_width = 0; // force re-wrap

                let backend = TestBackend::new(120, 40);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_messages(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });

        // Warm: pre-populate the cache by rendering once, then measure
        // subsequent renders with unchanged state (FR-003 cache hit).
        group.bench_with_input(BenchmarkId::new("warm", count), &count, |b, &n| {
            let messages = build_messages(n, session_id);
            let mut app = make_app();
            app.session_id = Some(session_id.to_string());
            app.messages = messages;

            // Prime the cache with one render.
            {
                let backend = TestBackend::new(120, 40);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_messages(frame, &mut app, frame.area());
                    })
                    .expect("prime cache");
            }

            b.iter(|| {
                let backend = TestBackend::new(120, 40);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_messages(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });
    }
    group.finish();
}

// =========================================================================
// Bench: active-agents panel (render_active_agents_subpanel)
// =========================================================================

fn bench_active_agents(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_active_agents_subpanel");

    for &count in &[5, 20, 50] {
        let session_id = "lead-session-0001";

        // Cold: rebuild the custom-names and teammate-ids caches each
        // iteration by invalidating the version counters (FR-003 miss).
        group.bench_with_input(BenchmarkId::new("cold", count), &count, |b, &n| {
            let tasks = build_active_tasks(n, session_id);
            b.iter(|| {
                let mut app = make_app();
                app.session_id = Some(session_id.to_string());
                app.active_tasks = tasks.clone();
                // Invalidate caches so they are rebuilt.
                app.active_agents_custom_names_version = usize::MAX;
                app.active_agents_teammate_ids_version = usize::MAX;

                let backend = TestBackend::new(120, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_active_agents_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });

        // Warm: caches are already populated from the previous iteration
        // (version matches), so the render path skips the rebuild (FR-003 hit).
        group.bench_with_input(BenchmarkId::new("warm", count), &count, |b, &n| {
            let tasks = build_active_tasks(n, session_id);
            let mut app = make_app();
            app.session_id = Some(session_id.to_string());
            app.active_tasks = tasks;

            // Prime the caches with one render.
            {
                let backend = TestBackend::new(120, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_active_agents_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("prime cache");
            }

            b.iter(|| {
                let backend = TestBackend::new(120, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_active_agents_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });
    }
    group.finish();
}

// =========================================================================
// Bench: teams panel (render_teams_subpanel)
// =========================================================================

fn bench_teams_panel(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_teams_subpanel");

    for &count in &[2, 10, 20] {
        let session_id = "lead-session-0001";

        // Cold: force task-counts to reload from disk by setting the dirty
        // flag before each iteration (FR-009 one-shot hydration miss).
        group.bench_with_input(BenchmarkId::new("cold", count), &count, |b, &n| {
            let members = build_team_members(n);
            b.iter(|| {
                let mut app = make_app();
                app.session_id = Some(session_id.to_string());
                app.active_team = Some(TeamConfig::new("bench-team", session_id));
                app.team_members = members.clone();
                app.team_task_counts_dirty = true;
                app.lead_session_created_at = Some(chrono::Utc::now());

                let backend = TestBackend::new(160, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_teams_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });

        // Warm: caches are already populated (dirty = false, counts in
        // memory), so the render path reads exclusively from in-memory state
        // (FR-009 cache hit — no disk reads per frame).
        group.bench_with_input(BenchmarkId::new("warm", count), &count, |b, &n| {
            let members = build_team_members(n);
            let mut app = make_app();
            app.session_id = Some(session_id.to_string());
            app.active_team = Some(TeamConfig::new("bench-team", session_id));
            app.team_members = members;
            app.lead_session_created_at = Some(chrono::Utc::now());

            // Prime caches with one render.
            {
                let backend = TestBackend::new(160, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_teams_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("prime cache");
            }

            b.iter(|| {
                let backend = TestBackend::new(160, 24);
                let mut terminal = Terminal::new(backend).expect("test terminal");
                terminal
                    .draw(|frame| {
                        render_teams_subpanel(frame, &mut app, frame.area());
                    })
                    .expect("draw");
            });
        });
    }
    group.finish();
}

// Suppress unused-import warnings for items that are part of the public API
// surface exercised by the benchmark helpers.
#[allow(dead_code)]
fn _suppress_unused() {
    let _ = EventBus::default();
    let _ = TriggerRuntime::default();
}

criterion_group!(
    benches,
    bench_message_list,
    bench_active_agents,
    bench_teams_panel,
);
criterion_main!(benches);
