//! Per-session pull/merge request history (spec SESSIONMENU): every
//! request bound to a session is kept, counted and listed newest first.

use ragent_storage::{SessionRequest, Storage};

fn request(session: &str, number: u64, state: &str) -> SessionRequest {
    SessionRequest {
        session_id: session.to_string(),
        forge: "gitlab".to_string(),
        host: "gitlab.example.com".to_string(),
        repo: "group/app".to_string(),
        number,
        title: format!("MR {number}"),
        web_url: format!("https://gitlab.example.com/group/app/-/merge_requests/{number}"),
        head_branch: format!("b{number}"),
        state: state.to_string(),
    }
}

#[test]
fn test_session_request_history_keeps_every_binding() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("s", "/nonexistent/history")
        .expect("s");
    storage
        .create_session("t", "/nonexistent/history")
        .expect("t");

    storage
        .set_session_request(&request("s", 1, "open"))
        .expect("1");
    std::thread::sleep(std::time::Duration::from_millis(5));
    storage
        .set_session_request(&request("s", 2, "open"))
        .expect("2");
    // Rebinding the same request updates it, it is not counted twice.
    storage
        .set_session_request(&request("s", 2, "merged"))
        .expect("2 again");
    storage
        .set_session_request(&request("t", 9, "open"))
        .expect("9");
    storage.clear_session_request("s").expect("unbind");

    let history = storage.session_request_history("s").expect("history");
    let seen: Vec<(u64, &str)> = history
        .iter()
        .map(|r| (r.number, r.state.as_str()))
        .collect();
    assert_eq!(seen, [(2, "merged"), (1, "open")]);
    let totals = storage.session_request_totals().expect("totals");
    assert_eq!(totals.get("s"), Some(&2));
    assert_eq!(totals.get("t"), Some(&1));
}

#[test]
fn test_session_request_history_goes_with_a_deleted_session() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_session("s", "/nonexistent/history")
        .expect("s");
    storage
        .set_session_request(&request("s", 1, "open"))
        .expect("1");

    storage.delete_session("s").expect("delete");

    assert_eq!(
        storage.session_request_history("s").expect("history"),
        Vec::new()
    );
    assert!(storage.session_request_totals().expect("totals").is_empty());
}
