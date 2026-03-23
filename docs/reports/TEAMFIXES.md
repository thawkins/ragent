# Team Fixes for ragent-server

- task-001: Remove logging of server auth token
  - File: crates/ragent-server/src/routes/mod.rs
  - Short: Logging the auth token at startup exposes a secret. Replace with a non-sensitive message and ensure tests don't log the token.

- task-002: Handle poisoned Mutex in rate limiter lock
  - File: crates/ragent-server/src/routes/mod.rs
  - Short: Current code calls `lock().unwrap_or_else(...)`, which silently recovers from poison. Replace with explicit error handling returning 500 and add unit test.

- task-003: Use constant-time comparison for auth token check
  - File: crates/ragent-server/src/routes/mod.rs
  - Short: Use a constant-time comparison (e.g., `subtle::ConstantTimeEq`) to compare bearer tokens to mitigate timing attacks.

