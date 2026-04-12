#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_core::team::{Mailbox, MailboxMessage, MessageType};
use tempfile::tempdir;

fn bench_mailbox_push(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let team_dir = dir.path();
    let mailbox = Mailbox::open(team_dir, "agent-bench").expect("open mailbox");

    c.bench_function("mailbox_push_message", |b| {
        b.iter(|| {
            let msg = MailboxMessage::new(
                "sender-agent",
                "agent-bench",
                MessageType::Message,
                "benchmark message payload",
            );
            mailbox.push(msg).expect("push");
        });
    });
}

fn bench_mailbox_drain(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let team_dir = dir.path();
    let mailbox = Mailbox::open(team_dir, "agent-drain").expect("open mailbox");

    // Pre-populate with 20 messages
    for i in 0..20 {
        let msg = MailboxMessage::new(
            "sender",
            "agent-drain",
            MessageType::Message,
            format!("message {i}"),
        );
        mailbox.push(msg).expect("push");
    }

    c.bench_function("mailbox_drain_20_messages", |b| {
        // Repopulate before each iteration
        b.iter(|| {
            let msgs = mailbox.drain_unread().expect("drain");
            std::hint::black_box(msgs);
        });
    });
}

fn bench_mailbox_read_all(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let team_dir = dir.path();
    let mailbox = Mailbox::open(team_dir, "agent-read").expect("open mailbox");

    for i in 0..20 {
        let msg = MailboxMessage::new(
            "sender",
            "agent-read",
            MessageType::Message,
            format!("message {i}"),
        );
        mailbox.push(msg).expect("push");
    }

    c.bench_function("mailbox_read_all_20_messages", |b| {
        b.iter(|| {
            let msgs = mailbox.read_all().expect("read_all");
            std::hint::black_box(msgs);
        });
    });
}

criterion_group!(
    benches,
    bench_mailbox_push,
    bench_mailbox_drain,
    bench_mailbox_read_all
);
criterion_main!(benches);
