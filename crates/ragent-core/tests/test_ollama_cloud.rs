//! Tests for Ollama Cloud provider.
#![allow(missing_docs)]

use ragent_core::provider::ollama_cloud::list_ollama_cloud_models;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_ollama_cloud_model_listing_uses_bearer_token() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 4096];
        let n = socket.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..n]).to_lowercase();
        assert!(request.contains("get /api/tags http/1.1"));
        assert!(request.contains("authorization: bearer test-token"));

        let body =
            r#"{"models":[{"name":"gpt-oss:120b-cloud","details":{"parameter_size":"120B"}}]}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });

    let base_url = format!("http://{}", addr);
    let models = list_ollama_cloud_models("test-token", Some(&base_url))
        .await
        .unwrap();

    server.await.unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].provider_id, "ollama_cloud");
    assert_eq!(models[0].id, "gpt-oss:120b-cloud");
    assert_eq!(models[0].name, "gpt-oss:120b-cloud (120B)");
}
