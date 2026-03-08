use std::sync::LazyLock;

use regex::Regex;

static SECRET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(sk-[a-zA-Z0-9]{20,}|key-[a-zA-Z0-9]{20,}|Bearer [a-zA-Z0-9\-]{20,})")
        .expect("valid regex pattern")
});

pub fn redact_secrets(msg: &str) -> String {
    SECRET_PATTERN.replace_all(msg, "[REDACTED]").into_owned()
}
