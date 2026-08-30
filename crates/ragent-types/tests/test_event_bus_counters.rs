//! Tests for the shared [`EventBus`] counters.

use ragent_types::event::EventBus;

#[test]
fn test_event_bus_step_and_tool_call_counters_are_independent() {
    let bus = EventBus::new(64);

    // Loop step counter is set explicitly.
    bus.set_step("sess-a", 3);
    assert_eq!(bus.current_step("sess-a"), 3);
    assert_eq!(bus.current_tool_calls("sess-a"), 0);

    // Tool-call counter is incremented independently.
    bus.increment_tool_calls("sess-a");
    bus.increment_tool_calls("sess-a");
    assert_eq!(bus.current_tool_calls("sess-a"), 2);
    // Step counter is untouched.
    assert_eq!(bus.current_step("sess-a"), 3);
}

#[test]
fn test_set_step_zero_clears_tool_call_counter() {
    let bus = EventBus::new(64);

    bus.set_step("sess-a", 2);
    bus.increment_tool_calls("sess-a");
    bus.increment_tool_calls("sess-a");
    assert_eq!(bus.current_step("sess-a"), 2);
    assert_eq!(bus.current_tool_calls("sess-a"), 2);

    // Resetting the step counter (used at the start of a new run) also resets
    // tool-call count so the UI starts from zero.
    bus.set_step("sess-a", 0);
    assert_eq!(bus.current_step("sess-a"), 0);
    assert_eq!(bus.current_tool_calls("sess-a"), 0);
}

#[test]
fn test_tool_call_counter_per_session() {
    let bus = EventBus::new(64);

    bus.increment_tool_calls("sess-a");
    bus.increment_tool_calls("sess-b");
    bus.increment_tool_calls("sess-b");

    assert_eq!(bus.current_tool_calls("sess-a"), 1);
    assert_eq!(bus.current_tool_calls("sess-b"), 2);
    assert_eq!(bus.current_tool_calls("sess-c"), 0);
}
