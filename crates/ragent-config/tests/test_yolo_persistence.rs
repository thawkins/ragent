//! Tests for YOLO mode persistence.

use std::io::Write;

use ragent_config::Config;

fn write_config(dir: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let ragent_dir = dir.join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent dir");
    let path = ragent_dir.join("ragent.json");
    let mut file = std::fs::File::create(&path).expect("create config file");
    file.write_all(contents.as_bytes())
        .expect("write config file");
    path
}

#[test]
fn test_yolo_defaults_to_false() {
    let config = Config::default();
    assert!(!config.yolo);
}

#[test]
#[serial_test::serial]
fn test_yolo_round_trips_through_config() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = write_config(temp.path(), r#"{ "yolo": true }"#);

    unsafe { std::env::set_var("RAGENT_CONFIG", &path) };
    let config = Config::load().expect("load config");

    assert!(config.yolo);
    ragent_config::yolo::sync_from_config();
    assert!(ragent_config::yolo::is_enabled());
}

#[test]
#[serial_test::serial]
fn test_yolo_persist_helper_updates_config_file() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = write_config(temp.path(), r#"{ "yolo": false }"#);

    unsafe { std::env::set_var("RAGENT_CONFIG", &path) };
    ragent_config::yolo::persist_yolo(true).expect("persist yolo");

    // Reloading should pick up the persisted value.
    let config = Config::load().expect("reload config");
    assert!(config.yolo);
    assert!(ragent_config::yolo::is_enabled());
}

#[test]
fn test_yolo_false_is_serialized() {
    let mut config = Config::default();
    config.yolo = false;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(!json.contains("\"yolo\": true"));
}

#[test]
fn test_yolo_true_is_serialized() {
    let mut config = Config::default();
    config.yolo = true;
    let json = serde_json::to_string_pretty(&config).expect("serialize config");
    assert!(json.contains("\"yolo\": true"));
}
