# Web source

- URL: https://rust-training.ferrous-systems.com/latest/book/drop-panic-abort
- Title: ## Keyboard shortcuts
- Captured (UTC): 2026-06-29T16:20:25.573174253+00:00

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

# Rust Training Slides by Ferrous Systems

[ ][1]

# [Drop, panic, and abort][2]

What happens in detail when values drop?

## [Drop-Order][3]

Rust generally guarantees drop order ([RFC1857][4])

## [Drop-Order][5]
* Values are dropped at the end of their scope
* The order is *the reverse introduction order*
* Unbound values drop immediately
* Structure fields are dropped *first to last*

## [Destructors][6]

Sometimes, certain actions must be taken before deallocation.

For this, the `Drop` trait can be implemented.

`struct LevelDB {
    handle: *mut leveldb_database_t
}

impl Drop for LevelDB {
    fn drop(&mut self) {
        unsafe { leveldb_close(self.handle) };
    }
}`

## [Warning!][7]

Destructors cannot return errors.

## [Also possible][8]

Explicit destruction of a value through a consuming function. This cannot be statically enforced currently.

Implementing a `Drop`-bomb (a failing destructor) can make sure this error is caught early.

## [Panics][9]

Rust also has another error mechanism: `panic!`

`fn main() {
    panicking_function();
}

fn panicking_function() {
    panic!("gosh, don't call me!");
}`

In case of a panic, the following happens:
* The current thread immediately halts
* The stack is unwound
* All affected values are dropped and their destructors run

Panics are implementation-wise similar to C++-Exceptions, but should only be used for fatal errors. They cannot be
(normally) caught.

The affected thread dies.

## [Catching Panics][10]

Panicking across FFI-boundaries is undefined behaviour. In these cases, panics *must* be caught. For cases like this,
there are [std::panic::catch-unwind][11] and [std::panic::resume-unwind][12].

## [Hooks][13]

[std::panic::set_hook][14] allows setting a global handler that is run *before* the unwinding happens.

In general, `Result` is always the right way to propagate errors if they are to be handled.

## [Abort][15]

In some environments, unwinding on `panic!` is not very meaningful. For those cases, `rustc` and `cargo` have a switch
that immediately aborts the program on panic.

The panic hook is executed.

## [Double-panics][16]

Panicking while a panic is being handled - for example in a destructor - invokes undefined behaviour. For that reason,
the program will immediately abort.

[ ][17] [ ][18]
[ ][19] [ ][20]

[1]: print.html
[2]: #drop-panic-and-abort
[3]: #drop-order
[4]: https://github.com/rust-lang/rfcs/issues/1857
[5]: #drop-order-1
[6]: #destructors
[7]: #warning
[8]: #also-possible
[9]: #panics
[10]: #catching-panics
[11]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[12]: https://doc.rust-lang.org/std/panic/fn.resume_unwind.html
[13]: #hooks
[14]: https://doc.rust-lang.org/std/panic/fn.set_hook.html
[15]: #abort
[16]: #double-panics
[17]: documentation.html
[18]: dynamic-dispatch.html
[19]: documentation.html
[20]: dynamic-dispatch.html
```
