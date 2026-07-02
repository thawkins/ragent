# Web source

- URL: https://www.ralfj.de/blog/2019/11/25/how-to-panic-in-rust.html
- Title: * [ralfj.de][1]
- Captured (UTC): 2026-06-29T16:20:20.874427967+00:00

```text
* [ralfj.de][1]
* * [Projects][2]
  * [Blog][3]
  * * [RSS Feed][4]
Nov 25, 2019 • [Rust][5] • [Edits][6] • [Permalink][7]

# How to Panic in Rust

What exactly happens when you `panic!()`? I recently spent a lot of time looking at the parts of the standard library
concerned with this, and it turns out the answer is quite complicated! I have not been able to find docs explaining the
high-level picture of panicking in Rust, so this feels worth writing down.

(Shameless plug: the reason I looked at this is that @Aaron1011 implemented unwinding support in Miri. I wanted to see
that in Miri since forever and never had the time to implement it myself, so it was really great to see someone just
submit PRs for that out of the blue. After a lot of review rounds, this landed just recently. There [are still some
rough edges][8], but the foundations are solid.)

The purpose of this post is to document the high-level structure and the relevant interfaces that come into play on the
Rust side of this. The actual mechanism of unwinding is a totally different matter (and one that I am not qualified to
speak about).

*Note:* This post describes panicking as of [this commit][9]. Many of the interfaces described here are unstable
internal details of libstd, and subject to change any time.

*Update (2025-08-18):* As expected, many parts of this post are quite outdated by now. See [here][10] for an issue
tracking the ongoing cleanup effort of the panic machinery.

## High-level structure

When trying to figure out how panicking works by reading the code in libstd, one can easily get lost in the maze. There
are multiple layers of indirection that are only put together by the linker, there is the [`#[panic_handler]`
attribute][11] and the [“panic runtime”][12] (controlled by the panic *strategy*, which is set via `-C panic`) and
[“panic hooks”][13], and it turns out panicking in `#[no_std]` context takes an entirely different code path… there is
just a lot going on. To make things worse, [the RFC describing panic hooks][14] calls them “panic handler”, but that
term has since been re-purposed.

I think the best place to start are the interfaces controlling the two indirections:
* The *panic runtime* is used by libstd to control what happens after the panic information has been printed to stderr.
  It is determined by the panic *strategy*: either we abort (`-C panic=abort`) or we unwind (`-C panic=unwind`). (The
  panic runtime also provides the implementation for [`catch_unwind`][15] but we are not concerned with that here.)
* The *panic handler* is used by libcore to implement (a) panics inserted by code generation (such as panics caused by
  arithmetic overflow or out-of-bounds array/slice indexing) and (b) the `core::panic!` macro (this is the `panic!`
  macro in libcore itself and in `#[no_std]` context in general).

Both of these interfaces are implemented through `extern` blocks: listd/libcore, respectively, just import some function
that they delegate to, and somewhere entirely else in the crate tree, that function gets implemented. The import only
gets resolved during linking; looking locally at that code, one cannot tell where the actual implementation of the
respective interface lives. No wonder that I got lost several times along the way.

In the following, both of these interfaces will come up a lot; when you get confused, the first thing to check is if you
just mixed up panic *handler* and panic *runtime*. (And remember there’s also panic *hooks*, we will get to those.) That
happens to me all the time.

Moreover, `core::panic!` and `std::panic!` are *not* the same; as we will see, they take very different code paths.
libcore and libstd each implement their own way to cause panics:
* libcore’s `core::panic!` does very little, it basically just delegates to the panic *handler* immediately.
* libstd’s `std::panic!` (the “normal” `panic!` macro in Rust) triggers a fully-featured panic machinery that provides a
  user-controlled [panic *hook*][16]. The default hook will print the panic message to stderr. After the hook is done,
  libstd delegates to the panic *runtime*.
  
  libstd also provides a panic *handler* that calls the same machinery, so `core::panic!` also ends up here.

Let us now look at these pieces in a bit more detail.

## Panic Runtime

The [interface to the panic runtime][17] (introduced by [this RFC][18]) is a function `__rust_start_panic(payload:
usize) -> u32` that gets imported by libstd, and that is later resolved by the linker.

