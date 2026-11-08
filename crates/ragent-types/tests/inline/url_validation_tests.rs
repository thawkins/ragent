use super::*;

#[test]
fn test_https_remote_is_accepted_and_normalised() {
    assert_eq!(
        validate_instance_url("  https://gitlab.example.com/api/v4/  ").unwrap(),
        "https://gitlab.example.com/api/v4"
    );
    assert_eq!(
        validate_instance_url("https://gitlab.com").unwrap(),
        "https://gitlab.com"
    );
}

#[test]
fn test_http_is_refused_for_a_remote_host() {
    let err = validate_instance_url("http://gitlab.example.com").unwrap_err();
    assert!(matches!(err, InstanceUrlError::InsecureScheme(_)));
    // The message names the fix, not just the failure.
    assert!(err.to_string().contains("https"));
}

#[test]
fn test_http_is_allowed_for_loopback() {
    for url in [
        "http://localhost:8080",
        "http://127.0.0.1:3000",
        "http://[::1]:8080",
        "http://gitlab.localhost",
    ] {
        assert!(
            validate_instance_url(url).is_ok(),
            "{url} should be accepted"
        );
    }
}

#[test]
fn test_other_schemes_are_refused() {
    for url in [
        "ftp://example.com",
        "file:///etc/passwd",
        "git://example.com",
    ] {
        assert!(
            matches!(
                validate_instance_url(url),
                Err(InstanceUrlError::UnsupportedScheme(_))
            ),
            "{url} should be refused"
        );
    }
}

#[test]
fn test_userinfo_is_refused() {
    assert_eq!(
        validate_instance_url("https://user:pw@gitlab.example.com"),
        Err(InstanceUrlError::CredentialsInUrl)
    );
    assert_eq!(
        validate_instance_url("https://token@gitlab.example.com"),
        Err(InstanceUrlError::CredentialsInUrl)
    );
}

#[test]
fn test_empty_and_relative_are_refused() {
    assert_eq!(validate_instance_url(""), Err(InstanceUrlError::Empty));
    assert_eq!(validate_instance_url("   "), Err(InstanceUrlError::Empty));
    assert!(matches!(
        validate_instance_url("gitlab.example.com"),
        Err(InstanceUrlError::NotAbsolute(_))
    ));
    assert!(matches!(
        validate_instance_url("/api/v4"),
        Err(InstanceUrlError::NotAbsolute(_))
    ));
}

#[test]
fn test_query_and_fragment_are_rejected_from_the_base() {
    assert_eq!(
        validate_instance_url("https://gitlab.example.com/base?x=1#frag").unwrap(),
        "https://gitlab.example.com/base"
    );
}

#[test]
fn test_instance_scheme_classification() {
    assert_eq!(instance_scheme("https://gitlab.com"), InstanceScheme::Https);
    assert_eq!(
        instance_scheme("http://localhost:8080"),
        InstanceScheme::HttpLoopback
    );
}
