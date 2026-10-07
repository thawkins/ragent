//! Integration tests for the `os_info` tool.
//!
//! Covers the externally observable contract of spec `osinfo`:
//!
//! - registration in the default registry with the `none` permission
//!   category (FR-001);
//! - OS identity fields present in both renderings (FR-002, FR-003);
//! - text format renders the four markdown sections and omits JSON braces
//!   (FR-004);
//! - JSON format is a single flat object with the documented snake_case key
//!   set and matches the metadata payload (FR-004);
//! - an unknown `format` value errors with a message naming the offending
//!   value and the accepted ones (FR-004);
//! - the parameters schema exposes only `format` and `probe` with
//!   `additionalProperties: false` (FR-010);
//! - `probe: false` selects the process-free pass while the default probes
//!   (FR-020);
//! - each detected graphics API carries a version (`gpu_apis` element objects
//!   with `name` and `version`; text `- **APIs**` line renders `Name Version`)
//!   (FR-021).

use ragent_agent::event::EventBus;
use ragent_agent::tool::{ToolContext, create_default_registry};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// Build a minimal [`ToolContext`] for a read-only tool call.
///
/// `os_info` needs no storage, model, provider, or configuration, so every
/// optional field is left empty to prove the tool is self-contained.
fn base_ctx() -> ToolContext {
    ToolContext {
        session_id: "session-1".to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// The flat snake_case key set the JSON format must always expose (FR-004).
const EXPECTED_JSON_KEYS: &[&str] = &[
    "os_family",
    "os_name",
    "os_version",
    "os_long_version",
    "kernel_version",
    "hostname",
    "distribution",
    "cpu_arch",
    "physical_cores",
    "logical_cores",
    "cpu_vendor",
    "cpu_brand",
    "cpu_frequency_mhz",
    "gpus",
    "gpu_apis",
    "system_vendor",
    "system_model",
    "system_version",
    "product_family",
    "chassis_type",
    "chassis_vendor",
    "board_vendor",
    "board_name",
    "board_version",
    "bios_vendor",
    "bios_version",
    "bios_date",
    "storage_devices",
    "network_interfaces",
    "total_memory_bytes",
    "available_memory_bytes",
    "total_swap_bytes",
    "free_swap_bytes",
    "uptime_seconds",
    "boot_time_utc",
    "pid",
    "working_directory",
    "shell",
    "username",
];

#[test]
fn test_os_info_is_registered() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    assert_eq!(tool.name(), "os_info");
    assert_eq!(
        tool.permission_category(),
        "none",
        "os_info is read-only and must not prompt for permission"
    );

    let names = registry.list();
    assert_eq!(
        names.iter().filter(|n| n.as_str() == "os_info").count(),
        1,
        "os_info should appear exactly once in the registry listing"
    );
}

#[test]
fn test_os_info_schema_only_exposes_format() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let schema = tool.parameters_schema();

    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["additionalProperties"],
        json!(false),
        "FR-010: no user-controlled input surface beyond format"
    );

    let properties = schema["properties"]
        .as_object()
        .expect("properties object present");
    assert_eq!(
        properties.keys().collect::<Vec<_>>(),
        vec!["format", "probe"],
        "only the optional format and probe parameters are allowed"
    );
    assert_eq!(properties["format"]["enum"], json!(["text", "json"]));
    assert_eq!(properties["probe"]["type"], json!("boolean"));
}

#[tokio::test]
async fn test_os_info_probe_false_selects_process_free_pass() {
    // FR-020: `probe: false` skips the vendor diagnostics and reports from the
    // read-only inference alone. The API set must still be well-formed, with
    // every version non-empty, and the text line must use it (FR-021).
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool
        .execute(json!({"probe": false}), &ctx)
        .await
        .expect("execute ok");
    let meta = output.metadata.expect("metadata present");
    for api in meta["gpu_apis"].as_array().expect("gpu_apis array") {
        let version = api["version"].as_str().expect("api version");
        assert!(
            !version.is_empty(),
            "FR-021: api version must never be empty: {api}"
        );
        if version != "unknown" {
            let name = api["name"].as_str().expect("api name");
            assert!(
                output.content.contains(&format!("{name} {version}")),
                "text report must render `{name} {version}`: {}",
                output.content
            );
        }
    }
}

