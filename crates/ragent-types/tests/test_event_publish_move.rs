//! Tests binding the PERF-054 contract for [`ragent_types::event::EventBus::publish`]:
//! publishing moves the event into the broadcast channel instead of cloning its
//! payload.
//!
//! The move itself is enforced at compile time (the call is `send(event)`, not
//! `send(event.clone())`); these tests pin the observable behaviour that the
//! change must preserve — a large payload reaches every subscriber intact and
//! publishing with no subscribers is a silent no-op.

use ragent_types::event::{Event, EventBus};

#[test]
fn large_text_delta_payload_reaches_all_subscribers() {
    let bus = EventBus::new(64);
    let mut rx_a = bus.subscribe();
    let mut rx_b = bus.subscribe();

    // A large payload: pre-change this was deep-cloned once per publish, once
    // per receiver internally. The content must still arrive byte-identical.
    let text = "x".repeat(100_000);
    bus.publish(Event::TextDelta {
        session_id: "sess-perf054".to_string(),
        text: text.clone(),
    });

    for rx in [&mut rx_a, &mut rx_b] {
        match rx.try_recv().expect("event delivered") {
            Event::TextDelta { text: got, .. } => assert_eq!(got, text),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}

#[test]
fn publish_without_subscribers_does_not_panic() {
    let bus = EventBus::new(8);
    bus.publish(Event::SessionCreated {
        session_id: "sess-perf054-none".to_string(),
    });
}
