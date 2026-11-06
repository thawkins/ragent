//! Consolidated test target for `ragent-hyperresearch`.

/// The crate-wide lock serialising tests that touch a process-global.
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Take the crate-wide lock that serialises tests touching a process-global.
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex().lock().unwrap_or_else(|e| e.into_inner())
}

#[path = "test_note_roundtrip.rs"]
mod test_note_roundtrip;