The `usize` argument here actually is a `*mut &mut dyn core::panic::BoxMeUp` – this is where the “payload” of the panic
(the information available when it gets caught) gets passed in. `BoxMeUp` is an unstable internal implementation detail,
but [looking at the trait][19] we can see that all it really does is wrap a `dyn Any + Send`, which is the [type of the
panic payload][20] as returned by `catch_unwind` and `thread::spawn`. `BoxMeUp::take_box` returns a `Box<dyn Any +
Send>`, but as a raw pointer (because `Box` is not available in the context where this trait is defined); `BoxMeUp::get`
just borrows the contents.

The two implementations of this interface Rust ships with are [`libpanic_unwind`][21] for `-C panic=unwind` (the default
on most platforms) and [`libpanic_abort`][22] for `-C panic=abort`.

## `std::panic!`

On top of the panic *runtime* interface, libstd implements the default Rust panic machinery in the internal
[`std::panicking`][23] module.

#### `rust_panic_with_hook`

The key function that almost everything passes through is [`rust_panic_with_hook`][24]:

`fn rust_panic_with_hook(
    payload: &mut dyn BoxMeUp,
    message: Option<&fmt::Arguments<'_>>,
    file_line_col: &(&str, u32, u32),
) -> !`

This function takes a panic source location, an optional unformatted panic message (see the [`fmt::Arguments`][25]
docs), and a payload.

Its main job is to call whatever the current panic hook is. Panic hooks have a [`PanicInfo`][26] argument, so we need a
panic source location, format information for a panic message, and a payload. This matches `rust_panic_with_hook`’s
arguments quite nicely! `file_line_col` and `message` can be used directly for the first two elements; `payload` gets
turned into `&(dyn Any + Send)` through the `BoxMeUp` interface.

Interestingly, the *default* panic hook entirely ignores the `message`; what you actually see printed is [the `payload`
downcast to `&str` or `String`][27] (whatever works). Supposedly, the caller should ensure that formatting `message`, if
present, gives the same result. (And the ones we discuss below do ensure this.)

Finally, `rust_panic_with_hook` dispatches to the current panic *runtime*. At this point, only the `payload` is still
relevant – and that is important: `message` (as the `'_` lifetime indicates) may contain short-lived references, but the
panic payload will be propagated up the stack and hence must be `'static`. The `'static` bound is quite well hidden
there, but after a while I realized that [`Any`][28] implies `'static` (and remember `dyn BoxMeUp` is just used to
obtain a `Box<dyn Any + Send>`).

#### libstd panicking entry points

`rust_panic_with_hook` is a private function to `std::panicking`; the module provides three entry points on top of this
central function, and one that circumvents it:
* the [`begin_panic_handler`][29], the default panic handler implementation that backs (as we will see) panics from
  `core::panic!` and built-in panics (from arithmetic overflow or out-of-bounds array/slice indexing). This obtains as
  input a [`PanicInfo` ][30], and it has to turn that into arguments for `rust_panic_with_hook`. Curiously, even though
  the components of `PanicInfo` and the arguments of `rust_panic_with_hook` match up pretty well and seem like they
  could just be forwarded, that is *not* what happens. Instead, libstd entirely *ignores* the `payload` component of the
  `PanicInfo`, and sets up the actual payload (passed to `rust_panic_with_hook`) such that it contains [the formatted
  `message`][31].
  
  In particular, this means that the panic *runtime* is irrelevant for `no_std` applications. It only comes into play
  when libstd’s panic handler implementation is used. (The panic *strategy* selected via `-C panic` still has an effect
  as it also influences code generation. For example, with `-C panic=abort` code can become simpler as it does not need
  to support unwinding.)
* [`begin_panic_fmt`][32], backing the [format string version of `std::panic!`][33] (i.e., this is used when you pass
  multiple arguments to the macro). This basically just packages the format string arguments into a `PanicInfo` (with a
  [dummy payload][34]) and calls the default panic handler that we just discussed.
