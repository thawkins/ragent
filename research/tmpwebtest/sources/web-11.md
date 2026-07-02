# Web source

- URL: https://stackoverflow.blog/2020/01/20/what-is-rust-and-why-is-it-so-popular
- Title: [Blog][1]
- Captured (UTC): 2026-06-29T00:55:54.597554958+00:00

```text
[Blog][1]
Loading…
[ The 2026 Stack Overflow Developer Survey is live. **Take the survey**. ][2]
* [ Everything ][3]
* [ The Heap ][4]
* [ Productivity ][5]
* [ AI/ML ][6]
* [ Open Source ][7]
* [ Business Hub ][8]
* [ Company ][9]
* [ Releases ][10]
* [ Podcast ][11]
* [ Newsletter ][12]
[ Stack Overflow Business ][13]
[
**Stack Internal**: the knowledge intelligence layer that powers enterprise AI.
][14][
**Stack Data Licensing**: decades of verified, technical knowledge to boost AI performance and trust.
][15][
**Stack Ads**: engage developers where it matters — in their daily workflow.
][16]
January 20, 2020

# What is Rust and why is it so popular?

Rust has been Stack Overflow's most loved language for four years in a row, indicating that many of those who have had
the opportunity to use Rust have fallen in love with it. However, the roughly 97% of survey respondents who haven't used
Rust may wonder, "What's the deal with Rust?"

[Article hero image]

Rust has been [Stack Overflow's most loved language for four years in a row][17], indicating that many of those who have
had the opportunity to use Rust have fallen in love with it. However, the roughly 97% of survey respondents who haven't
used Rust may wonder, "What's the deal with Rust?"

The short answer is that Rust solves pain points present in many other languages, providing a solid step forward with a
limited number of downsides.

I’ll show a sample of what Rust offers to users of other programming languages and what the current ecosystem looks
like. It’s not all roses in Rust-land, so I talk about the downsides, too.

## Coming from dynamically-typed languages

The arguments between programmers who prefer dynamic versus static type systems are likely to endure for decades more,
but it's hard to argue about the benefits of static types. You only need to look at the rise of languages like
TypeScript or features like Python's type hints as people have become frustrated with the current state of dynamic
typing in today's larger codebases. Statically-typed languages allow for compiler-checked constraints on the data and
its behavior, alleviating cognitive overhead and misunderstandings.

This isn't to say that all static type systems are equivalent. Many statically-typed languages have a large asterisk
next to them: they allow for the concept of `NULL`. This means any value may be what it says **or nothing**, effectively
[creating a second possible type for every type][18]. Like Haskell and some other modern programming languages, Rust
encodes this possibility using an *optional type*, and the compiler requires you to handle the `None` case. This
prevents occurrences of the dreaded `TypeError: Cannot read property 'foo' of null` runtime error (or language
equivalent), instead promoting it to a compile time error you can resolve before a user ever sees it. Here's an example
of a function to greet someone whether or not we know their name; if we had forgotten the `None` case in the `match` or
tried to use `name` as if it was an always-present `String` value, the compiler would complain.

`fn greet_user(name: Option<String>) {
    match name {
        Some(name) => println!("Hello there, {}!", name),
        None => println!("Well howdy, stranger!"),
    }
}`

Rust's static typing does its best to get out of the programmer's way while encouraging long-term maintainability. Some
statically-typed languages place a large burden on the programmer, requiring them to repeat the type of a variable
multiple times, which hinders readability and refactoring. Other statically-typed languages allow whole-program type
inference. While convenient during initial development, this reduces the ability of the compiler to provide useful error
information when types no longer match. Rust learns from both of these styles and requires top-level items like function
arguments and constants to have explicit types, while allowing type inference inside of function bodies. In this
example, the Rust compiler can infer the type of `twice`, `2`, and `1` because the `val` parameter and the return type
are declared as 32-bit signed integers.

`fn simple_math(val: i32) -> i32 {
    let twice = val * 2;
    twice - 1
}`

## Coming from garbage-collected languages

One of the biggest benefits of using a systems programming language is the ability to have control over low-level
details.

Rust gives you the choice of storing data on the stack or on the heap and determines at compile time when memory is no
longer needed and can be cleaned up. This allows efficient usage of memory as well as more performant memory access.
[Tilde][19], an early production user of Rust in their [Skylight][20] product, found they were able to [reduce their
memory usage from 5GiB to 50MiB][21] by rewriting certain Java HTTP endpoints in idiomatic Rust. Savings like this
quickly add up when cloud providers charge premium prices for increased memory or additional nodes.

Without the need to have a garbage collector continuously running, Rust projects are well-suited to be used as libraries
by other programming languages via foreign-function interfaces. This allows existing projects to replace
performance-critical pieces with speedy Rust code without the memory safety risks inherent with other systems
programming languages. Some projects have even been [incrementally rewritten in Rust][22] using these techniques.

With direct access to hardware and memory, Rust is an ideal language for embedded and bare-metal development. You can
write extremely low-level code, such as [operating system kernels][23] or [microcontroller applications][24]. Rust's
core types and functions as well as reusable library code shine in these especially challenging environments.

## Coming from other systems programming languages

To many people, Rust is largely viewed as an alternative to other systems programming languages, like C or C++. The
biggest benefit Rust can provide compared to these languages is the *borrow checker*. This is the part of the compiler
responsible for ensuring that references do not outlive the data they refer to, and it helps eliminate entire classes of
bugs caused by memory unsafety.

Unlike many existing systems programming languages, Rust doesn't require that you spend all of your time mired in
nitty-gritty details. Rust strives to have as many *zero-cost abstractions* as possible—abstractions that are as equally
as performant as the equivalent hand-written code. In this example, we show how iterators, a primary Rust abstraction,
can be used to succinctly create a vector containing the first ten square numbers.

`let squares: Vec<_> = (0..10).map(|i| i * i).collect();`

When safe Rust isn't able to express some concept, [you can use *unsafe* Rust][25]. This unlocks a few extra powers, but
in exchange the programmer is now responsible for ensuring that the code is truly safe. This unsafe code can then be
wrapped in higher-level abstractions which guarantee that all uses of the abstraction are safe.

Using unsafe code should be a calculated decision, as using it correctly requires as much thought and care as any other
language where you are responsible for avoiding [undefined behavior][26]. Minimizing unsafe code is the best way to
minimize the possibilities for segfaults and vulnerabilities due to memory unsafety.

Systems programming languages have the implicit expectation that they will be around effectively forever. While some
modern development doesn't require that amount of longevity, many businesses want to know that their fundamental code
base will be usable for the foreseeable future. Rust recognizes this and has made conscious design decisions around
backwards compatibility and stability; it's a language [designed for the next 40 years][27].

## The Rust ecosystem

The Rust experience is larger than a language specification and a compiler; many aspects of creating and maintaining
production-quality software are treated as first-class citizens. Multiple concurrent Rust toolchains can be installed
and managed via [rustup][28]. Rust installations come with Cargo, a command line tool to manage dependencies, run tests,
generate documentation, and more. Because dependencies, tests, and documentation are available by default, their usage
is prevalent. [crates.io][29] is the community site for sharing and discovering Rust libraries. Any library published to
crates.io will have its documentation built and published on [docs.rs][30].

In addition to the built-in tools, the Rust community has created a large number of development tools. Benchmarking,
fuzzing, and property-based testing are all easily accessible and well-used in projects. Extra compiler lints are
available from [Clippy][31] and automatic idiomatic formatting is provided by [rustfmt][32]. IDE support is healthy and
growing more capable every day.

Going beyond technical points, Rust has a vibrant, welcoming community. There are several official and unofficial
avenues for people to get help, such as the [chat][33], the [user's forum][34], the [Rust subreddit][35], and, of
course, Stack Overflow [questions and answers][36] and [chatroom][37]. Rust has a [code of conduct][38] enforced by an
awesome moderation team to make sure that the official spaces are welcoming, and most unofficial spaces also observe
something similar.

Offline, Rust has multiple conferences across the globe, such as [RustConf][39], [Rust Belt Rust][40], [RustFest][41],
[Rust Latam][42], [RustCon Asia][43], and more.

## It's not all roses

Rust's strong type system and emphasis on memory safety—all enforced at compile time—mean that it's extremely common to
get errors when compiling your code. This can be a frustrating feeling for programmers not used to such an opinionated
programming language. However, the Rust developers have spent a large amount of time working to improve the error
messages to ensure that they are clear and actionable. Don't let your eyes gloss over while reading Rust errors!

It's especially common to hear someone complain that they've been "fighting the borrow checker." While these errors can
be disheartening, it's important to recognize that each of the locations identified had the potential to introduce bugs
and potential vulnerabilities in a language that didn't perform the same checks.

In this example, we create a mutable string containing a name, then take a reference to the first three bytes of the
name. While that reference is outstanding, we attempt to mutate the string by clearing it. There's now no guarantee that
the reference points to valid data and dereferencing it could lead to undefined behavior, so the compiler stops us:

`fn no_mutable_aliasing() {
    let mut name = String::from("Vivian");
    let nickname = &name[..3];
    name.clear();
    println!("Hello there, {}!", nickname);
}`

`error[E0502]: cannot borrow `name` as mutable because it is also borrowed as immutable
 --> a.rs:4:5
  |
