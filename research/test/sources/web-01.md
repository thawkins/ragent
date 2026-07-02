# Web source

- URL: https://doc.rust-lang.org/std/macro.panic.html
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T16:20:10.509868993+00:00

```text
[Skip to main content][1]

## [panic][2]

[[logo]][3]

## [std][4]1.96.0

(ac68faa20 2026-05-25)

## [panic][5]

### [Sections][6]
* [When to use `panic!` vs `Result`][7]
* [Current implementation][8]
* [Editions][9]
  * [2021 and later][10]
  * [2018 and 2015][11]
* [Examples][12]

## [In crate std][13]

[std][14]

# Macro panic Copy item path

1.0.0 · [Source][15]

`macro_rules! panic {
    ($($arg:tt)*) => { ... };
}`

Expand description

Panics the current thread.

This allows a program to terminate immediately and provide feedback to the caller of the program.

This macro is the perfect way to assert conditions in example code and in tests. `panic!` is closely tied with the
`unwrap` method of both [`Option`][16] and [`Result`][17] enums. Both implementations call `panic!` when they are set to
[`None`][18] or [`Err`][19] variants.

When using `panic!()` you can specify a string payload that is built using [formatting syntax][20]. That payload is used
when injecting the panic into the calling Rust thread, causing the thread to panic entirely.

The behavior of the default `std` hook, i.e. the code that runs directly after the panic is invoked, is to print the
message payload to `stderr` along with the file/line/column information of the `panic!()` call. You can override the
panic hook using [`std::panic::set_hook()`][21]. Inside the hook a panic can be accessed as a `&dyn Any + Send`, which
contains either a `&str` or `String` for regular `panic!()` invocations. (Whether a particular invocation contains the
payload at type `&str` or `String` is unspecified and can change.) To panic with a value of another other type,
[`panic_any`][22] can be used.

See also the macro [`compile_error!`][23], for raising errors during compilation.

## [§][24]When to use `panic!` vs `Result`

The Rust language provides two complementary systems for constructing / representing, reporting, propagating, reacting
to, and discarding errors. These responsibilities are collectively known as “error handling.” `panic!` and `Result` are
similar in that they are each the primary interface of their respective error handling systems; however, the meaning
these interfaces attach to their errors and the responsibilities they fulfill within their respective error handling
systems differ.

The `panic!` macro is used to construct errors that represent a bug that has been detected in your program. With
`panic!` you provide a message that describes the bug and the language then constructs an error with that message,
reports it, and propagates it for you.

`Result` on the other hand is used to wrap other types that represent either the successful result of some computation,
`Ok(T)`, or error types that represent an anticipated runtime failure mode of that computation, `Err(E)`. `Result` is
used alongside user defined types which represent the various anticipated runtime failure modes that the associated
computation could encounter. `Result` must be propagated manually, often with the help of the `?` operator and `Try`
trait, and they must be reported manually, often with the help of the `Error` trait.

For more detailed information about error handling check out the [book][25] or the [`std::result`][26] module docs.

## [§][27]Current implementation

If the main thread panics it will terminate all your threads and end your program with code `101`.

## [§][28]Editions

Behavior of the panic macros changed over editions.

### [§][29]2021 and later

In Rust 2021 and later, `panic!` always requires a format string and the applicable format arguments, and is the same in
`core` and `std`. Use [`std::panic::panic_any(x)`][30] to panic with an arbitrary payload.

### [§][31]2018 and 2015

In Rust Editions prior to 2021, `std::panic!(x)` with a single argument directly uses that argument as a payload. This
is true even if the argument is a string literal. For example, `panic!("problem: {reason}")` panics with a payload of
literally `"problem: {reason}"` (a `&'static str`).

`core::panic!(x)` with a single argument requires that `x` be `&str`, but otherwise behaves like `std::panic!`. In
particular, the string need not be a literal, and is not interpreted as a format string.

## [§][32]Examples

[ⓘ][33]

`panic!();
panic!("this is a terrible mistake!");
panic!("this is a {} {message}", "fancy", message = "message");
std::panic::panic_any(4); // panic with the value of 4 to be collected elsewhere`

[1]: #main-content
[2]: #
[3]: ../std/index.html
[4]: ../std/index.html
[5]: #
[6]: #
[7]: #when-to-use-panic-vs-result
[8]: #current-implementation
[9]: #editions
[10]: #2021-and-later
[11]: #2018-and-2015
[12]: #examples
[13]: index.html
[14]: index.html
[15]: ../src/std/macros.rs.html#17-23
[16]: option/enum.Option.html#method.unwrap
[17]: result/enum.Result.html#method.unwrap
[18]: option/enum.Option.html#variant.None
[19]: result/enum.Result.html#variant.Err
[20]: ../std/fmt/index.html
[21]: ../std/panic/fn.set_hook.html
[22]: ../std/panic/fn.panic_any.html
[23]: macro.compile_error.html
[24]: #when-to-use-panic-vs-result
[25]: ../book/ch09-00-error-handling.html
[26]: ../std/result/index.html
[27]: #current-implementation
[28]: #editions
[29]: #2021-and-later
[30]: ../std/panic/fn.panic_any.html
[31]: #2018-and-2015
[32]: #examples
[33]: #
```
