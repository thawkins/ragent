# Web source

- URL: https://discuss.ocaml.org/t/catching-panics-in-rust/11730
- Title: [OCaml][1]
- Captured (UTC): 2026-06-29T16:20:29.374419258+00:00

```text
[OCaml][1]

# [Catching panics in Rust][2]

[ Ecosystem ][3]
[rust][4]
[mimoo][5] March 20, 2023, 7:28pm 1

I’m doing a lot of FFI work with OCaml and Rust (OCaml using Rust code) and I’m running into some issues that are hard
for me to explain (due to my lack of knowledge of the internals).

Perhaps someone could think of something here, or know what I should read to understand these things more.

So, weird things arise when I panic in Rust, and I don’t really get clean stack traces. What I’ve been trying to do is
move panics to the OCaml side, and raise exception there when I can instead. But it doesn’t always work as sometimes
there’s a bug and a panic is still there. In these cases I want to be able to investigate and debug to find the source
of things.

My solutions have been to try to either:
1. catch the panic in Rust, and do something with it. ([Better backtrace when Rust panics · Issue #117 ·
   zshipko/ocaml-rs · GitHub][6])
2. create an error in Rust that contains a backtrace. (using [thiserror - Rust][7])

but both ways end up giving me weird ABRT signals and

" ```
fatal runtime error: failed to initiate panic, error 5

`
I'm guessing there's something fundamentally wrong about trying to play with Rust's panics in an OCaml binary, but I'm n
ot sure.

EDIT: catching a panic looks like this from OCaml:

`

thread panicked while processing panic. aborting.
File “src/lib/pickles/dune”, line 2, characters 1-15:
2 | (inline_tests)
^^^^^^^^^^^^^^
Command got signal ABRT.

``

[Chet_Murthy][8] March 20, 2023, 7:30pm 2

Is it actually kosher to catch a panic? I thought that Rust gave few guarantees about the state of the runtime when a
panic happens – and that you were meant to abort the program and never try to recover ?

[mimoo][9] March 20, 2023, 7:41pm 3

Good question. (Note that in the case where I’m just creating a backtrace I’m not even trying to catch a panic.)

FYI I’ve used this API from Rust: [set_hook in std::panic - Rust][10]

I’ll read more about it now.

EDIT: it looks like the default panic runtime is “unwind”, which will cleanly unwind and free memory, but you can also
set `panic_abort` which will not. In my case I’m wondering if the OCaml panics and the Rust panics are colliding
somehow.

[gadmm][11] March 21, 2023, 12:25am 4

In some cases, panics are not set to unwinding by default but aborting (e.g. in case of cross-compilation). Your issue
sounds similar to [rust - catch_unwind signal SIGABRT when unwrap a Result inside it - Stack Overflow][12]. This post
does not have a solution, but does explicitly setting panics to unwinding help?

As for OCaml exceptions and Rust panics colliding, this is possible in principle (but I am not sure this happens in your
example). Unwinding across OCaml frames or raising OCaml exceptions across Rust frames is undefined behaviour. You have
to convert between exceptions and panics at boundaries.^{[[1]][13]}
1. Ideally, this should be handled by the FFI package (ocaml-rs, ocaml-interop). Moreover, I think that round-trip
   conversion between exceptions and panics is possible with more effort (conditional on an appropriate use of
   exceptions on the OCaml side), but for simple error reporting this is probably not needed. [↩︎][14]

[gadmm][15] March 21, 2023, 12:34am 5

Panics can be caught, this comes from the time the exception-handling story of Rust was inspired by Erlang (let it
fail—a thread should be able to fail without disturbing the rest of the program). `panic::catch_unwind` has the
`UnwindSafe` bound on its closure argument, which is meant to ensure that any escaping value remains in a consistent
state in case of panic. Though this bound does not apply to the case of catching panics at FFI boundaries, for which
there are many risks.

[zeroexcuses][16] March 21, 2023, 1:02am 6
Chet_Murthy:

> Is it actually kosher to catch a panic?

I have no experience with catching panics in Rust, but my limited understanding of catching panics in Rust is:
* all bets are off
* print some error msg, and terminate

I think the solution here is to engineer the Rust side to return a Result instead of trying to catch panics.

[Chet_Murthy][17] March 21, 2023, 1:13am 7
zeroexcuses:

> I think the solution here is to engineer the Rust side to return a Result instead of trying to catch panics.

Yes, that is my assessment, too.

[mimoo][18] March 21, 2023, 9:49am 8

I’m wondering if during unwinding Rust is also trying to free some memory that’s still alive from OCaml’s perspective…

I’m also wondering why Rust seems to be “all bets are off” if you can also cleanly catch panics in Rust

[zeroexcuses][19] March 21, 2023, 9:57am 9
mimoo:

> I’m also wondering why Rust seems to be “all bets are off” if you can also cleanly catch panics in Rust

