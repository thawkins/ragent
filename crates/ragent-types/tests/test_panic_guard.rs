//! Unit tests for the contained-panic guard.

use ragent_types::panic_guard;

#[test]
fn test_panic_guard_reports_outside_container() {
    assert!(!panic_guard::is_active());
}

#[test]
fn test_panic_guard_active_inside_container() {
    let observed = panic_guard::run(panic_guard::is_active);
    assert!(matches!(observed, Ok(true)));
    assert!(!panic_guard::is_active());
}

#[test]
fn test_panic_guard_catches_panic_and_clears_flag() {
    let result = panic_guard::run(|| -> i32 {
        assert!(panic_guard::is_active());
        panic!("deliberate test panic");
    });
    assert!(result.is_err());
    assert!(
        !panic_guard::is_active(),
        "flag must be reset after the panic"
    );
}

#[test]
fn test_panic_guard_returns_value_on_success() {
    let result = panic_guard::run(|| 41 + 1);
    assert!(matches!(result, Ok(42)));
    assert!(!panic_guard::is_active());
}

#[test]
fn test_panic_guard_nested_and_thread_local() {
    // Spawned thread must see its own flag (false) while the parent is active.
    let handle = std::thread::spawn(panic_guard::is_active);
    let observed_in_thread = handle.join().expect("thread join");
    assert!(!observed_in_thread);

    let nested = panic_guard::run(|| panic_guard::run(panic_guard::is_active));
    assert!(matches!(nested, Ok(Ok(true))));
    assert!(!panic_guard::is_active());
}
