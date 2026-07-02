# Web source

- URL: https://doc.rust-lang.org/rust-by-example/std/panic.html
- Title: ## Keyboard shortcuts
- Captured (UTC): 2026-06-29T16:20:19.584044341+00:00

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

# Rust By Example

[ ][1] [ ][2] [ ][3]

# [`panic!`][4]

The `panic!` macro can be used to generate a panic and start unwinding its stack. While unwinding, the runtime will take
care of freeing all the resources *owned* by the thread by calling the destructor of all its objects.

Since we are dealing with programs with only one thread, `panic!` will cause the program to report the panic message and
exit.

`// Re-implementation of integer division (/)
fn division(dividend: i32, divisor: i32) -> i32 {
    if divisor == 0 {
        // Division by zero triggers a panic
        panic!("division by zero");
    } else {
        dividend / divisor
    }
}

// The `main` task
fn main() {
    // Heap allocated integer
    let _x = Box::new(0i32);

    // This operation will trigger a task failure
    division(3, 0);

    println!("This point won't be reached!");

    // `_x` should get destroyed at this point
}`

Let’s check that `panic!` doesn’t leak memory.

`$ rustc panic.rs && valgrind ./panic
==4401== Memcheck, a memory error detector
==4401== Copyright (C) 2002-2013, and GNU GPL'd, by Julian Seward et al.
==4401== Using Valgrind-3.10.0.SVN and LibVEX; rerun with -h for copyright info
==4401== Command: ./panic
==4401==
thread '<main>' panicked at 'division by zero', panic.rs:5
==4401==
==4401== HEAP SUMMARY:
==4401==     in use at exit: 0 bytes in 0 blocks
==4401==   total heap usage: 18 allocs, 18 frees, 1,648 bytes allocated
==4401==
==4401== All heap blocks were freed -- no leaks are possible
==4401==
==4401== For counts of detected and suppressed errors, rerun with: -v
==4401== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
`

[ ][5] [ ][6]
[ ][7] [ ][8]

[1]: ../print.html
[2]: https://github.com/rust-lang/rust-by-example
[3]: https://github.com/rust-lang/rust-by-example/edit/master/src/std/panic.md
[4]: #panic
[5]: ../std/result/question_mark.html
[6]: ../std/hash.html
[7]: ../std/result/question_mark.html
[8]: ../std/hash.html
```
