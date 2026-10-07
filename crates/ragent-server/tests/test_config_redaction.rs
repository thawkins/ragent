//! Audit T-108: `GET /config` must redact credential values by key *shape*,
//! not by a hardcoded path list, so a newly-added secret field is masked
//! without editing the redactor.

use ragent_server::routes::{REDACTED, redact_config_secrets};
use serde_json::json;

#[test]
fn test_redacts_known_and_newly_named_secret_fields() {
    let mut value = json!({
        "tavily_api_key": "tvly-1234567890",
        "gitlab": { "token": "glpat-abcdef", "instance_url": "https://gitlab.com" },
        "channels": {
            "telegram": { "bot_token": "123:abc" },
            "discord": { "webhook_url": "https://discord.com/api/webhooks/x/y" }
        },
        "provider": {
            "anthropic": { "api_key": "sk-ant-xyz" },
            // A hypothetical future secret field: redacted by shape alone.
            "future": { "some_new_secret": "leak-me" }
        },
        "gmail": { "client_secret": "GOCSPX-secret", "client_id": "abc.apps.googleusercontent.com" }
    });

    redact_config_secrets(&mut value);

    assert_eq!(value["tavily_api_key"], REDACTED);
    assert_eq!(value["gitlab"]["token"], REDACTED);
    assert_eq!(value["channels"]["telegram"]["bot_token"], REDACTED);
    assert_eq!(value["channels"]["discord"]["webhook_url"], REDACTED);
    assert_eq!(value["provider"]["anthropic"]["api_key"], REDACTED);
    assert_eq!(value["provider"]["future"]["some_new_secret"], REDACTED);
    assert_eq!(value["gmail"]["client_secret"], REDACTED);
}

#[test]
fn test_preserves_non_secret_values() {
    let mut value = json!({
        "default_agent": "coder",
        "gitlab": { "instance_url": "https://gitlab.com" },
        "codeindex": { "keywords": ["rust", "config"], "token_budget": 4096 },
        "provider": { "ollama": { "base_url": "http://localhost:11434" } }
    });

    redact_config_secrets(&mut value);

    assert_eq!(value["default_agent"], "coder");
    assert_eq!(value["gitlab"]["instance_url"], "https://gitlab.com");
    assert_eq!(value["codeindex"]["keywords"][0], "rust");
    assert_eq!(value["codeindex"]["token_budget"], 4096);
    assert_eq!(
        value["provider"]["ollama"]["base_url"],
        "http://localhost:11434"
    );
}

#[test]
fn test_redacts_secrets_nested_in_arrays() {
    let mut value = json!({
        "plugins": [
            { "name": "a", "api_key": "secret-a" },
            { "name": "b", "nested": [{ "refresh_token": "secret-b" }] }
        ]
    });

    redact_config_secrets(&mut value);

    assert_eq!(value["plugins"][0]["api_key"], REDACTED);
    assert_eq!(value["plugins"][1]["nested"][0]["refresh_token"], REDACTED);
    assert_eq!(value["plugins"][0]["name"], "a");
}

#[test]
fn test_empty_and_already_redacted_values_are_left_alone() {
    let mut value = json!({ "api_key": "", "token": "<redacted>" });
    redact_config_secrets(&mut value);
    assert_eq!(value["api_key"], "");
    assert_eq!(value["token"], REDACTED);
}
