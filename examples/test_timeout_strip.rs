/// Test the strip_timeout_prefix and split_bash_command logic
///
/// Run with: cargo run --example test_timeout_strip

fn strip_timeout_prefix(command: &str) -> &str {
    let trimmed = command.trim();

    // Check if the command starts with "timeout"
    if let Some(rest) = trimmed.strip_prefix("timeout") {
        // Must be followed by whitespace
        if rest.starts_with(char::is_whitespace) {
            let rest = rest.trim_start();

            // Next token should be a number (the timeout value)
            if let Some(space_pos) = rest.find(char::is_whitespace) {
                let potential_number = &rest[..space_pos];
                if potential_number.chars().all(|c| c.is_ascii_digit()) {
                    // Found "timeout [nnn] ...", return the rest after the number
                    return rest[space_pos..].trim_start();
                }
            }
        }
    }

    // No timeout prefix found, return original
    trimmed
}

fn split_bash_command(command: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
                current.push(c);
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
                current.push(c);
            }
            '&' | '|' | ';' if !in_single_quote && !in_double_quote => {
                // Check for && or ||
                if (c == '&' || c == '|') && chars.peek() == Some(&c) {
                    chars.next(); // consume the second character
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        // Strip timeout prefix before adding to parts
                        parts.push(strip_timeout_prefix(trimmed).to_string());
                    }
                    current.clear();
                } else if c == ';' {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        // Strip timeout prefix before adding to parts
                        parts.push(strip_timeout_prefix(trimmed).to_string());
                    }
                    current.clear();
                } else {
                    // Single & or | - add to current command
                    current.push(c);
                }
            }
            _ => current.push(c),
        }
    }

    // Add the final part
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        // Strip timeout prefix before adding to parts
        parts.push(strip_timeout_prefix(trimmed).to_string());
    }

    // If no delimiters found, return the original command (with timeout stripped)
    if parts.is_empty() {
        vec![strip_timeout_prefix(command).to_string()]
    } else {
        parts
    }
}

fn main() {
    let test_cases = vec![
        ("timeout 600 cargo build", vec!["cargo build"]),
        ("timeout 10 ls -la", vec!["ls -la"]),
        ("cargo test", vec!["cargo test"]),
        (
            "timeout 600 cargo build && cargo test",
            vec!["cargo build", "cargo test"],
        ),
        (
            "timeout 10 ls && timeout 20 cat file",
            vec!["ls", "cat file"],
        ),
        (
            "ls && timeout 30 pwd && echo done",
            vec!["ls", "pwd", "echo done"],
        ),
        ("timeout 5 echo 'hello'", vec!["echo 'hello'"]),
        ("timeout_tool --flag", vec!["timeout_tool --flag"]),
        ("TIMEOUT 600 cargo build", vec!["TIMEOUT 600 cargo build"]),
        ("  timeout 600  cargo build  ", vec!["cargo build"]),
    ];

    println!("Testing strip_timeout_prefix and split_bash_command:\n");

    for (input, expected) in test_cases {
        let result = split_bash_command(input);
        let passed = result == expected;

        println!("Input:    {:?}", input);
        println!("Expected: {:?}", expected);
        println!("Got:      {:?}", result);
        println!("Status:   {}\n", if passed { "✓ PASS" } else { "✗ FAIL" });
    }
}
