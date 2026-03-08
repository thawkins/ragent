use ragent_core::event::{Event, EventBus};

#[test]
fn test_event_bus_publish_subscribe() {
    let bus = EventBus::default();
    let mut rx = bus.subscribe();

    let event = Event::SessionCreated {
        session_id: "s1".to_string(),
    };
    bus.publish(event);

    let received = rx.try_recv().unwrap();
    match received {
        Event::SessionCreated { session_id } => assert_eq!(session_id, "s1"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn test_event_bus_multiple_subscribers() {
    let bus = EventBus::default();
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.publish(Event::TextDelta {
        session_id: "s1".to_string(),
        text: "hello".to_string(),
    });

    let e1 = rx1.try_recv().unwrap();
    let e2 = rx2.try_recv().unwrap();

    match (&e1, &e2) {
        (
            Event::TextDelta {
                session_id: s1,
                text: t1,
            },
            Event::TextDelta {
                session_id: s2,
                text: t2,
            },
        ) => {
            assert_eq!(s1, "s1");
            assert_eq!(t1, "hello");
            assert_eq!(s2, "s1");
            assert_eq!(t2, "hello");
        }
        _ => panic!("unexpected events: {e1:?}, {e2:?}"),
    }
}
