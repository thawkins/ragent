# Web source

- URL: https://docs.rs/no-panic
- Title: [ Docs.rs ][1]
- Captured (UTC): 2026-06-29T16:20:26.100923243+00:00

```text
[ Docs.rs ][1]
* [ no-panic-0.1.36 ][2]
  * no-panic 0.1.36
  * [ Permalink ][3]
  * [ Docs.rs crate page ][4]
  * [MIT][5] OR [Apache-2.0][6]
  * Links
  * [ Repository ][7]
  * [ crates.io ][8]
  * [ Source ][9]
  * Owners
  * [ dtolnay ][10]
  * Dependencies
  * * [ proc-macro2 ^1.0.74 normal ][11]
    * [ quote ^1.0.35 normal ][12]
    * [ syn ^2.0.46 normal ][13]
    * [ rustversion ^1.0.13 dev ][14]
    * [ scratch ^1 dev ][15]
    * [ trybuild ^1.0.108 dev ][16]
  * Versions
  * [ **50%** of the crate is documented ][17]
* [ Platform ][18]
  * [x86_64-unknown-linux-gnu][19]
* [ Feature flags ][20]
* [docs.rs][21]
  * [ About docs.rs][22]
  * [ Badges][23]
  * [ Builds][24]
  * [ Metadata][25]
  * [ Shorthand URLs][26]
  * [ Download][27]
  * [ Rustdoc JSON][28]
  * [ Build queue][29]
  * [ Privacy policy][30]
* [Rust][31]
  * [Rust website][32]
  * [The Book][33]
  * [Standard Library API Reference][34]
  * [Rust by Example][35]
  * [The Cargo Guide][36]
  * [Clippy Documentation][37]
[Skip to main content][38]

## [Crate no_panic][39]

## [no_panic][40]0.1.36
* [All Items][41]

### [Sections][42]
* [Caveats][43]
* [Acknowledgments][44]

### [Crate Items][45]
* [Attribute Macros][46]

# Crate no_panic Copy item path

[Source][47]
Expand description

[[github]][48] [[crates-io]][49] [[docs-rs]][50]

A Rust attribute macro to require that the compiler prove a function can’t ever panic.

`[dependencies]
no-panic = "0.1"`

`use no_panic::no_panic;

#[no_panic]
fn demo(s: &str) -> &str {
    &s[1..]
}

fn main() {
    println!("{}", demo("input string"));
}`

If the function does panic (or the compiler fails to prove that the function cannot panic), the program fails to compile
with a linker error that identifies the function name. Let’s trigger that by passing a string that cannot be sliced at
the first byte:

[ⓘ][51]

`fn main() {
    println!("{}", demo("\u{1f980}input string"));
}`

`   Compiling no-panic-demo v0.0.1
error: linking with `cc` failed: exit code: 1
  |
  = note: /no-panic-demo/target/release/deps/no_panic_demo-7170785b672ae322.no_p
anic_demo1-cba7f4b666ccdbcbbf02b7348e5df1b2.rs.rcgu.o: In function `_$LT$no_pani
c_demo..demo..__NoPanic$u20$as$u20$core..ops..drop..Drop$GT$::drop::h72f8f423002
b8d9f':
          no_panic_demo1-cba7f4b666ccdbcbbf02b7348e5df1b2.rs:(.text._ZN72_$LT$no
_panic_demo..demo..__NoPanic$u20$as$u20$core..ops..drop..Drop$GT$4drop17h72f8f42
3002b8d9fE+0x2): undefined reference to `

          ERROR[no-panic]: detected panic in function `demo`
          '
          collect2: error: ld returned 1 exit status`

The error is not stellar but notice the ERROR[no-panic] part at the end that provides the name of the offending
function.

### [§][52]Caveats
* Functions that require some amount of optimization to prove that they do not panic may no longer compile in debug mode
  after being marked `#[no_panic]`.
* Panic detection happens at link time across the entire dependency graph, so any Cargo commands that do not invoke a
  linker will not trigger panic detection. This includes `cargo build` of library crates and `cargo check` of binary and
  library crates.
* The attribute is useless in code built with `panic = "abort"`. Code must be built with `panic = "unwind"` (the
  default) in order for any panics to be detected. After confirming absence of panics, you can of course still ship your
  software as a `panic = "abort"` build.
* Const functions are not supported. The attribute will fail to compile if placed on a `const fn`.

If you find that code requires optimization to pass `#[no_panic]`, either make no-panic an optional dependency that you
only enable in release builds, or add a section like the following to your Cargo.toml or .cargo/config.toml to enable
very basic optimization in debug builds.

`[profile.dev]
opt-level = 1`

If the code that you need to prove isn’t panicking makes function calls to non-generic non-inline functions from a
different crate, you may need thin LTO enabled for the linker to deduce those do not panic.

`[profile.release]
lto = "thin"`

If thin LTO isn’t cutting it, the next thing to try would be fat LTO with a single codegen unit:

`[profile.release]
lto = "fat"
codegen-units = 1`

If you want no_panic to just assume that some function you call doesn’t panic, and get Undefined Behavior if it does at
runtime, see [dtolnay/no-panic#16][53]; try wrapping that call in an `unsafe extern "C"` wrapper.

### [§][54]Acknowledgments

The linker error technique is based on [Kixunil][55]’s crate [`dont_panic`][56]. Check out that crate for other
convenient ways to require absence of panics.

## Attribute Macros[§][57]

*[no_panic][58]*

[1]: /
[2]: #
[3]: /no-panic/0.1.36/no_panic/
[4]: /crate/no-panic/latest
[5]: https://spdx.org/licenses/MIT
[6]: https://spdx.org/licenses/Apache-2.0
[7]: https://github.com/dtolnay/no-panic
[8]: https://crates.io/crates/no-panic
[9]: /crate/no-panic/latest/source/
[10]: https://crates.io/users/dtolnay
[11]: /proc-macro2/^1.0.74/
[12]: /quote/^1.0.35/
[13]: /syn/^2.0.46/
[14]: /rustversion/^1.0.13/
[15]: /scratch/^1/
[16]: /trybuild/^1.0.108/
[17]: /crate/no-panic/latest
[18]: #
[19]: /crate/no-panic/latest/target-redirect/no_panic/
[20]: /crate/no-panic/latest/features
[21]: #
[22]: /about
[23]: /about/badges
[24]: /about/builds
[25]: /about/metadata
[26]: /about/redirections
[27]: /about/download
[28]: /about/rustdoc-json
[29]: /releases/queue
[30]: https://foundation.rust-lang.org/policies/privacy-policy/#docs.rs
[31]: #
[32]: https://www.rust-lang.org/
[33]: https://doc.rust-lang.org/book/
[34]: https://doc.rust-lang.org/std/
[35]: https://doc.rust-lang.org/rust-by-example/
[36]: https://doc.rust-lang.org/cargo/guide/
[37]: https://doc.rust-lang.org/nightly/clippy
[38]: #main-content
[39]: #
[40]: ../no_panic/index.html
[41]: all.html
[42]: #
[43]: #caveats
[44]: #acknowledgments
[45]: #attributes
[46]: #attributes
[47]: ../src/no_panic/lib.rs.html#1-329
[48]: https://github.com/dtolnay/no-panic
[49]: https://crates.io/crates/no-panic
[50]: https://docs.rs/no-panic
[51]: #
[52]: #caveats
[53]: https://github.com/dtolnay/no-panic/issues/16
[54]: #acknowledgments
[55]: https://github.com/Kixunil
[56]: https://github.com/Kixunil/dont_panic
[57]: #attributes
[58]: attr.no_panic.html
```