#[tokio::test]
async fn test_os_info_text_renders_documented_sections() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");

    for section in [
        "## Operating System",
        "## CPU",
        "## GPU",
        "## Hardware",
        "## Memory",
        "## Process",
    ] {
        assert!(
            output.content.contains(section),
            "text report missing section `{section}`: {}",
            output.content
        );
    }

    // An explicit `format: text` call must match the no-argument default.
    let explicit = tool
        .execute(json!({"format": "text"}), &ctx)
        .await
        .expect("execute ok");
    assert!(
        explicit.content.contains("## Operating System"),
        "explicit text format must render markdown"
    );
    assert!(
        !explicit.content.trim_start().starts_with('{'),
        "text format must not emit a JSON object: {}",
        explicit.content
    );

    // Text output still carries the structured payload for downstream consumers.
    let meta = explicit.metadata.expect("text format carries metadata");
    assert!(meta["os_family"].is_string());
}

#[tokio::test]
async fn test_os_info_text_reports_os_identity() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");

    // FR-002: the identity fields are always labelled in the text report.
    for label in [
        "**Family**",
        "**Name**",
        "**Version**",
        "**Kernel**",
        "**Hostname**",
    ] {
        assert!(
            output.content.contains(label),
            "identity label `{label}` missing: {}",
            output.content
        );
    }

    // FR-003: the distribution line is always present and, on Linux, is a
    // `id / display-name` pair; off Linux it is the `n/a (<family>)` marker.
    assert!(
        output.content.contains("**Distribution**: "),
        "distribution line missing: {}",
        output.content
    );
    let meta = output.metadata.expect("metadata present");
    let family = meta["os_family"].as_str().expect("os_family string");
    assert!(!family.is_empty(), "os_family must be populated");
    let distribution = meta["distribution"].as_str().expect("distribution string");
    if family == "linux" {
        assert!(
            distribution.contains(" / "),
            "linux distribution should be `id / name`: {distribution}"
        );
    } else {
        assert_eq!(
            distribution,
            format!("n/a ({family})"),
            "non-linux distribution should be the n/a marker"
        );
    }
}

#[tokio::test]
async fn test_os_info_text_reports_gpu_adapters() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");

    // FR-019: the GPU section is always present. On Linux with a DRM card it
    // lists an adapter (name/vendor/PCI ids/driver/kind); otherwise it renders
    // the `Adapters: unknown` placeholder so the section never disappears.
    assert!(
        output.content.contains("## GPU"),
        "GPU section must always render: {}",
        output.content
    );

    // FR-020: the graphics-API line is always present (a comma-separated list,
    // or the `none detected` placeholder) and never on a platform where the API
    // is out of scope.
    assert!(
        output.content.contains("**APIs**:"),
        "GPU API line must always render: {}",
        output.content
    );

    let meta = output.metadata.expect("metadata present");
    let gpus = meta["gpus"].as_array().expect("gpus array");
    let family = meta["os_family"].as_str().expect("os_family string");
    if gpus.is_empty() {
        assert!(
            output.content.contains("**Adapters**: unknown"),
            "empty GPU list must render the unknown placeholder: {}",
            output.content
        );
    } else {
        for key in [
            "**Adapter**",
            "**Vendor**",
            "**PCI Ids**",
            "**Driver**",
            "**Kind**",
        ] {
            assert!(
                output.content.contains(key),
                "GPU adapter line `{key}` missing on {family}: {}",
                output.content
            );
        }
    }
}