* [`begin_panic`][35], backing the [single-argument version of `std::panic!`][36]. Interestingly, this uses a very
  different code path than the other two entry points! In particular, this is the only entry point that permits passing
  in an *arbitrary payload*. That payload is just [converted into a `Box<dyn Any + Send>`][37] so that it can be passed
  to `rust_panic_with_hook`, and that’s it.
  
  In particular, a panic hook that looks at the `message` field of the `PanicData` it is passed will *not* be able to
  see the message in a `std::panic!("do panic")`, but it *will* see the message in a `std::panic!("panic with data: {}",
  data)` as the latter passes through `begin_panic_fmt` instead. That seems quite surprising. (But also note that
  `PanicData::message()` is not stable yet.)
* [`rust_panic_without_hook`][38] is the odd one out: this entry point backs [`resume_unwind`][39], and it actually does
  *not* call the panic hook. Instead, it dispatches to the panic runtime immediately. Like, `begin_panic`, it lets the
  caller pick an arbitrary payload. Unlike `begin_panic`, the caller is responsible for boxing and unsizing the payload;
  `update_count_then_panic` just forwards that pretty much verbatim to the panic runtime.

## Panic Handler

All of the `std::panic!` machinery is really useful, but it relies on heap allocations through `Box` which is not always
available. To give libcore a way to cause panics, [panic handlers were introduced][40]. As we have seen, if libstd is
available, it provides an implementation of that interface to wire `core::panic!` into the libstd panic machinery.

The [interface to the panic handler][41] is a function `fn panic(info: &core::panic::PanicInfo) -> !` that libcore
imports and that is later resolved by the linker. The [`PanicInfo` ][42] type is the same as for panic hooks: it
contains a panic source location, a panic message, and a payload (a `dyn Any + Send`). The panic message is represented
as [`fmt::Arguments`][43], i.e., a format string with its arguments that has not been formatted yet.

## `core::panic!`

On top of the panic handler interface, libcore provides a [minimal panic API][44]. The [`core::panic!`][45] macro
creates a `fmt::Arguments` which is then [passed to the panic handler][46]. No formatting happens here as that would
require heap allocations; this is why `PanicInfo` contains an “uninterpreted” format string with its arguments.

Curiously, the `payload` field of the `PanicInfo` that gets passed to the panic handler is always set to [a dummy
value][47]. This explains why the libstd panic handler ignores the payload (and instead constructs a new payload from
the `message`), but that makes me wonder why that field is part of the panic handler API in the first place. Another
consequence of this is that [`core::panic!("message")`][48] and [`std::panic!("message")`][49] (the variants without any
formatting) actually result in very different panics: the former gets turned into `fmt::Arguments`, passed through the
panic handler interface, and then libstd creates a `String` payload by formatting it. The latter, however, directly uses
the `&str` as a payload, and the `message` field remains `None` (as already mentioned).

Some elements of the libcore panic API are lang items because the compiler inserts calls to these functions during code
generation:
* The [`panic` lang item][50] is called when the compiler needs to raise a panic that does not require any formatting
  (such as arithmetic overflow); this is the same function that also backs single-argument [`core::panic!`][51].
* The [`panic_bounds_check`][52] lang item is called on a failed array/slice bounds check. It calls into the same method
  as [`core::panic!` with formatting][53].

## Conclusion

We have walked through 4 layers of APIs, 2 of which are indirected through imported function calls and resolved by the
linker. That’s quite a journey! But we have reached the end now. I hope you [didn’t panic][54] yourself along the way.
;)

I mentioned some things as being surprising. Turns out they all have to do with the fact that panic hooks and panic
handlers share the `PanicInfo` struct in their interface, which contains *both* an optional not-yet-formatted `message`
and a type-erased `payload`:
* The panic *hook* can always find the already formatted message in the `payload`, so the `message` seems pointless for
  hooks. In fact, `message` can be missing even if `payload` contains a message (e.g., for `std::panic!("message")`).
* The panic *handler* will never actually receive a useful `payload`, so that field seems pointless for handlers.

Reading the [panic handler RFC][55], it seems like the plan was for `core::panic!` to also support arbitrary payloads,
but so far that has not materialized. However, even with that future extension, I think we have the invariant that when
`message` is `Some`, then either `payload == &NoPayload` (so the payload is redundant) or `payload` is the formatted
message (so the message is redundant). I wonder if there is any case where *both* fields will be useful – and if not,
couldn’t we encode that by making them two variants of an `enum`? There are probably good reasons against that proposal
and for the current design; it would be great to get them documented somewhere. :)

