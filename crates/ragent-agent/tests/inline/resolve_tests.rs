//! Inline tests for `resolve.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

/// Return a unique scratch directory under the system temp dir.
///
/// Each test gets its own directory so the tests are safe to run in
/// parallel; a shared fixed name caused spurious failures when the
/// workspace suite ran both this crate's lib and integration tests at once.
fn unique_tmp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ragent_test_{tag}_{}", uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn test_resolve_file() {
    let tmp = unique_tmp("resolve_file");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    std::fs::write(tmp.join("hello.txt"), "Hello\nWorld\n").expect("write");

    let parsed = ParsedRef {
        raw: "hello.txt".to_string(),
        kind: FileRef::File(PathBuf::from("hello.txt")),
        span: 0..10,
    };

    let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
    assert!(resolved.content.contains("Hello"));
    assert!(resolved.content.contains("World"));
    assert!(!resolved.truncated);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_resolve_directory() {
    let tmp = unique_tmp("resolve_dir");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("subdir")).expect("mkdir");
    std::fs::write(tmp.join("subdir/file.txt"), "content").expect("write");

    let parsed = ParsedRef {
        raw: "subdir/".to_string(),
        kind: FileRef::Directory(PathBuf::from("subdir")),
        span: 0..8,
    };

    let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
    assert!(resolved.content.contains("file.txt"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_resolve_fuzzy() {
    let tmp = unique_tmp("resolve_fuzzy");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("src")).expect("mkdir");
    std::fs::write(tmp.join("src/main.rs"), "fn main() {}").expect("write");

    let parsed = ParsedRef {
        raw: "main".to_string(),
        kind: FileRef::Fuzzy("main".to_string()),
        span: 0..5,
    };

    let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve");
    assert!(resolved.content.contains("fn main()"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_resolve_nonexistent_file() {
    let tmp = unique_tmp("resolve_nofile");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");

    let parsed = ParsedRef {
        raw: "nope.txt".to_string(),
        kind: FileRef::File(PathBuf::from("nope.txt")),
        span: 0..9,
    };

    assert!(resolve_ref(&parsed, &tmp).await.is_err());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn test_resolve_url_uses_shared_client() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;
        let response =
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 11\r\n\r\nhello world";
        socket.write_all(response.as_bytes()).await.expect("write");
    });

    let url = format!("http://127.0.0.1:{port}/");
    let parsed = ParsedRef {
        raw: url.clone(),
        kind: FileRef::Url(url),
        span: 0..10,
    };

    let tmp = std::env::temp_dir();
    let resolved = resolve_ref(&parsed, &tmp).await.expect("resolve url");
    assert!(resolved.content.contains("hello world"));
    assert!(!resolved.truncated);

    server.await.expect("server task");
}

#[tokio::test]
async fn test_resolve_all_refs_no_refs() {
    let tmp = std::env::temp_dir();
    let (text, resolved) = resolve_all_refs("plain text", &tmp).await.expect("resolve");
    assert_eq!(text, "plain text");
    assert!(resolved.is_empty());
}

#[tokio::test]
async fn test_resolve_all_refs_with_file() {
    let tmp = unique_tmp("resolve_all");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");
    std::fs::write(tmp.join("data.txt"), "line1\nline2\n").expect("write");

    let (text, resolved) = resolve_all_refs("Check @data.txt please", &tmp)
        .await
        .expect("resolve");

    assert_eq!(resolved.len(), 1);
    assert!(text.contains("<referenced_files>"));
    assert!(text.contains("line1"));
    assert!(text.contains("</file>"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_truncate_content_small() {
    let (content, truncated) = truncate_content("hello".to_string());
    assert_eq!(content, "hello");
    assert!(!truncated);
}

#[test]
fn test_truncate_content_large() {
    let large = "x".repeat(MAX_CONTENT_SIZE + 100);
    let (content, truncated) = truncate_content(large);
    assert!(truncated);
    assert!(content.contains("[Content truncated"));
    assert!(content.len() <= MAX_CONTENT_SIZE + 100);
}