#[tokio::test]
async fn test_os_info_json_format_is_structured() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool
        .execute(json!({"format": "json"}), &ctx)
        .await
        .expect("execute ok");

    assert!(
        !output.content.contains("## "),
        "json format must not contain markdown headings: {}",
        output.content
    );

    let parsed: Value = serde_json::from_str(&output.content).expect("valid json");
    let obj = parsed.as_object().expect("json object");

    for key in EXPECTED_JSON_KEYS {
        assert!(
            obj.contains_key(*key),
            "json format missing key `{key}`: {parsed}"
        );
    }

    // Values are typed as documented (FR-002, FR-005, FR-006, FR-007).
    assert!(parsed["os_family"].is_string());
    assert!(parsed["hostname"].is_string());
    assert!(parsed["cpu_arch"].is_string());
    assert!(
        parsed["gpus"].is_array(),
        "FR-019: gpus must always be an array: {parsed}"
    );
    assert!(
        parsed["gpu_apis"].is_array(),
        "FR-020: gpu_apis must always be an array: {parsed}"
    );

    // FR-021: every gpu_apis element is an object carrying `name` and `version`.
    for api in parsed["gpu_apis"].as_array().expect("gpu_apis array") {
        assert!(
            api.get("name").is_some_and(Value::is_string),
            "FR-021: gpu_apis element missing string `name`: {api}"
        );
        assert!(
            api.get("version").is_some_and(Value::is_string),
            "FR-021: gpu_apis element missing string `version`: {api}"
        );
    }
    assert!(parsed["physical_cores"].is_u64());
    assert!(parsed["logical_cores"].is_u64());
    assert!(parsed["total_memory_bytes"].is_u64());
    assert!(parsed["available_memory_bytes"].is_u64());
    assert!(parsed["total_swap_bytes"].is_u64());
    assert!(parsed["uptime_seconds"].is_u64());
    assert!(parsed["pid"].is_u64());
    assert!(parsed["boot_time_utc"].is_string());
    assert!(parsed["working_directory"].is_string());
    assert!(parsed["shell"].is_string());
    assert!(parsed["username"].is_string());

    // FR-008: no key leaks file contents or an environment dump.
    assert!(
        !obj.contains_key("env") && !obj.contains_key("environment"),
        "json format must not dump the environment: {parsed}"
    );

    // FR-019: each GPU element carries the documented snake_case key set.
    for gpu in parsed["gpus"].as_array().expect("gpus array") {
        for key in [
            "name",
            "vendor",
            "vendor_id",
            "device_id",
            "driver",
            "kind",
            "vram_bytes",
        ] {
            assert!(
                gpu.get(key).is_some(),
                "GPU entry missing key `{key}`: {gpu}"
            );
        }
        assert!(gpu["name"].is_string());
        assert!(gpu["kind"].is_string());
        assert!(gpu["vram_bytes"].is_u64());
    }

    // FR-022: the physical-hardware identity fields are strings.
    for key in [
        "system_vendor",
        "system_model",
        "system_version",
        "product_family",
        "chassis_type",
        "chassis_vendor",
        "board_vendor",
        "board_name",
        "board_version",
        "bios_vendor",
        "bios_version",
        "bios_date",
    ] {
        assert!(
            parsed[key].is_string(),
            "hardware identity key `{key}` must be a string: {parsed}"
        );
    }

    // FR-023/FR-024: storage and network are always arrays with fixed key sets.
    assert!(
        parsed["storage_devices"].is_array(),
        "storage_devices must always be an array: {parsed}"
    );
    for device in parsed["storage_devices"].as_array().expect("storage array") {
        for key in ["name", "model", "vendor", "size_bytes", "kind"] {
            assert!(
                device.get(key).is_some(),
                "storage entry missing key `{key}`: {device}"
            );
        }
        assert!(device["name"].is_string());
        assert!(device["size_bytes"].is_u64());
    }
    assert!(
        parsed["network_interfaces"].is_array(),
        "network_interfaces must always be an array: {parsed}"
    );
    for iface in parsed["network_interfaces"]
        .as_array()
        .expect("network array")
    {
        for key in [
            "name",
            "mac_address",
            "oper_state",
            "speed_mbps",
            "is_loopback",
        ] {
            assert!(
                iface.get(key).is_some(),
                "network entry missing key `{key}`: {iface}"
            );
        }
        assert!(iface["name"].is_string());
        assert!(iface["is_loopback"].is_boolean());
    }

    // The metadata payload mirrors the JSON content exactly (FR-004, FR-018).
    let meta = output.metadata.expect("metadata present");
    assert_eq!(meta, parsed, "metadata should mirror the json content");
}

