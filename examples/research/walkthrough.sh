#!/usr/bin/env bash
# Demonstrates the research → spec → implement workflow on a tiny fixture
# project. Each command below is the same one a user would type in the TUI
# or run from the CLI.
set -euo pipefail

cd "$(dirname "$0")"

echo "=== 1. Create a research item from the CLI ==="
"$(dirname "$0")/../../target/debug/ragent" research create rust-async "async/await idioms in stable Rust"

echo "=== 2. List research items ==="
"$(dirname "$0")/../../target/debug/ragent" research list

echo "=== 3. Show item metadata ==="
"$(dirname "$0")/../../target/debug/ragent" research show rust-async

echo "=== 4. Search across all research items ==="
"$(dirname "$0")/../../target/debug/ragent" research search async

echo "=== 5. Archive the item (excluded from default list) ==="
"$(dirname "$0")/../../target/debug/ragent" research archive rust-async
"$(dirname "$0")/../../target/debug/ragent" research list

echo "=== 6. Cleanup ==="
"$(dirname "$0")/../../target/debug/ragent" research delete rust-async --yes
