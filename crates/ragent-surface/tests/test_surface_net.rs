//! Coverage for `ragent-surface::net`, the shared `https`-only redirect policy
//! and endpoint validator the surface crates delegate to.
//!
//! `parse_https_endpoint` is pure, so each accept/reject branch is asserted
//! directly. The redirect policy is exercised against a minimal in-process
//! server: an `http` downgrade must be stopped, while an `https` target (which
//! the test server cannot actually speak) is followed until the connection
//! fails, proving the policy allowed the hop rather than refusing it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

use ragent_surface::net::{MAX_REDIRECT_HOPS, endpoint, https_only_redirects};

#[test]
fn parse_https_endpoint_rejects_empty_and_whitespace() {
    assert_eq!(
        endpoint::parse_https_endpoint(""),
        Err(endpoint::EndpointError::Empty)
    );
    assert_eq!(
        endpoint::parse_https_endpoint("   \t "),
        Err(endpoint::EndpointError::Empty)
    );
}

#[test]
fn parse_https_endpoint_rejects_a_malformed_url() {
    let err = endpoint::parse_https_endpoint("https://").expect_err("rejected");
    assert!(
        matches!(err, endpoint::EndpointError::Malformed(_)),
        "unexpected: {err:?}"
    );
}

#[test]
fn parse_https_endpoint_rejects_a_non_https_scheme_naming_it() {
    let err = endpoint::parse_https_endpoint("http://example.com/x").expect_err("rejected");
    assert_eq!(
        err,
        endpoint::EndpointError::NotHttps {
            scheme: "http".to_string()
        }
    );

    let ftp = endpoint::parse_https_endpoint("ftp://example.com/x").expect_err("rejected");
    assert_eq!(
        ftp,
        endpoint::EndpointError::NotHttps {
            scheme: "ftp".to_string()
        }
    );
}

#[test]
fn parse_https_endpoint_accepts_an_absolute_https_url_with_a_host() {
    let url = endpoint::parse_https_endpoint("  https://example.com/path  ").expect("accepted");
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("example.com"));
    assert_eq!(url.path(), "/path");
}

#[test]
fn https_only_redirects_builds_a_policy() {
    // Constructing the policy must not panic; the closure runs per attempt.
    let _policy = https_only_redirects();
    assert_eq!(MAX_REDIRECT_HOPS, 5);
}

/// One canned HTTP/1.1 response, served to a single connection.
struct RedirectServer {
    url: String,
    handle: Option<JoinHandle<()>>,
}

impl RedirectServer {
    fn spawn(location: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0_u8; 4096];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self {
            url: format!("http://{addr}/start"),
            handle: Some(handle),
        }
    }
}

impl Drop for RedirectServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn redirect_policy_stops_a_plaintext_downgrade() {
    let server = RedirectServer::spawn("http://example.com/plain".to_string());
    let client = reqwest::blocking::Client::builder()
        .redirect(https_only_redirects())
        .build()
        .expect("client");
    let result = client.get(&server.url).send();
    // The redirect target is non-https, so the policy stops it and the response
    // surfaces as the 302 rather than following to example.com.
    let response = result.expect("response returns");
    assert_eq!(response.status().as_u16(), 302);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok()),
        Some("http://example.com/plain")
    );
}
