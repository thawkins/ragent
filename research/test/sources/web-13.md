# Web source

- URL: https://www.alexdwilson.dev/learning-in-public/result-vs-panic-in-rust
- Title: [
- Captured (UTC): 2026-06-29T16:20:24.544433375+00:00

```text
[
0
][1]
[ Skip to Content ][2]
[Alex Wilson][3]
[ Home ][4]
[ Blog ][5]
[ About ][6]
[ Search ][7]
Open Menu Close Menu
[Alex Wilson][8]
[ Home ][9]
[ Blog ][10]
[ About ][11]
[ Search ][12]
Open Menu Close Menu
[
Home
][13]
[
Blog
][14]
[
About
][15]
[
Search
][16]

# Result Vs. Panic in Rust

Mar 16
Written By [Alex Wilson][17]

This is going to be a short one because I need a short one, and also it’s such a weird thing I’m not sure I’ve
encountered similarities in other languages. I’ve designed a custom concept in TypeScript for giving myself an easier
time with async error debugging, but I’ve never seen something like this built-in before.

P.S. If you somehow found this post first in your error information search, and instead you’d like a well-written, very
detailed post about this check out [this blog][18].

## Unrecoverable Errors

These are the situations that halt a program. All programming languages have some flavor of this, even if it just
results in the system completely stopping and doing nothing else it’s a representation of the same fundamental thing.
And that thing is, “The system has encountered a command to carry out an operation that the compiler or the interpreter
is not capable of.”

The reason it’s not capable of doing these operations is wildly different across cases and systems, but it really boils
down to the compiler not really understanding how to approach the command. You cannot read a file that does not exist
for example, so you either fail to read that file, or let someone know you failed to read that file.

### Enter Panic!

Alerting you to the failure mode is what Rust Panic does. Unrecoverable errors that the compiler cannot handle trigger
the panic macro, and that is the system’s way of letting you know what’s gone on.

Rust Errors are superb, a case study in excellence in error reporting. It’s certainly the best erroring system I’ve
encountered, and it’s just baked in like that.

But Rust also has the ability to allow you to create a nicely packaged Error message when errors of certain types might
occur, and that is a little ‘wrapper’ called a [Result][19].

## Result is so aptly named

Result is a really cool way to look at the value that a bit of code is supposed to produce. In file reads, HTTP
requests, or any sort of read-write op that wasn’t synchronous, it’s pretty common to evaluate that incoming structure
or response as something that can be something other than what’s expected.

Result just represents the output of a bit of code that either worked, or didn’t. Or perhaps ended up in some other
state that we need to represent.

It’s a cool thing for the compiler to distinguish this as the non-norm because now if you break plausibility it panics.
And using the result you still won’t break plausibility because the output of the code will be what you expect, or not,
and the compiler has instructions to work with both.

The compiler in Rust protects itself from you by forcing you to follow plausibility rules. It protects you from yourself
as well because now the bits of your code-base that are no longer present in your mind while you are problem-solving are
producing the stuff they claim they are producing, or they are panicking.

This is not true of any other language I’ve worked with so far.

## Conclusion

Rust **Panic** is the Rusty way of dealing with program implausibility or fault-producing code that violates compiler
rules in any way. Panic jumps out of the program, handles program wind down, and gives you all sorts of details about
where and why it happened.

**Result** is a way to deal with a place in the code where things have a good to high chance of producing an
unrecoverable error, and triggering the Rust panic macro. I’d be interested to see how result fares in code that has the
need of being highly performant, I’ll probably be learning more about this soon in the chapters about Smart Pointers and
Unsafe Code.

[Rust][20]
[ Alex Wilson ][21]
[
Previous
Previous

## Grocery and Recipe Simplification

][22] [
Next
Next

## What are Traits in Rust?

][23]

#### Socials

[
][24] [
][25] [
][26]

#### alex@alexdwilson.dev

[1]: /cart
[2]: #page
[3]: /
[4]: /
[5]: /learning-in-public
[6]: /about
[7]: /search-stuff
[8]: /
[9]: /
[10]: /learning-in-public
[11]: /about
[12]: /search-stuff
[13]: /
[14]: /learning-in-public
[15]: /about
[16]: /search-stuff
[17]: /learning-in-public?author=62d57df579363d1cceddb704
[18]: https://www.shuttle.rs/blog/2022/06/30/error-handling
[19]: https://doc.rust-lang.org/std/result/
[20]: /learning-in-public/tag/Rust
[21]: /learning-in-public?author=62d57df579363d1cceddb704
[22]: /learning-in-public/grocery-and-recipe-simplification
[23]: /learning-in-public/what-are-traits-in-rust
[24]: https://github.com/aptlyundecided
[25]: https://medium.com/@adwilson0286
[26]: https://www.linkedin.com/in/alexander-wilson-47417a4b/
```
