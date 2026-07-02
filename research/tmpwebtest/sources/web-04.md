# Web source

- URL: https://www.reddit.com/r/rust
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T00:55:19.380607309+00:00

```text
[ Skip to main content ][1]
Open menu Open navigation [ ][2]Go to Reddit Home
[ Sign Up ][3]Sign up for Reddit [ Log In ][4]Log in to Reddit
Expand user menu Open settings menu

# r/rust

Create Post
[ Feed ][5] [ About ][6]
Best
Open sort options

[ Best ][7]

[ Hot ][8]

[ New ][9]

[ Top ][10]

[ Rising ][11]

Change post view

[ Card ][12]

[ Compact ][13]

### Community highlights

[

## This Week in Rust #657

Announcement votes • comments ][14]

[

## Hey Rustaceans! Got a question? Ask here (26/2026)!

[llogiq] votes • comments ][15]

[

## What's everyone working on this week (26/2026)?

[llogiq] votes • comments ][16]

[

## Official /r/rust "Who's Hiring" thread for job-seekers and job-offerers [Rust 1.96]

Megathread votes • comments ][17]

[ Ante: A New Way to Blend Borrow Checking and Reference Counting ][18]
[
[u/verdagon avatar] u/verdagon
][19]
•
[ Ante: A New Way to Blend Borrow Checking and Reference Counting ][20]
[https://verdagon.dev/blog/ante-blending-borrowing-rc][21]
[ Introducing Test That!: A powerful test assertion library for Rust ][22]
[
[u/hovinen avatar] u/hovinen
][23]
• [ Introducing Test That!: A powerful test assertion library for Rust ][24] [

Test That! lets you write test assertions which precisely specify your *intent*:

let vec = vec![5, 123, -4];
assert_that!(vec, each(gt(0)));

and get informative, meaningful diagnostics when the tests fail:

Value of: vec
Expected: only contains elements that is greater than 0
Actual: [5, 123, -4],
  whose element #2 is -4, which is less than or equal to 0

I've recently published this new crate as a fork of GoogleTest Rust, which I had spearheaded a few years ago while I was
at Google. Test That! has a much improved developer experience compared with the original. See my blog post (linked
below) for more details.

I'm really keen on getting feedback on this crate. Please give it a try and let me know your experiences!
* Crate: [https://crates.io/crates/test-that][26]
* GitHub: [https://github.com/hovinen/test-that][27]
* Blog post announcement: [https://hovinen.me/announcements/2026/06/24/introducing-test-that.html][28]

][28]
[ calybris-core: Proof-carrying decision engine — 115ns/decision, Loom + Miri + proptest, async WAL,
#![forbid(unsafe_code)] ][29]
[
[u/Right-Might8576 avatar] u/Right-Might8576
][30]
• [ calybris-core: Proof-carrying decision engine — 115ns/decision, Loom + Miri + proptest, async WAL,
#![forbid(unsafe_code)] ][31] [
🛠️ project
][32] [

Calybris Core is a deterministic decision engine for systems where every routing decision must be explainable,
replayable, and cryptographically verifiable.

You define models with cost/quality/latency, send a request with budget and risk constraints, and get back a decision +
proof. Same input always produces the same output. If you need to prove why model X was selected over model Y six months
later, the proof is there.

Built-in financial layer: CAS atomic budget engine with per-tenant reserve/commit/release, exposure caps, conservation
invariant (remaining + reserved + committed = initial), and fixed-point i64 microcent arithmetic — no floating point in
the money path.

Primary use case is LLM model routing, but it's domain-neutral — works for anything shaped like candidates + constraints
→ decision + proof.

#![forbid(unsafe_code)], HMAC-SHA256 tamper-evident WAL, Loom + Miri in CI.

cargo add calybris-core

[https://github.com/emirhuseynrmx/calybris-core][34]

[https://crates.io/crates/calybris-core][35]

[https://docs.rs/calybris-core][36]

][36]
Created Dec 2, 2010
Public

Anyone can view, post, and comment to this community

171K 3.6K

## Community Bookmarks

Megathreads

[
Alternative Venues
][37]

[
Official Blog Posts
][38]

[
Rust Foundation Blog
][39]

[
Got a Question?
][40]

[
What Are You Up To?
][41]

[
Who's Hiring?
][42]

[
This Week in Rust
][43]

Official Resources

[
Official Website
][44]

[
Official Blog
][45]

[
This Week In Rust
][46]

[
Installers
][47]

[
Source Code
][48]

[
Bug Tracker
][49]

Learn Rust

[
The Rust E-Book
][50]

[
Stdlib API Reference
][51]

[
Rust By Example
][52]

[
Rustlings
][53]

[
Online Playground
][54]

Discussion Platforms

[
Official Users Forum
][55]

[
Community Discord
][56]

[
Mozilla Matrix Chat
][57]

[
Stack Overflow Chat
][58]

## Pic

## r/rust Rules

1

## Observe our code of conduct

Strive to treat others with respect, patience, kindness, and empathy.

We observe [the Rust Project Code of Conduct][59].

[Details][60]

2

## Submissions must be on-topic

Posts must reference Rust or relate to things using Rust. For content that does not, use a text post to explain its
relevance.

Post titles should include useful context.

For Rust questions, use the stickied Q&A thread.

Arts-and-crafts posts are permitted [on weekends][61].

No meta posts; [message the mods][62] instead.

[Details][63]

3

## Constructive criticism only

Criticism is encouraged, though it must be constructive, useful and actionable.

Avoid posting links to web pages which allow commenting, such as Twitter, or Github projects/issues when criticizing
them. Please [create a read-only mirror][64] and link that instead.

[Details][65]

4

## Keep things in perspective

A programming language is rarely worth getting worked up over.

No zealotry or fanaticism.

Be charitable in intent. Err on the side of giving others the benefit of the doubt.

[Details][66]

5

## No endless re-litigation

Avoid re-treading topics that have been long-settled or utterly exhausted.

Avoid bikeshedding.

This is not an official Rust forum, and cannot fulfill feature requests. Use the official venues for that.

[Details][67]

6

## No low-effort content

No memes, image macros, etc.

No slop, whether automatically generated or not.

Consider the existing content of the subreddit and whether your post fits in. Does it inspire thoughtful discussion?

Use text for code or error messages, not screenshots.

Submissions appearing to contain AI-generated content may be removed at moderator discretion.

[Details][68]
* [Home][69]
* [Popular][70]
* [News][71]
* [Explore][72]
* [Best of Reddit][73]
* [Best of Reddit in Portuguese][74]
* [Best of Reddit in German][75]
* [Reddit Rules][76]
* [Privacy Policy][77]
* [User Agreement][78]
* [Accessibility][79]
* [Reddit, Inc. © 2026. All rights reserved.][80]

Join the most real place on the internet

Continue with Phone Number
Continue with Email

By continuing, you agree to our [User Agreement][81] and acknowledge that you understand the [Privacy Policy][82].

[1]: #main-content
[2]: https://www.reddit.com/
[3]: https://www.reddit.com/register/
[4]: https://www.reddit.com/login/
[5]: /r/rust
[6]: /r/rust/about/
[7]: /r/rust/best/
[8]: /r/rust/hot/
[9]: /r/rust/new/
[10]: /r/rust/top/
[11]: /r/rust/rising/
[12]: ?feedViewType=cardView
[13]: ?feedViewType=compactView
[14]: /r/rust/comments/1uewqig/this_week_in_rust_657/
[15]: /r/rust/comments/1ucdjab/hey_rustaceans_got_a_question_ask_here_262026/
[16]: /r/rust/comments/1ucdk53/whats_everyone_working_on_this_week_262026/
[17]: /r/rust/comments/1ttbtf5/official_rrust_whos_hiring_thread_for_jobseekers/
[18]: https://www.reddit.com/r/rust/comments/1ui2u0x/ante_a_new_way_to_blend_borrow_checking_and/
[19]: https://www.reddit.com/user/verdagon/
[20]: https://www.reddit.com/r/rust/comments/1ui2u0x/ante_a_new_way_to_blend_borrow_checking_and/
[21]: https://verdagon.dev/blog/ante-blending-borrowing-rc
[22]: https://www.reddit.com/r/rust/comments/1uhuhsb/introducing_test_that_a_powerful_test_assertion/
[23]: https://www.reddit.com/user/hovinen/
[24]: https://www.reddit.com/r/rust/comments/1uhuhsb/introducing_test_that_a_powerful_test_assertion/
[25]: /r/rust/comments/1uhuhsb/introducing_test_that_a_powerful_test_assertion/
[26]: https://crates.io/crates/test-that
[27]: https://github.com/hovinen/test-that
[28]: https://hovinen.me/announcements/2026/06/24/introducing-test-that.html
[29]: https://www.reddit.com/r/rust/comments/1uic1pt/calybriscore_proofcarrying_decision_engine/
[30]: https://www.reddit.com/user/Right-Might8576/
[31]: https://www.reddit.com/r/rust/comments/1uic1pt/calybriscore_proofcarrying_decision_engine/
[32]: /r/rust/?f=flair_name%3A%22%F0%9F%9B%A0%EF%B8%8F%20project%22
[33]: /r/rust/comments/1uic1pt/calybriscore_proofcarrying_decision_engine/
[34]: https://github.com/emirhuseynrmx/calybris-core
[35]: https://crates.io/crates/calybris-core
[36]: https://docs.rs/calybris-core
[37]: https://www.reddit.com/r/rust/comments/14921t7/alternative_rust_discussion_venues/
[38]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%93%A1%20official%20blog%22
[39]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%AB%B1%F0%9F%8F%BB%E2%80%8D%F0%9F%AB%B2%F0%9F%8F%BE%20found
ation%22
[40]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%99%8B%20questions%20megathread%22
[41]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%90%9D%20activity%20megathread%22
[42]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%92%BC%20jobs%20megathread%22
[43]: https://new.reddit.com/r/rust/?f=flair_name%3A%22%F0%9F%93%85%20this%20week%20in%20rust%22
[44]: https://www.rust-lang.org
[45]: https://blog.rust-lang.org/
[46]: https://this-week-in-rust.org/
[47]: https://www.rust-lang.org/install.html
[48]: https://github.com/rust-lang/
[49]: https://github.com/rust-lang/rust/issues
[50]: https://doc.rust-lang.org/book/
[51]: https://doc.rust-lang.org/std/index.html
[52]: https://doc.rust-lang.org/rust-by-example/index.html
[53]: https://github.com/rust-lang/rustlings
[54]: https://play.rust-lang.org/
[55]: https://users.rust-lang.org/
[56]: https://discord.gg/rust-lang-community
[57]: https://chat.mozilla.org/#/room/#rust:mozilla.org
[58]: https://stackoverflow.com/questions/tagged/rust
[59]: https://www.rust-lang.org/policies/code-of-conduct
[60]: https://reddit.com/r/rust/wiki/rules#wiki_1._observe_our_code_of_conduct
[61]: https://mrmonday.github.io/craft-time/
[62]: https://reddit.com/message/compose/?to=/r/rust
[63]: https://reddit.com/r/rust/wiki/rules#wiki_2._submissions_must_be_on-topic
[64]: https://archive.org/web/
[65]: https://reddit.com/r/rust/wiki/rules#wiki_3._constructive_criticism_only
[66]: https://reddit.com/r/rust/wiki/rules#wiki_4._keep_things_in_perspective
[67]: https://reddit.com/r/rust/wiki/rules#wiki_5._no_endless_re-litigation
[68]: https://reddit.com/r/rust/wiki/rules#wiki_6._no_low-effort_content
[69]: /?feed=home
[70]: /r/popular/
[71]: /news/
[72]: /explore/
[73]: https://www.reddit.com/posts/2026/global/
[74]: https://www.reddit.com/posts/2026/tl-pt-BR/
[75]: https://www.reddit.com/posts/2026/tl-de/
[76]: https://www.redditinc.com/policies/content-policy
[77]: https://www.reddit.com/policies/privacy-policy
[78]: https://www.redditinc.com/policies/user-agreement
[79]: https://support.reddithelp.com/hc/sections/38303584022676-Accessibility
[80]: https://redditinc.com
[81]: https://www.redditinc.com/policies/user-agreement
[82]: https://www.redditinc.com/policies/privacy-policy
```