There are other nasty things besides this (so this is a lower bound on the danger of catching panic; not the only danger
of catching panic) – the example I often read about is this.
1. we acquire a `Arc<Mutex<...>>`
2. we panic
3. we catch the panic

What do we do now with the Mutex ?

If we release it, well, now the system is in some inconsistent state / we broke assumptions about the Mutex.

If we continue to hold it, we’re probably going to dead lock something.

At this point, it seems like we might as well as terminate the program.

Again, I’m not saying this is the only bad thing from catching mutex; just one of many things that can go wrong.

[mimoo][20] March 21, 2023, 10:12am 10

My understanding is that they poison the mutex at this point, and anybody who tries to unwrap it will then panic (or
they can try to handle a poisoned mutex more gracefully)

[gadmm][21] March 21, 2023, 11:57am 11

Looking at the issue you filed, in the first case it seems that you replaced the ocaml-rs panic hook with your own.
OCaml-rs used to convert panics into exceptions at the boundary, but this seems to have been changed to *directly
raising an OCaml exception from the panic hook*. (I do not think this is a good thing to do, but this is off-topic for
this thread.) Thus when you replaced the panic hook, you changed the panic behaviour to continue with normal unwinding
and thus started unwinding OCaml frames (which is undefined behaviour), since ocaml-rs no longer had other protections
against panics.

As advised by Zach in the issue above, I would try calling the `backtrace` crate *from the ocaml-rs panic handler*, that
is without changing the existing behaviour. (In the longer term it would be good to go back to re-raising the panic as
an exception at the moment of returning to OCaml rather than from the panic hook, but this is another topic.)

[zeroexcuses][22] March 21, 2023, 8:59pm 12

Personally, if I am not start enough to prevent panics; I am certainly not smart enough to reason about what will
continue to work / break after panic, when there are poisoned Mutex among other things.

Thus, in theory, I agree with you that you could carefully reason about post-panic code; in practice, I view it as all
bets are off (with exception of printing some error msgs) – because if I’m not smart enough to prevent the panic, I’m
probably not smart enough to reason about post-panic code.

[gadmm][23] March 23, 2023, 6:13pm 13

As the discussion revolves around FFI work with OCaml and Rust, catching panics in Rust is indeed relevant and can be
done safely when the panic is converted into an exception at the boundary. Of course, it is essential that the OCaml
code takes these exceptions just as seriously as a panic. It is in fact instructive for us to correctly understand the
panic mechanism as it teaches a lot about how one can write exception-safe OCaml code.

You raise valid concerns about reasoning about post-panic code in the general case, but one of the main reasons behind
panics is to simplify dealing with post-panic situations. It can actually be easier to reason about post-panic code than
figuring out all the possible sources of panic. By using Rust’s mechanisms, such as destructors to clean up state and
the `UnwindSafe` trait, one can be more confident in the safety and consistency of the code after a panic. The key is to
choose a suitable location for recovery, one where reasoning about the state of the program is manageable.

Think of it like force-closing a process in an operating system. The process may not have planned for every crash
scenario, but the OS has mechanisms to safely close it and clean up resources without destabilizing the whole system.

Panics and `Result` have distinct roles in Rust, each with specific use cases. Replacing panics with `Result` is not a
solution, as it would imply having to consider all possible error scenarios. External factors, like using an FFI, should
not force the use of one error-handling mechanism over the other.

[zeroexcuses][24] March 23, 2023, 8:59pm 14

I’m disengaging from this discussion. I will leave with two links, from “the book”

[https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html][25]

[https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html][26]

It is pretty clear to me: panic are NOT exceptions; when a library panics, it is the library author saying “I give up”;
catching panics & resuming execution as if everything is normal … is insanity to me.

[mimoo][27] March 23, 2023, 11:00pm 15

your first link does say that it will unwind cleanly though. I think the language of “unrecoverable” is due to panics
not being treated as exceptions in Rust. As in, they’re heavily discouraged, but it seems like you could be writing a
safe erlang-like supervisor/agent model in Rust via catching panics

[gadmm][28] March 24, 2023, 12:09am 16

Indeed, Erlang was an inspiration for Rust’s panic model. For sources that explain the notion of panic safety maybe
better than I did, you can check out the [Nomicon][29] and the [documentation for UnwindSafe][30].

Thanks for the valuable discussion, as I believe this topic is crucial for the robust handling of serious exceptions in
multicore OCaml too. These aspects are often overlooked, but they represent another aspect of Rust that seeks to bring
the qualities of pure code to imperative programming. I appreciate the scepticism, and I would always be happy to
continue the discussion.

### Related topics

────────────────────────────────────────────────────────────────────────────────────────────┬──┬────┬─────────────┬─────
Topic                                                                                       │  │Repl│Views        │Activ
                                                                                            │  │ies │             │ity  
