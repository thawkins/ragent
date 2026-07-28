fn main() {
    let prefix = "x".repeat(4999);
    let html = format!("{prefix}…").to_ascii_lowercase();
    let _ = &html[..html.len().min(5000)];
}