There is a lot more to say, but at this point, I invite you to follow the links to the source code that I included
above. With the high-level structure in mind, you should be able to follow that code. If people think this overview
would be worth putting somewhere more permanently, I’d be happy to work this blog post into some form of docs – I am not
sure what would be a good place for those, though. And if you find any mistakes in what I wrote, please let me know!

Posted on [Ralf's Ramblings][56] on Nov 25, 2019.
Comments? [Drop me a mail][57] or [leave a note in the forum][58]!

[1]: /
[2]: /projects/
[3]: /blog/
[4]: /blog/feed.xml
[5]: /blog/categories/rust.html
[6]: https://git.ralfj.de/web.git/history/refs/heads/master:/personal/_posts/2019-11-25-how-to-panic-in-rust.md
[7]: /blog/2019/11/25/how-to-panic-in-rust.html
[8]: https://github.com/rust-lang/miri/issues?q=is%3Aissue+is%3Aopen+label%3AA-panics
[9]: https://github.com/rust-lang/rust/commit/4007d4ef26eab44bdabc2b7574d032152264d3ad
[10]: https://github.com/rust-lang/rust/issues/116005
[11]: https://doc.rust-lang.org/nomicon/panic-handler.html
[12]: https://github.com/rust-lang/rfcs/blob/master/text/1513-less-unwinding.md
[13]: https://doc.rust-lang.org/std/panic/fn.set_hook.html
[14]: https://github.com/rust-lang/rfcs/blob/master/text/1328-global-panic-handler.md
[15]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[16]: https://doc.rust-lang.org/std/panic/fn.set_hook.html
[17]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L53
[18]: https://github.com/rust-lang/rfcs/blob/master/text/1513-less-unwinding.md
[19]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panic.rs#L268-L281
[20]: https://doc.rust-lang.org/std/thread/type.Result.html
[21]: https://github.com/rust-lang/rust/tree/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libpanic_unwind
[22]: https://github.com/rust-lang/rust/tree/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libpanic_abort
[23]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs
[24]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L439-L441
[25]: https://doc.rust-lang.org/std/fmt/struct.Arguments.html
[26]: https://doc.rust-lang.org/core/panic/struct.PanicInfo.html
[27]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L176-L182
[28]: https://doc.rust-lang.org/std/any/trait.Any.html
[29]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L332
[30]: https://doc.rust-lang.org/core/panic/struct.PanicInfo.html
[31]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L350
[32]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L316
[33]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/macros.rs#L22-L23
[34]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panic.rs#L56
[35]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L392
[36]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/macros.rs#L16
[37]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L419
[38]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/panicking.rs#L496
[39]: https://doc.rust-lang.org/nightly/std/panic/fn.resume_unwind.html
[40]: https://github.com/rust-lang/rfcs/blob/master/text/2070-panic-implementation.md
[41]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panicking.rs#L78
[42]: https://doc.rust-lang.org/core/panic/struct.PanicInfo.html
[43]: https://doc.rust-lang.org/std/fmt/struct.Arguments.html
[44]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panicking.rs
[45]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/macros/mod.rs#L10-L26
[46]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panicking.rs#L82
[47]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panic.rs#L56
[48]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/macros/mod.rs#L15
[49]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libstd/macros.rs#L16
[50]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panicking.rs#L39
[51]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/macros/mod.rs#L15
[52]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/panicking.rs#L55
[53]: https://github.com/rust-lang/rust/blob/4007d4ef26eab44bdabc2b7574d032152264d3ad/src/libcore/macros/mod.rs#L21-L24
[54]: https://en.wikipedia.org/wiki/Phrases_from_The_Hitchhiker%27s_Guide_to_the_Galaxy#Don't_Panic
[55]: https://github.com/rust-lang/rfcs/blob/master/text/2070-panic-implementation.md
[56]: /blog
[57]: mailto:post+blog-AT-ralfj-DOT-de?subject=Blog post comment: How%20to%20Panic%20in%20Rust
[58]: https://internals.rust-lang.org/t/how-to-panic-in-rust/11368
```
