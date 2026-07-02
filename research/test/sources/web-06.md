# Web source

- URL: https://google.github.io/comprehensive-rust/error-handling/panics.html
- Title: ## Keyboard shortcuts
- Captured (UTC): 2026-06-29T16:20:17.784861008+00:00

```text
## Keyboard shortcuts

Press ← or → to navigate between chapters

Press S or / to search in the book

Press ? to show this help

Press Esc to hide this help
* Auto
* Light
* Rust
* Coal
* Navy
* Ayu

# Comprehensive Rust 🦀
* English
* Brazilian Portuguese (Português do Brasil)
* Chinese Simplified (汉语)
* Chinese Traditional (漢語)
* Japanese (日本語)
* Korean (한국어)
* Farsi (فارسی)
* Spanish (Español)
* Ukrainian (українська)

[ ][1] [ ][2] [ ][3]

# [Panics][4]

In case of a fatal runtime error, Rust triggers a “panic”:

`// Copyright 2022 Google LLC
// SPDX-License-Identifier: Apache-2.0

fn main() {
    let v = vec![10, 20, 30];
    dbg!(v[100]);
}`
* Panics are for unrecoverable and unexpected errors.
  * Panics are symptoms of bugs in the program.
  * Runtime failures like failed bounds checks can panic.
  * Assertions (such as `assert!`) panic on failure.
  * Purpose-specific panics can use the `panic!` macro.
* A panic will “unwind” the stack, dropping values just as if the functions had returned.
* Use non-panicking APIs (such as `Vec::get`) if crashing is not acceptable.

This slide should take about 3 minutes.

By default, a panic will cause the stack to unwind. The unwinding can be caught:

`// Copyright 2022 Google LLC
// SPDX-License-Identifier: Apache-2.0

use std::panic;

fn main() {
    let result = panic::catch_unwind(|| "No problem here!");
    dbg!(result);

    let result = panic::catch_unwind(|| {
        panic!("oh no!");
    });
    dbg!(result);
}`
* Catching is unusual; do not attempt to implement exceptions with `catch_unwind`!
* This can be useful in servers which should keep running even if a single request crashes.
* This does not work if `panic = 'abort'` is set in your `Cargo.toml`.

[ ][5] [ ][6]
[ ][7] [ ][8]

[1]: ../print.html
[2]: https://github.com/google/comprehensive-rust
[3]: https://github.com/google/comprehensive-rust/edit/main/src/error-handling/panics.md
[4]: #panics
[5]: ../error-handling.html
[6]: ../error-handling/result.html
[7]: ../error-handling.html
[8]: ../error-handling/result.html
```
