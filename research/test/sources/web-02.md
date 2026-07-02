# Web source

- URL: https://effective-rust.com/panic.html
- Title: 1.  [Effective Rust][1]
- Captured (UTC): 2026-06-29T16:20:12.174036446+00:00

```text
1.  [Effective Rust][1]
2.  [Preface][2]
3.  [**1.** Types][3]
4.  1. [**1.1.** Item 1: Use the type system to express your data structures][4]
    2. [**1.2.** Item 2: Use the type system to express common behavior][5]
    3. [**1.3.** Item 3: Prefer Option and Result transforms over explicit match expressions][6]
    4. [**1.4.** Item 4: Prefer idiomatic Error types][7]
    5. [**1.5.** Item 5: Understand type conversions][8]
    6. [**1.6.** Item 6: Embrace the newtype pattern][9]
    7. [**1.7.** Item 7: Use builders for complex types][10]
    8. [**1.8.** Item 8: Familiarize yourself with reference and pointer types][11]
    9. [**1.9.** Item 9: Consider using iterator transforms instead of explicit loops][12]
5.  [**2.** Traits][13]
6.  1. [**2.1.** Item 10: Familiarize yourself with standard traits][14]
    2. [**2.2.** Item 11: Implement the Drop trait for RAII patterns][15]
    3. [**2.3.** Item 12: Understand the trade-offs between generics and trait objects][16]
    4. [**2.4.** Item 13: Use default implementations to minimize required trait methods][17]
7.  [**3.** Concepts][18]
8.  1. [**3.1.** Item 14: Understand lifetimes][19]
    2. [**3.2.** Item 15: Understand the borrow checker][20]
    3. [**3.3.** Item 16: Avoid writing unsafe code][21]
    4. [**3.4.** Item 17: Be wary of shared-state parallelism][22]
    5. [**3.5.** Item 18: Don't panic][23]
    6. [**3.6.** Item 19: Avoid reflection][24]
    7. [**3.7.** Item 20: Avoid the temptation to over-optimize][25]
9.  [**4.** Dependencies][26]
10. 1. [**4.1.** Item 21: Understand what semantic versioning promises][27]
    2. [**4.2.** Item 22: Minimize visibility][28]
    3. [**4.3.** Item 23: Avoid wildcard imports][29]
    4. [**4.4.** Item 24: Re-export dependencies whose types appear in your API][30]
    5. [**4.5.** Item 25: Manage your dependency graph][31]
    6. [**4.6.** Item 26: Be wary of feature creep][32]
11. [**5.** Tooling][33]
12. 1. [**5.1.** Item 27: Document public interfaces][34]
    2. [**5.2.** Item 28: Use macros judiciously][35]
    3. [**5.3.** Item 29: Listen to Clippy][36]
    4. [**5.4.** Item 30: Write more than unit tests][37]
    5. [**5.5.** Item 31: Take advantage of the tooling ecosystem][38]
    6. [**5.6.** Item 32: Set up a continuous integration (CI) system][39]
13. [**6.** Beyond Standard Rust][40]
14. 1. [**6.1.** Item 33: Consider making library code no_std compatible][41]
    2. [**6.2.** Item 34: Control what crosses FFI boundaries][42]
    3. [**6.3.** Item 35: Prefer bindgen to manual FFI mappings][43]
15. [Afterword][44]
16. [Index][45]
* Light
* Rust
* Coal
* Navy
* Ayu

# Effective Rust

[ ][46]

# [Item 18: Don't panic][47]


> "It looked insanely complicated, and this was one of the reasons why the snug plastic cover it fitted into had the
> words DON’T PANIC printed on it in large friendly letters." – Douglas Adams

The title of this Item would be more accurately described as **prefer returning a `Result` to using `panic!`** (but
**don't panic** is much catchier).

Rust's panic mechanism is primarily designed for unrecoverable bugs in your program, and *by default* it terminates the
thread that issues the `panic!`. However, there are alternatives to this default.

In particular, newcomers to Rust who have come from languages that have an exception system (such as Java or C++)
sometimes pounce on [`std::panic::catch_unwind`][48] as a way to simulate exceptions, because it appears to provide a
mechanism for catching panics at a point further up the call stack.

Consider a function that panics on an invalid input:


`#![allow(unused)]
fn main() {
fn divide(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("Cowardly refusing to divide by zero!");
    }
    a / b
}
}`

Trying to invoke this with an invalid input fails as expected:

`// Attempt to discover what 0/0 is...
let result = divide(0, 0);`

`thread 'main' panicked at 'Cowardly refusing to divide by zero!', main.rs:11:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
`

A wrapper that uses `catch_unwind` to catch the panic:

`fn divide_recover(a: i64, b: i64, default: i64) -> i64 {
    let result = std::panic::catch_unwind(|| divide(a, b));
    match result {
        Ok(x) => x,
        Err(_) => default,
    }
}`

*appears* to work and to simulate `catch`:

`let result = divide_recover(0, 0, 42);
println!("result = {result}");`

`result = 42
`

Appearances can be deceptive, however. The first problem with this approach is that panics don't always unwind; there is
a [compiler option][49] (which is also accessible via a *Cargo.toml* [profile setting][50]) that shifts panic behavior
so that it immediately aborts the process:

`thread 'main' panicked at 'Cowardly refusing to divide by zero!', main.rs:11:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
/bin/sh: line 1: 29100 Abort trap: 6  cargo run --release
`

This leaves any attempt to simulate exceptions entirely at the mercy of the wider project settings. It's also the case
that some target platforms (for example, WebAssembly) *always* abort on panic, regardless of any compiler or project
settings.

A more subtle problem that's surfaced by panic handling is [*exception safety*][51]: if a panic occurs midway through an
operation on a data structure, it removes any guarantees that the data structure has been left in a self-consistent
state. Preserving internal invariants in the presence of exceptions has been known to be extremely difficult since the
1990s;^{[1][52]} this is one of the main reasons why [Google (famously) bans the use of exceptions in its C++ code][53].

Finally, panic propagation also [interacts poorly][54] with FFI (foreign function interface) boundaries ([Item 34][55]);
**use `catch_unwind` to prevent panics in Rust code from propagating to non-Rust calling code** across an FFI boundary.

So what's the alternative to `panic!` for dealing with error conditions? For library code, the best alternative is to
make the error [someone else's problem][56], by returning a `Result` with an appropriate error type ([Item 4][57]). This
allows the library user to make their own decisions about what to do next—which may involve passing the problem on to
the next caller in line, via the `?` operator.

The buck has to stop somewhere, and a useful rule of thumb is that it's OK to `panic!` (or to `unwrap()`, `expect()`,
etc.) if you have control of `main`; at that point, there's no further caller that the buck could be passed to.

Another sensible use of `panic!`, even in library code, is in situations where it's very rare to encounter errors, and
you don't want users to have to litter their code with `.unwrap()` calls.

If an error situation *should* occur only because (say) internal data is corrupted, rather than as a result of invalid
inputs, then triggering a `panic!` is legitimate.

It can even be occasionally useful to allow panics that can be triggered by invalid input but where such invalid inputs
are out of the ordinary. This works best when the relevant entrypoints come in pairs:
* An "infallible" version whose signature implies it always succeeds (and which panics if it can't succeed)
* A "fallible" version that returns a `Result`

For the former, Rust's [API guidelines][58] suggest that the `panic!` should be documented in a specific section of the
inline documentation ([Item 27][59]).

The [`String::from_utf8_unchecked`][60] and [`String::from_utf8`][61] entrypoints in the standard library are an example
of the latter (although in this case, the panics are actually deferred to the point where a `String` constructed from
invalid input gets used).

Assuming that you are trying to comply with the advice in this Item, there are a few things to bear in mind. The first
is that panics can appear in different guises; avoiding `panic!` also involves avoiding the following:
* [`unwrap()`][62] and [`unwrap_err()`][63]
* [`expect()`][64] and [`expect_err()`][65]
* [`unreachable!()`][66]

Harder to spot are things like these:
* `slice[index]` when the index is out of range
* `x / y` when `y` is zero

The second observation around avoiding panics is that a plan that involves constant vigilance of humans is never a good
idea.

However, constant vigilance of machines is another matter: adding a check to your continuous integration (see [Item
32][67]) system that spots new, potentially panicking code is much more reliable. A simple version could be a simple
grep for the most common panicking entrypoints (as shown previously); a more thorough check could involve additional
tooling from the Rust ecosystem ([Item 31][68]), such as setting up a build variant that pulls in the [`no_panic`][69]
crate.

¹

Tom Cargill's 1994 [article in the *C++ Report*][70] explores just how difficult exception safety is for C++ template
code, as does Herb Sutter's [Guru of the Week #8 column][71].

Effective Rust by David Drysdale
© 2024 Galloglass Consulting Limited

[[Creative Commons Attribution-NonCommercial-NoDerivatives 4.0 International License]][72]

[1]: title-page.html
[2]: preface.html
[3]: types.html
[4]: use-types.html
[5]: use-types-2.html
[6]: transform.html
[7]: errors.html
[8]: casts.html
[9]: newtype.html
[10]: builders.html
[11]: references.html
[12]: iterators.html
[13]: traits.html
[14]: std-traits.html
[15]: raii.html
[16]: generics.html
[17]: default-impl.html
[18]: concepts.html
[19]: lifetimes.html
[20]: borrows.html
[21]: unsafe.html
[22]: deadlock.html
[23]: panic.html
[24]: reflection.html
[25]: optimize.html
[26]: deps.html
[27]: semver.html
[28]: visibility.html
[29]: wildcard.html
[30]: re-export.html
[31]: dep-graph.html
[32]: features.html
[33]: tooling.html
[34]: documentation.html
[35]: macros.html
[36]: clippy.html
[37]: testing.html
[38]: use-tools.html
[39]: ci.html
[40]: beyond-std.html
[41]: no-std.html
[42]: ffi.html
[43]: bindgen.html
[44]: afterword.html
[45]: indexing.html
[46]: print.html
[47]: #item-18-dont-panic
[48]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[49]: https://doc.rust-lang.org/rustc/codegen-options/index.html#panic
[50]: https://doc.rust-lang.org/cargo/reference/profiles.html#panic
[51]: https://en.wikipedia.org/wiki/Exception_safety
[52]: #1
[53]: https://google.github.io/styleguide/cppguide.html#Exceptions
[54]: https://doc.rust-lang.org/nomicon/ffi.html#ffi-and-unwinding
[55]: ffi.html
[56]: https://en.wikipedia.org/wiki/Somebody_else%27s_problem
[57]: errors.html
[58]: https://rust-lang.github.io/api-guidelines/documentation.html#function-docs-include-error-panic-and-safety-conside
rations-c-failure
[59]: documentation.html
[60]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8_unchecked
[61]: https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf8
[62]: https://doc.rust-lang.org/std/result/enum.Result.html#method.unwrap
[63]: https://doc.rust-lang.org/std/result/enum.Result.html#method.unwrap_err
[64]: https://doc.rust-lang.org/std/result/enum.Result.html#method.expect
[65]: https://doc.rust-lang.org/std/result/enum.Result.html#method.expect_err
[66]: https://doc.rust-lang.org/std/macro.unreachable.html
[67]: ci.html
[68]: use-tools.html
[69]: https://docs.rs/no-panic
[70]: https://ptgmedia.pearsoncmg.com/imprint_downloads/informit/aw/meyerscddemo/DEMO/MAGAZINE/CA_FRAME.HTM
[71]: http://www.gotw.ca/gotw/008.htm
[72]: http://creativecommons.org/licenses/by-nc-nd/4.0/
```