#[tokio::test]
async fn test_os_info_invalid_format_errors_clearly() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let err = tool
        .execute(json!({"format": "yaml"}), &ctx)
        .await
        .expect_err("unknown format must fail");
    let message = err.to_string();
    assert!(
        message.contains("yaml"),
        "error should name the offending value: {message}"
    );
    assert!(
        message.contains("text") && message.contains("json"),
        "error should list the accepted values: {message}"
    );

    // A non-string format (e.g. a number from a sloppy provider) must also be
    // rejected rather than silently coerced.
    let type_err = tool
        .execute(json!({"format": 3}), &ctx)
        .await
        .expect_err("non-string format must fail");
    assert!(
        type_err.to_string().contains("format"),
        "type error should mention format: {}",
        type_err
    );
}

#[tokio::test]
async fn test_os_info_missing_and_null_format_default_to_text() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    // Omitted format.
    let omitted = tool.execute(json!({}), &ctx).await.expect("execute ok");
    assert!(omitted.content.contains("## Operating System"));

    // Explicit JSON null format (providers sometimes send this).
    let null = tool
        .execute(json!({"format": null}), &ctx)
        .await
        .expect("execute ok");
    assert!(
        null.content.contains("## Operating System"),
        "null format should default to text: {}",
        null.content
    );
}