3 |     let nickname = &name[..3];
  |                     ---- immutable borrow occurs here
4 |     name.clear();
  |     ^^^^^^^^^^^^ mutable borrow occurs here
5 |     println!("Hello there, {}!", nickname);
  |                                  -------- immutable borrow later used here

For more information about this error, try `rustc --explain E0502`.`

Helpfully, the error message incorporates our code and tries its hardest to explain the problem, pointing out exact
locations.

Prototyping solutions in Rust can be challenging due to its statically-typed nature and because Rust requires covering
100% of the conditions, not just 99%. Edge cases must have applicable code, even when the programmer doesn't yet know
what the happy path should do. During early development, these edge cases can often be addressed by causing the program
to crash, and then rigorous error handling can be added at a later point. This is a different workflow than in languages
such as Ruby, where developers often try out code in a REPL and then move that to a prototype without considering error
cases at all.

Rust is still relatively new, which means that some desired libraries may not be available yet. The upside is there's
plenty of fertile ground to develop these needed libraries, perhaps even taking advantage of recent developments in the
relevant computer science fields. Due to this and Rust's capabilities, some of Rust's libraries, such as the [regex][44]
crate, are the best-in-breed across any language.

While Rust has a strong commitment to stability and backwards compatibility, that doesn't imply the language is
finalized. A specific problem may not have access to language features that would make it simpler to express or perhaps
even possible to express. As an example, Rust has had asynchronous futures for over three years, but stable [`async` /
`await`][45] support in the language itself is only a few months old.

The Rust compiler is built on top of [LLVM][46], which means that the number of target platforms will be smaller than C
or C++.

## Come and join us!

Regardless of which programming languages you love now, there's bound to be something that excites or intrigues you
about Rust. These are some of the reasons why I and others love Rust so much, and there's many more. If you are looking
for extra structure in your project, faster or more efficient code, or the ability to write performant code more quickly
and safely, it's time for you to see if Rust will be *your* next most-loved language!

Authors
[
Jake Goulding
[Image of Jake Goulding]
][47]
[Bulletin][48][Code for a Living][49][programming][50][rust][51][Stackoverflow][52]
Recent articles
June 26, 2026[

# Paging Charity! How can engineering leaders avoid becoming Bond villains?

][53]
June 23, 2026[

# Your AI shipped a backend that boots. That is the whole problem.

][54]
June 23, 2026[

# The 2026 Developer Survey is now open (for human developers only)!

][55]
June 19, 2026[

# Dispatches from O'Reilly: From capabilities to responsibilities

][56]
Latest Podcast
June 25, 2026[

# Code isn’t the only thing causing your production failures

][57]

## Add to the discussion

Login with your **stackoverflow.com** account to take part in the discussion.

### Our Stack
* [Stack Internal][58]
  * [Features][59]
  * [Customers][60]
  * [Use cases][61]
  * [Security][62]
  * [Integrations][63]
  * [Pricing][64]
* [Stack Data Licensing][65]
* [Stack Ads][66]
* [Partnerships][67]
* [Services][68]
* [Stack Overflow][69]

### Company
* [Leadership][70]
* [Press][71]
* [Careers][72]
* [Social Impact][73]

### Support
* [Contact][74]
* [Stack Overflow help][75]
* [Stack Internal help][76]
* [Implement Stack Internal][77]
* [Terms][78]
* [Privacy policy][79]
* [Cookie policy][80]
* Your Privacy Choices

### Elsewhere
* [Blog][81]
* [Dev Newsletter][82]
* [Podcast][83]
* [Releases][84]
* [Dev Survey][85]

Site design / logo © 2026 Stack Exchange Inc.
LightDarkAuto

[1]: /
[2]: https://take.survey.stackoverflow.co/jfe/form/SV_4GHunpL3IfJ3rRc?utm_medium=referral&utm_source=blog&utm_campaign=d
ev-survey-2026&utm_content=announcement-banner-survey
[3]: /
[4]: /contributed/
[5]: /productivity/
[6]: /ai/
[7]: /open-source/
[8]: /business/
[9]: /company/
[10]: /releases/
[11]: /podcast/
[12]: /newsletter/
[13]: https://stackoverflow.co/
[14]: https://stackoverflow.co/internal/?utm_source=so-owned&utm_medium=blog&utm_campaign=nav-side-bar
[15]: https://stackoverflow.co/data-licensing/?utm_source=so-owned&utm_medium=blog&utm_campaign=nav-side-bar
[16]: https://stackoverflow.co/advertising/employer-branding/?utm_source=so-owned&utm_medium=blog&utm_campaign=nav-side-
bar
[17]: https://insights.stackoverflow.com/survey/2019
[18]: https://franklinchen.com/blog/2012/09/06/my-pittsburgh-ruby-talk-nil/
[19]: https://www.tilde.io/
[20]: https://www.skylight.io/
[21]: https://www.rust-lang.org/static/pdfs/Rust-Tilde-Whitepaper.pdf
[22]: https://viruta.org/librsvg-is-almost-rustified-now.html
[23]: https://os.phil-opp.com/
[24]: https://docs.rust-embedded.org/discovery/
[25]: http://cliffle.com/p/dangerust/
[26]: https://raphlinus.github.io/programming/rust/2018/08/17/undefined-behavior.html
[27]: https://www.youtube.com/watch?v=A3AdN7U24iU
[28]: https://rustup.rs/
[29]: https://crates.io/
[30]: https://docs.rs/
[31]: https://github.com/rust-lang/rust-clippy
[32]: https://github.com/rust-lang/rustfmt
[33]: https://discord.gg/rust-lang
[34]: https://users.rust-lang.org/
[35]: https://www.reddit.com/r/rust/
[36]: https://stackoverflow.com/questions/tagged/rust
[37]: https://chat.stackoverflow.com/rooms/62927/rust
[38]: https://www.rust-lang.org/policies/code-of-conduct
[39]: https://rustconf.com/
[40]: https://rust-belt-rust.com/
[41]: https://blog.rustfest.eu/
[42]: https://rustlatam.org/
[43]: https://rustcon.asia/
[44]: https://github.com/rust-lang/regex
[45]: https://blog.rust-lang.org/2019/11/07/Async-await-stable.html
[46]: https://llvm.org/
[47]: /author/jgoulding/
[48]: /bulletin/
[49]: /code-for-a-living/
[50]: /programming/
[51]: /rust/
[52]: /stackoverflow/
[53]: /2026/06/26/paging-charity-how-can-engineering-leaders-avoid-becoming-bond-villains/
[54]: /2026/06/23/your-ai-shipped-a-backend-that-boots-that-is-the-whole-problem/
[55]: /2026/06/23/the-2026-developer-survey-is-now-open-for-human-developers-only/
[56]: /2026/06/19/dispatches-from-o-reilly-from-capabilities-to-responsibilities/
[57]: /2026/06/25/code-isnt-causing-your-production-failures/
[58]: https://stackoverflow.co/internal/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[59]: https://stackoverflow.co/internal/features/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[60]: https://stackoverflow.co/internal/customers/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[61]: https://stackoverflow.co/internal/uses/ai?utm_source=blog&utm_medium=referral&utm_campaign=footer
[62]: https://stackoverflow.co/internal/security/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[63]: https://stackoverflow.co/internal/integrations/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[64]: https://stackoverflow.co/internal/pricing/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[65]: https://stackoverflow.co/data-licensing/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[66]: https://stackoverflow.co/advertising/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[67]: https://stackoverflow.co/partnerships/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[68]: https://stackoverflow.co/internal/services/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[69]: https://stackoverflow.com/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[70]: https://stackoverflow.co/company/leadership/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[71]: https://stackoverflow.co/company/press/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[72]: https://stackoverflow.co/company/careers/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[73]: https://stackoverflow.co/company/social-impact/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[74]: https://stackoverflow.com/contact/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[75]: https://stackoverflow.com/help/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[76]: https://support.stackenterprise.co/support/solutions/22000023964
[77]: https://stackoverflow.co/internal/implement/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[78]: https://policies.stackoverflow.co/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[79]: https://stackoverflow.com/legal/privacy-policy/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[80]: https://stackoverflow.com/legal/cookie-policy/?utm_source=blog&utm_medium=referral&utm_campaign=footer
[81]: https://stackoverflow.blog/
[82]: https://stackoverflow.blog/newsletter/
[83]: https://stackoverflow.blog/podcast/
[84]: https://stackoverflow.blog/releases/
[85]: https://survey.stackoverflow.co/
```
