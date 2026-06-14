//! Error type for Foundry Local service start failures.
//!
//! Carries structured diagnostics (full command path, captured stdout/stderr)
//! so the TUI can display a detailed error dialog when the service fails to
//! start.

/// Error type for Foundry Local service start failures.
///
/// Carries structured diagnostics (full command path, captured stdout/stderr)
/// so the TUI can display a detailed error dialog when the service fails to
/// start within the timeout.
#[derive(Debug, Clone)]
pub struct FoundryServiceError {
    /// Full resolved path of the command that was run.
    pub command_path: String,
    /// Captured standard output from the command (may be empty).
    pub stdout: String,
    /// Captured standard error from the command (may be empty).
    pub stderr: String,
    /// Human-readable summary of the failure.
    pub error: String,
}

impl std::fmt::Display for FoundryServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if !self.command_path.is_empty() {
            write!(f, "\nCommand: {}", self.command_path)?;
        }
        if !self.stderr.is_empty() {
            write!(f, "\nstderr: {}", self.stderr)?;
        }
        if !self.stdout.is_empty() {
            write!(f, "\nstdout: {}", self.stdout)?;
        }
        Ok(())
    }
}

impl std::error::Error for FoundryServiceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foundry_service_error_display() {
        let err = FoundryServiceError {
            command_path: "/usr/local/bin/foundry".to_string(),
            stdout: "Starting service...".to_string(),
            stderr: "Error: port already in use".to_string(),
            error: "Service failed to start".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Service failed to start"));
        assert!(msg.contains("/usr/local/bin/foundry"));
        assert!(msg.contains("Error: port already in use"));
        assert!(msg.contains("Starting service..."));
    }

    #[test]
    fn test_foundry_service_error_display_minimal() {
        let err = FoundryServiceError {
            command_path: String::new(),
            stdout: String::new(),
            stderr: String::new(),
            error: "Service not running".to_string(),
        };
        let msg = format!("{err}");
        assert_eq!(msg, "Service not running");
    }
}