────────────────────────────────────────────────────────────────────────────────────────────┼──┼────┼─────────────┴─────
[[BLOG] OCaml Backtraces on Uncaught Exceptions, by OCamlPro][31]                           │13│1033│May 23, 2024 
[ Learning ][32]                                                                            │  │    │             
[blog][33] ,  [ocaml][34] ,  [exceptions][35] ,  [backtrace][36]                            │  │    │             
────────────────────────────────────────────────────────────────────────────────────────────┼──┼────┼─────────────
[How OCaml exception handling works?][37]                                                   │10│1578│August 24,   
[ Learning ][38]                                                                            │  │    │2023         
[error-handling][39]                                                                        │  │    │             
────────────────────────────────────────────────────────────────────────────────────────────┼──┼────┼─────────────
[Catching non-fatal exceptions?][40]                                                        │6 │606 │April 20,    
[ Learning ][41]                                                                            │  │    │2024         
────────────────────────────────────────────────────────────────────────────────────────────┼──┼────┼─────────────
[[ANN] Rewriting the OCaml runtime in Rust and unionizing the standard library][42]         │5 │1199│April 1, 2023
[ Community ][43]                                                                           │  │    │             
[announce][44]                                                                              │  │    │             
────────────────────────────────────────────────────────────────────────────────────────────┼──┼────┼─────────────
[Exception vs Result][45]                                                                   │60│1016│September 15,
[ Learning ][46]                                                                            │  │2   │2023         
[exceptions][47]                                                                            │  │    │             
────────────────────────────────────────────────────────────────────────────────────────────┴──┴────┴─────────────
* [Home ][48]
* [Categories ][49]
* [Guidelines ][50]
* [Terms of Service ][51]
* [Privacy Policy ][52]

Powered by [Discourse][53], best viewed with JavaScript enabled

[1]: /
[2]: /t/catching-panics-in-rust/11730
[3]: /c/eco/15
[4]: https://discuss.ocaml.org/tag/rust
[5]: https://discuss.ocaml.org/u/mimoo
[6]: https://github.com/zshipko/ocaml-rs/issues/117
[7]: https://docs.rs/thiserror/latest/thiserror/
[8]: https://discuss.ocaml.org/u/Chet_Murthy
[9]: https://discuss.ocaml.org/u/mimoo
[10]: https://doc.rust-lang.org/std/panic/fn.set_hook.html
[11]: https://discuss.ocaml.org/u/gadmm
[12]: https://stackoverflow.com/q/57906689
[13]: #footnote-51179-1
[14]: #footnote-ref-51179-1
[15]: https://discuss.ocaml.org/u/gadmm
[16]: https://discuss.ocaml.org/u/zeroexcuses
[17]: https://discuss.ocaml.org/u/Chet_Murthy
[18]: https://discuss.ocaml.org/u/mimoo
[19]: https://discuss.ocaml.org/u/zeroexcuses
[20]: https://discuss.ocaml.org/u/mimoo
[21]: https://discuss.ocaml.org/u/gadmm
[22]: https://discuss.ocaml.org/u/zeroexcuses
[23]: https://discuss.ocaml.org/u/gadmm
[24]: https://discuss.ocaml.org/u/zeroexcuses
[25]: https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html
[26]: https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html
[27]: https://discuss.ocaml.org/u/mimoo
[28]: https://discuss.ocaml.org/u/gadmm
[29]: https://doc.rust-lang.org/nomicon/unwinding.html
[30]: https://doc.rust-lang.org/std/panic/trait.UnwindSafe.html
[31]: https://discuss.ocaml.org/t/blog-ocaml-backtraces-on-uncaught-exceptions-by-ocamlpro/14551
[32]: /c/learning/7
[33]: https://discuss.ocaml.org/tag/blog/16
[34]: https://discuss.ocaml.org/tag/ocaml/91
[35]: https://discuss.ocaml.org/tag/exceptions/314
[36]: https://discuss.ocaml.org/tag/backtrace/493
[37]: https://discuss.ocaml.org/t/how-ocaml-exception-handling-works/12878
[38]: /c/learning/7
[39]: https://discuss.ocaml.org/tag/error-handling/614
[40]: https://discuss.ocaml.org/t/catching-non-fatal-exceptions/14484
[41]: /c/learning/7
[42]: https://discuss.ocaml.org/t/ann-rewriting-the-ocaml-runtime-in-rust-and-unionizing-the-standard-library/11852
[43]: /c/community/5
[44]: https://discuss.ocaml.org/tag/announce/23
[45]: https://discuss.ocaml.org/t/exception-vs-result/6931
[46]: /c/learning/7
[47]: https://discuss.ocaml.org/tag/exceptions/314
[48]: /
[49]: /categories
[50]: /guidelines
[51]: /tos
[52]: /privacy
[53]: https://www.discourse.org
```