#[tokio::test]
async fn test_os_info_gpu_apis_are_platform_scoped() {
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let meta = output.metadata.expect("metadata present");
    let family = meta["os_family"].as_str().expect("os_family string");
    let apis: Vec<String> = meta["gpu_apis"]
        .as_array()
        .expect("gpu_apis array")
        .iter()
        .map(|v| {
            v.get("name")
                .and_then(Value::as_str)
                .expect("api name string")
                .to_string()
        })
        .collect();

    // FR-020: the list is drawn from the fixed vocabulary and reported in the
    // documented order.
    const ORDER: &[&str] = &[
        "Direct3D",
        "DirectX",
        "Metal",
        "OpenGL",
        "OpenGL ES",
        "Mesa",
        "Vulkan",
        "OptiX",
        "CUDA",
        "ROCm",
    ];
    let mut last = 0usize;
    for api in &apis {
        let index = ORDER
            .iter()
            .position(|candidate| candidate == api)
            .unwrap_or_else(|| panic!("unknown graphics API in report: {api}"));
        assert!(index >= last, "graphics APIs must follow the fixed order");
        last = index;
    }

    // Platform scoping: Direct3D/DirectX are Windows-only, Metal is macOS-only.
    if family != "windows" {
        assert!(
            !apis.iter().any(|api| api == "Direct3D" || api == "DirectX"),
            "Direct3D/DirectX must not be reported on {family}: {apis:?}"
        );
    }
    if family != "macos" {
        assert!(
            !apis.contains(&"Metal".to_string()),
            "Metal must not be reported on {family}: {apis:?}"
        );
    }

    // The text and JSON surfaces agree on the API list (FR-021: each entry
    // renders `Name Version` when the version is known, or the bare name).
    if apis.is_empty() {
        assert!(
            output.content.contains("- **APIs**: none detected"),
            "empty API list must render the placeholder: {}",
            output.content
        );
    } else {
        let rendered = meta["gpu_apis"]
            .as_array()
            .expect("gpu_apis array")
            .iter()
            .map(|entry| {
                let name = entry["name"].as_str().expect("api name");
                match entry["version"].as_str().expect("api version") {
                    "unknown" => name.to_string(),
                    version => format!("{name} {version}"),
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        assert!(
            output.content.contains(&rendered),
            "text report must list `{rendered}`: {}",
            output.content
        );
    }
}

#[tokio::test]
async fn test_os_info_gpu_api_versions_are_reported() {
    // FR-021: every reported graphics API carries a version string. The default
    // call probes, so on a host with a Vulkan ICD manifest the version comes
    // from the read-only file and on a host with `glxinfo` it comes from the
    // diagnostic; either way the field must never be empty and the text line
    // must use it.
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");
    let meta = output.metadata.expect("metadata present");
    let apis = meta["gpu_apis"].as_array().expect("gpu_apis array");

    for api in apis {
        let version = api["version"].as_str().expect("api version");
        assert!(
            !version.is_empty(),
            "FR-021: api version must never be empty: {api}"
        );
        // A known version renders next to the name in the text line.
        if version != "unknown" {
            let name = api["name"].as_str().expect("api name");
            assert!(
                output.content.contains(&format!("{name} {version}")),
                "text report must render `{name} {version}`: {}",
                output.content
            );
        }
    }
}

#[tokio::test]
async fn test_os_info_text_reports_hardware_section() {
    // FR-022..FR-024: the text report always renders the `## Hardware` section
    // with the identity bullets and, when devices are detected, the storage and
    // network lists; an empty list renders the `none detected` placeholder.
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool.execute(json!({}), &ctx).await.expect("execute ok");

    assert!(
        output.content.contains("## Hardware"),
        "hardware section must always render: {}",
        output.content
    );
    for label in [
        "**System Vendor**",
        "**System Model**",
        "**Chassis**",
        "**Board Vendor**",
        "**BIOS Vendor**",
    ] {
        assert!(
            output.content.contains(label),
            "hardware label `{label}` missing: {}",
            output.content
        );
    }

    let meta = output.metadata.expect("metadata present");
    let storage = meta["storage_devices"].as_array().expect("storage array");
    if storage.is_empty() {
        assert!(
            output.content.contains("**Storage**: none detected"),
            "empty storage must render the placeholder: {}",
            output.content
        );
    } else {
        assert!(
            output.content.contains("**Storage**:"),
            "storage list header missing: {}",
            output.content
        );
        // FR-023: the device name and kind appear on a per-device bullet.
        let name = storage[0]["name"].as_str().expect("device name");
        let kind = storage[0]["kind"].as_str().expect("device kind");
        assert!(
            output.content.contains(&format!("**{name}**")),
            "storage device `{name}` missing from the report: {}",
            output.content
        );
        assert!(
            output.content.contains(kind),
            "storage kind `{kind}` missing from the report: {}",
            output.content
        );
    }

    let network = meta["network_interfaces"]
        .as_array()
        .expect("network array");
    if network.is_empty() {
        assert!(
            output.content.contains("**Network**: none detected"),
            "empty network must render the placeholder: {}",
            output.content
        );
    } else {
        assert!(
            output.content.contains("**Network**:"),
            "network list header missing: {}",
            output.content
        );
    }
}

#[tokio::test]
async fn test_os_info_hardware_excludes_sensitive_identifiers() {
    // FR-008, FR-025: no serial number, UUID, or asset tag is ever read or
    // rendered, and no such key appears in the JSON payload.
    let registry = create_default_registry();
    let tool = registry.get("os_info").expect("os_info registered");
    let ctx = base_ctx();

    let output = tool
        .execute(json!({"format": "json", "probe": false}), &ctx)
        .await
        .expect("execute ok");

    let parsed: Value = serde_json::from_str(&output.content).expect("valid json");
    for forbidden in ["serial", "uuid", "asset_tag"] {
        assert!(
            !parsed
                .as_object()
                .expect("object")
                .keys()
                .any(|k| k.contains(forbidden)),
            "hardware payload must not carry a `{forbidden}` key: {parsed}"
        );
    }

    let text = tool
        .execute(json!({}), &ctx)
        .await
        .expect("execute ok")
        .content;
    for forbidden in ["Serial", "UUID", "Asset Tag"] {
        assert!(
            !text.contains(forbidden),
            "hardware text must not render `{forbidden}`: {text}"
        );
    }
}
