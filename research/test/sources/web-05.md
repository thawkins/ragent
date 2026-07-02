# Web source

- URL: https://www.ncameron.org/blog/to-panic-or-not-to-panic
- Title: [[featherweight musings]][1]
- Captured (UTC): 2026-06-29T16:20:17.275377293+00:00

```text
[[featherweight musings]][1]
* [Home][2]
15 October 2025

# To panic or not to panic

From a user's perspective an uncaught panic in a Rust program is a crash. A panic will terminate the thread and unless
the developers have taken some care, that leads to the program terminating. This is not an exploitable crash and Rust
usually ensures that destructors are called, but the program still crashes.

This might seem fine or really bad, depending on your perspective. But I think we can all agree that an uncaught panic
is never a good user experience. As Rust developers, how should we think about panics? How do we write nice code and
give our users a nice experience?

Before getting into the options, it's worth noting that whatever approach you take to panicking, you need a robust error
handling strategy. Panicking should never be your primary mechanism for handling errors.

## Code which never panics

Writing code that never panics seems like the right thing to do. However, it is very difficult, verging on impossible.
Rust has no language features to help do it (such as an effect system to show whether calling a function can cause a
panic), and the language, standard library, and many crates are designed around an assumption that panicking is ok
(neither of which may be the best decisions in retrospect, but we've got what we've got).

To get specific, the language itself can panic on integer overflows (only in debug builds) and on out of bounds
indexing, the standard library can panic in a bunch of places, I couldn't find an exhaustive list, but my best effort
is:
* explicit panicking macros such as `panic`, `unimplemented`, etc.
* assertion macros such as `assert_eq`
* `unwrap`, `expect`, and similar methods on types like `Option` and `Result` which panic in the presence of an
  unexpected variant (it's worth calling out the `lock().unwrap()` idiom for handling mutex poisoning which is a
  frequent source of potential panics in many programs),
* methods on `RefCell`, `Cell`, etc. such as `RefCell::borrow` which panic on violations of their borrowing invariants,
* `push` and similar methods on collections when their capacity overflows,
* `Iterator::step_by(0)`
* any function which can allocate where allocation fails if the allocator panics (which is usually only in `no_std`
  builds; I'm not sure of the exact rules).

And any dependent crate could panic in any function, potentially (and arguably it's not a semver breaking change for
this behaviour to change with new releases).

It is possible to write code that avoids all of the above, but it's not very much fun, and the result is unlikely to be
idiomatic Rust for anything non-trivial.

Unfortunately there is not much to help eliminate, reduce, or contain panics. Rust doesn't have effect checking for
panics; there are Clippy lints but they only do a shallow check, they don't cover panics in nested function calls (nor
do they cover all sources of panics). There are some tricks with the linker which can be used to ensure a program
doesn't panic (see [https://blog.aheymans.xyz/post/don_t_panic_rust/][3]), but they're a pain to use.

If you are writing small programs with high-priority requirements for not panicking, it is possible to write
non-panicking code, and if the cost is justified then this is a feasible approach. However, the costs are high and for
most programs this is not worthwhile. **Pretending you are writing panic-free code by avoiding explicit panics is just
wishful thinking.**

## Code which only panics on bugs

The [official advice][4] from the Rust project is that panics should never occur unless there is a bug. Unfortunately,
bugs are not impossible, so following this advice will inevitably lead to production code panicking, which is (without
any further mitigation) a bad experience for your users.

To put it another way, it is very difficult to know (even more difficult to *prove*) that a potential panic in your code
will never be triggered. At the very least, this requires high-quality programming and extensive testing. That is still
only going to improve things, not solve them completely.

A useful distinction is to make a distinction between relying on local vs non-local invariants. Using panics which rely
on local invariants to demonstrate their impossibility is acceptable, but relying on non-local invariants is probably
too risky.

For example, this kind of thing is OK (from the perspective of not panicking, it is unlikely to be idiomatic Rust code
in general):

`if i < arr.len() {
  // arr[i] could panic, but the check above ensures that it won't.
  println!("{}", arr[i]);
}
`

But I would try to avoid this sort of thing (although at least it's documented):

`/// Caller must ensure `i < arr.len()` (otherwise will panic)
pub fn foo<T>(arr: &[T], i: usize) -> &T {
  &arr[i]
}
`

I think that striving for only impossible panics is a good start, but it still requires handling the 'impossible' panics
when they do happen.

## Handling panics

An alternative to not panicking is to assume your program might panic and ensure that those panics are handled in a way
that they don't end up as a bad user experience. Panics can be handled in various places - at thread boundaries, at
process boundaries (i.e., in your `main` function or similar), or outside the process (by running your program in some
kind of supervised environment). You can then either panic liberally or follow the above advice to only panic on bugs. I
would strongly recommend against using panics as a general exception mechanism though.

The big drawback is that your program has to be able to recover from panics. Since panicking runs destructors, you
should (in theory) be able to keep program state consistent, but often that doesn't work, which makes recovery harder,
possibly impossible. A particular hazard is that your program recovers but panics for the same reason as before and you
get into an infinite loop of panicking and recovering.

A few other potential issues are that you have to be aware of panics at FFI boundaries (you must use the `-unwind`
flavours of ABI and the interaction between panicking and other languages exception mechanisms is undefined), you must
be aware that double panics (panicking while unwinding a panic) will cause the process to abort, and that if you build
your program with `panic=abort` then its behaviour will change.

## Conclusion

There is no perfect answer. Making code strictly panic-free is possible, but hard work and only feasible in certain
situations. For most code, minimising panics in the code and handling panics is a good solution, but is a bit more work
than people usually expect. Letting the program panic is OK in some situations, but make sure that it really is ok and
you're not just telling yourself that (and also that you have realistic expectations about how often panics will happen,
i.e., not 'never').

There is no solution where you can forget about panicking and just think about the happy path. Although panics are
'safe', you do still need to think about panics when programming with Rust. All serious projects should always have a
strategy for panics as part of their high-level design. Panicking is an implicit edge case which you should always keep
in mind when writing or reviewing Rust code.

I currently have availability for Rust coaching, adoption, or development; from a single call to ongoing 3 days/week. I
can help your team get things done, adopt Rust and use it more effectively, or to accurately evaluate Rust as a new
technology.

If you're adopting Rust, I can help make that a success with advice, 1:1 or group mentoring, design and code review, or
online support. [Coaching][5].

If you're building with Rust and need a short or medium-term boost, I can join your team, quickly get up to speed, and
deliver value. I have expertise with async and unsafe code, database implementation, distributed systems, dev tools, and
language implementation. [Consulting][6].

[

## KCL part 0

For roughly the past year, I've been working with the good folk at Zoo on their CAD product, the Zoo Modelling Studio
(aka ZMS, aka KittyCAD). I've mostly been working on the design

][7]
[

## Back to work

Hi ho! Hi ho! It's back to work I go! After taking a bit of a break from work, I am officially back into it (I've
actually been back into it since July,

][8]
[ featherweight musings ][9]
—
To panic or not to panic
Share this
[featherweight musings][10] © 2026 [Latest Posts][11] [Ghost][12]

[1]: https://www.ncameron.org/blog
[2]: https://www.ncameron.org/blog/
[3]: https://blog.aheymans.xyz/post/don_t_panic_rust/
[4]: https://doc.rust-lang.org/std/macro.panic.html#when-to-use-panic-vs-result
[5]: https://www.ncameron.org/coaching
[6]: https://www.ncameron.org/consulting
[7]: /blog/kcl-part-0/
[8]: /blog/back-to-work/
[9]: https://www.ncameron.org/blog
[10]: https://www.ncameron.org/blog
[11]: https://www.ncameron.org/blog
[12]: https://ghost.org
```
