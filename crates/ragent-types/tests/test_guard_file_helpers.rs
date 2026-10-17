//! Coverage for the owner-only file-creation guards and the remaining
//! `guard` helpers that the existing shared-guard tests do not reach
//! (`open_owner_only`, `open_new_owner_only`).

use ragent_types::guard::{open_new_owner_only, open_owner_only};

#[test]
fn open_owner_only_creates_and_truncates_a_writable_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("owner.json");

    let mut file = open_owner_only(&path).expect("first create");
    use std::io::Write as _;
    file.write_all(b"first").expect("write");
    drop(file);
    assert_eq!(std::fs::read(&path).expect("read"), b"first");

    // A second open truncates the existing file.
    let mut file = open_owner_only(&path).expect("reopen");
    file.write_all(b"second").expect("write");
    drop(file);
    assert_eq!(std::fs::read(&path).expect("read"), b"second");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "file must be owner-only");
    }
}

#[test]
fn open_new_owner_only_refuses_an_existing_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("fresh.json");

    let mut file = open_new_owner_only(&path).expect("first create");
    use std::io::Write as _;
    file.write_all(b"x").expect("write");
    drop(file);

    // The second attempt must fail with AlreadyExists (O_EXCL semantics).
    let err = open_new_owner_only(&path).expect_err("existing path must be refused");
    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "file must be owner-only");
    }
}

#[test]
fn open_owner_only_reports_the_underlying_io_error_for_a_missing_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("no-such-subdir").join("file.json");
    let err = open_owner_only(&path).expect_err("missing parent directory must fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
