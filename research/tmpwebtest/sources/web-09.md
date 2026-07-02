# Web source

- URL: https://www.infoworld.com/article/2255250/what-is-rust-safe-fast-and-easy-software-development.html
- Title: 1. Topics
- Captured (UTC): 2026-06-29T00:55:52.439292188+00:00

```text
1. Topics
2. [Latest][1]
3. [Newsletters][2]
4. [Resources][3]
5. [Buyer’s Guides][4]
6. [Events][5]
[ Search ][6]
Menu

## Topics

Close
1.  [Analytics][7]
2.  [Artificial Intelligence][8]
3.  [Careers][9]
4.  [Cloud Computing][10]
5.  [Data Management][11]
6.  [Databases][12]
7.  [Development Tools][13]
8.  [Devops][14]
9.  [Emerging Technology][15]
10. [Enterprise Buyer’s Guides][16]
11. [Generative AI][17]
12. [IT Leadership][18]
13. [Java][19]
14. [JavaScript][20]
15. [Microsoft .NET][21]
16. [Open Source][22]
17. [Programming Languages][23]
18. [Python][24]
19. [Security][25]
20. [Software Development][26]
21. [Technology Industry][27]
Back
Close
[ Search ][28]
Topics
[Latest][29]
[Newsletters][30]
[Resources][31]
[Buyer’s Guides][32]
More
1. [Features][33]
2. [Blogs][34]
3. [BrandPosts][35]
4. [Videos][36]
Topics
Topics
1.  [Analytics][37]
2.  [Artificial Intelligence][38]
3.  [Careers][39]
4.  [Cloud Computing][40]
5.  [Data Management][41]
6.  [Databases][42]
7.  [Development Tools][43]
8.  [Devops][44]
9.  [Emerging Technology][45]
10. [Enterprise Buyer’s Guides][46]
11. [Generative AI][47]
12. [IT Leadership][48]
13. [Java][49]
14. [JavaScript][50]
15. [Microsoft .NET][51]
16. [Open Source][52]
17. [Programming Languages][53]
18. [Python][54]
19. [Security][55]
20. [Software Development][56]
21. [Technology Industry][57]
1. [ Home ][58]
2. [ Software Development ][59]
[Serdar Yegulalp]
by [Serdar Yegulalp][60]
Senior Writer

# What is Rust? Safe, fast, and easy software development

feature
Nov 20, 202411 mins

## Unlike most programming languages, Rust doesn't make you choose between speed, safety, and ease of use. Find out how
## Rust delivers better code with fewer compromises, and a few downsides to consider before learning Rust.

[rust king iron bronze crown royal queen]
Credit: [Gratisography][61]

A programming language can be fast, safe, or easy to write. As developers, we get to choose our priorities but we can
only pick two. Programming languages that emphasize convenience and safety tend to be slow (like [Python][62]).
Languages that emphasize performance tend to be difficult to use and quick to blow things up (like [C][63] and C++).
That has been the state of software development for a good long time now.

Is it possible to deliver speed, safety, and ease of use in a single language? The Rust language, originally created by
Graydon Hoare and currently sponsored by Google, Microsoft, Mozilla, Arm, and others, attempts to bring together these
three attributes in one language. (Google’s [Go language][64] has [similar ambitions][65], but Rust aims to make fewer
concessions along the way.)

### Related video: Developing safer software with Rust

Get up to speed quickly on the Rust language, designed to create fast, system-level software. This two-minute animated
explainer shows how Rust bypasses the thorny issues of memory and management.

Rust is meant to be [fast, safe][66], and reasonably easy to use. It’s also intended to be used widely, and not simply
end up as a curiosity or an also-ran in the [programming language sweepstakes][67]. Good reasons abound for creating a
language where safety sits on equal footing with speed and development power. After all, there’s a tremendous amount of
software—some of it driving critical infrastructure—built with [languages that did not put safety first][68].

## Rust language advantages

Rust started as a Mozilla research project partly meant to [reimplement key components of the Firefox browser][69]. The
project’s priorities were driven by the need to make better use of multicore processors in Firefox, and the sheer
ubiquity of web browsers meant that they must be safe to use.

But it turns out *all software* needs to be fast and safe, not just browsers. So, Rust evolved from its origins as a
browser component project into a full-blown language project.

This article is a quick look at the key characteristics that make Rust an increasingly popular language for developers
seeking an alternative to the status quo. We’ll also consider some of the downsides to adopting Rust.

### Rust is fast

Rust code compiles to native machine code across multiple platforms. Binaries are self-contained, with no external
runtime apart from what the operating system might provide, and the generated code is meant to perform as well as
comparable code written in C or C++.

### Rust is memory-safe

Rust won’t compile programs that attempt unsafe memory usage.

In other languages, many classes of memory errors are discovered when a program is running. Rust’s syntax and language
metaphors ensure that common memory-related problems in other languages—null or dangling pointers, data races, and so
on—never make it into production. [The Rust compiler flags those issues and forces them to be fixed][70] before the
program ever runs.

### Rust features low-overhead memory management

Rust controls memory management via strict rules. Rust’s memory-management system is expressed in the language’s syntax
through a metaphor called *ownership*. Any given value in the language can be “owned,” or held and manipulated, only by
a single variable at a time. Every bit of memory in a Rust program is tracked and released automatically through the
ownership metaphor.

The way ownership is transferred between objects is strictly governed by the compiler, so there are no surprises at
runtime in the form of memory-allocation errors. The ownership approach also means that Rust does not require
[garbage-collected memory management][71], as in languages like Go or C#. (That also gives Rust another performance
boost.)

### Rust’s safety model is flexible

Rust lets you live dangerously, up to a point. [Rust’s safeties can be partly suspended][72] where you need to
manipulate memory directly, such as dereferencing a raw pointer *à la* C/C++. The key word here is *partly*, because
Rust’s memory safety operations can never be completely disabled. Even then, you almost never have to take off the
seatbelts for common use cases, so the end result is software that’s safer by default.

### Rust is cross-platform

Rust works on all three major platforms: Linux, Windows, and macOS. Others are supported beyond those three. If you want
to *cross-compile*, or produce binaries for a different architecture or platform than the one you’re currently running,
[some additional work is involved][73]. However, one of Rust’s general missions is to minimize the amount of heavy
lifting needed for such work. Also, although Rust works on the majority of current platforms, its creators are not
trying to have Rust compile everywhere—just on whatever platforms are popular, and wherever they don’t have to make
unnecessary compromises to the language to do so.

### Rust is easy to deploy

None of Rust’s safety and integrity features add up to much if they aren’t used. That’s why Rust’s developers and
community have tried to make the language as useful and welcoming as possible to both newcomers and experienced
developers.

Everything needed to produce Rust binaries comes in the same package. You only need external compilers like GCC if you
are compiling other components outside the Rust ecosystem (such as a C library that you’re compiling from source).
Windows users are not second-class citizens here, either; the Rust toolchain is as capable on Windows as it is on Linux
and macOS.

### Rust has powerful language features

Few developers want to start work in a new language if they find it has fewer or weaker features than the ones they’re
already using. Rust’s [native language features][74] compare favorably to what languages like C++ have: Macros,
generics, pattern matching, and composition (via “[traits][75]”) are all first-class citizens in Rust. Some features
found in other languages, like [inline asembler][76], are also available, albeit under Rust’s “unsafe” label.

### Rust has a useful standard library

A part of Rust’s larger mission is to encourage C and C++ developers to use Rust instead of those languages whenever
possible. But C and C++ users expect to have a decent standard library—they want to be able to use containers,
collections, and iterators, perform string manipulations, manage processes and threading, perform network and file I/O,
and so on. Rust does all that, and more, in its [standard library][77]. Because Rust is designed to be cross-platform,
its standard library can contain only things that can be reliably ported across platforms. Platform-specific functions
like Linux’s epoll have to be supported via functions in third-party libraries such as [libc][78], [mio][79], or
[tokio][80].

It is also possible to use Rust [without its standard library][81]. One common reason to do so is to build binaries that
have no platform dependencies — e.g., an embedded system or an OS kernel.

### Rust has many third-party libraries, or ‘crates’

A measure of a language’s utility is how much can be done with it thanks to third parties. [Cargo][82], the official
repository for Rust libraries (called “crates”) lists some 60,000-plus crates. A healthy number of them are API bindings
to common libraries or frameworks, so Rust can be used as a viable language option with those frameworks. However, the
Rust community does not yet supply detailed curation or ranking of crates based on their overall quality and utility, so
you can’t tell what works well without trying things yourself or polling the community.

### Rust has strong IDE support

Again, few developers want to embrace a language with little or no support in the IDE of their choice. That’s why the
Rust team created the [Rust Language Server][83], which provides live feedback from the Rust compiler to IDEs such as
[Microsoft Visual Studio Code][84].

[[rust visual studio code]][85]

Live feedback in Visual Studio Code from the Rust Language Server. The Rust Language Server offers more than basic
syntax checking; it also determines things like variable use.

## Downsides of programming with Rust

Along with all of its attractive, powerful, and useful capabilities, Rust has its downsides. Some of these hurdles trip
up new “rustaceans” (as Rust fans call each other) and old hands alike.

### Rust is a young language

Rust is still a young language, having delivered its 1.0 version only in 2015. So, while much of the core language’s
syntax and functionality has been [hammered down][86], a great many other things around it are still fluid.
[Asynchronous operations][87], for example, are [still a work in progress][88] in Rust. Some parts of async are more
mature than others, and many parts are provided via [third-party components][89].

### Rust is difficult to learn

If any one thing about Rust is most problematic, it’s [how difficult it can be to grok Rust’s metaphors][90]. Ownership,
borrowing, and Rust’s other memory management conceits trip *everyone* up the first time. A common rite of passage for
newbie Rust programmers is fighting the borrow checker, where they discover firsthand how meticulous the compiler is
about keeping mutable and immutable things separate.

### Rust is complex

Some of the difficulty of learning Rust comes from how its metaphors make for more verbose code, especially compared to
other languages. For example, string concatenation in Rust isn’t always as straightforward as `string1+string2`. One
object might be mutable and the other immutable. Rust is inclined to insist that the programmer spell out how to handle
such things, rather than let the compiler guess.

Another example is how Rust and C/C++ work together. Much of the time, Rust is used to plug into existing libraries
written in C or C++; few projects in C and C++ are rewritten from scratch in Rust. (And when they are, they tend to be
rewritten [incrementally][91].)

### Rust is a systems language

Like C and C++, Rust can be used to write systems-level software, since it allows direct manipulation of memory. But for
some jobs, that’s overkill. If you have a task that is mainly I/O-bound, or doesn’t need machine-level speed, Rust might
be an ungainly choice. A Python script that takes five minutes to write and one second to execute is a better choice for
the developer than a Rust program that takes half an hour to write and a hundredth of a second to run.

## The future of Rust

The Rust team is conscious of many of its issues and is working to improve them. For example, to make it easier for Rust
to work with C and C++, the Rust team is investigating whether to expand projects like [bindgen][92], which
automatically generates Rust bindings to C code. The team also has plans to make borrowing and lifetimes more flexible
and easier to understand.

Still, Rust succeeds in its goal to provide a safe, concurrent, and practical systems language, in ways other languages
don’t, and to do it in ways that complement how developers already work.

[Software Development][93][Programming Languages][94][Rust][95]

## Related content

[ analysis

### Making Windows a developer platform, again

By Simon Bisson
Jun 25, 2026 8 mins
Development Tools Software Development ][96] [ feature

### Building a state-of-the-art development platform with Backstage

By Sameera Jayasoma
Jun 25, 2026 18 mins
Development Tools Software Development ][97] [ news

### AI coding token costs are on track to rival human payroll

By Taryn Plumb
Jun 24, 2026 6 mins
Artificial Intelligence Software Development ][98] [ opinion

### Open source grapples with agentic coding

By Nick Hodges
Jun 24, 2026 4 mins
Artificial Intelligence Generative AI Open Source ][99]

## Other Sections
* [ Resources ][100]
* [ Videos ][101]

[Serdar Yegulalp]
by [ Serdar Yegulalp ][102]
Senior Writer
1. [ Follow Serdar Yegulalp on X ][103]

Serdar Yegulalp is a senior writer at [InfoWorld][104]. A veteran technology journalist, Serdar has been writing about
computers, operating systems, databases, programming, and other information technology topics for 30 years. Before
joining InfoWorld in 2013, Serdar wrote for Windows Magazine, InformationWeek, Byte, and a slew of other publications.
At InfoWorld, Serdar has covered software development, devops, containerization, machine learning, and artificial
intelligence, winning several B2B journalism awards including a 2024 [Neal Award][105] and a 2025 [Azbee Award][106] for
best instructional content and best how-to article, respectively. He currently focuses on software development tools and
technologies and major programming languages including Python, Rust, Go, Zig, and Wasm. Tune into his weekly [Dev with
Serdar][107] videos for programming tips and techniques and close looks at programming libraries and tools.

## More from this author
* [
  feature
  
  ### Using Visual Studio Code’s ‘air-gapped’ AI model mode
  
  Jun 24, 2026 7 mins
  ][108]
* [
  feature
  
  ### Write cleaner and faster Python code
  
  Jun 19, 2026 3 mins
  ][109]
* [
  feature
  
  ### How to use virtual environments in Python
  
  Jun 10, 2026 13 mins
  ][110]
* [
  feature
  
  ### Pyrefly 1.0: A fast, forward-looking Python linter
  
  Jun 3, 2026 6 mins
  ][111]
* [
  feature
  
  ### Plunge into Python profiling
  
  May 29, 2026 3 mins
  ][112]
* [
  feature
  
  ### Docker Sandboxes and microVMs, explained
  
  May 27, 2026 4 mins
  ][113]
* [
  feature
  
  ### First look: Mojo 1.0 mixes Python and Rust
  
  May 20, 2026 12 mins
  ][114]
* [
  feature
  
  ### First look: Lemonade serves up local AI with limitations
  
  May 13, 2026 5 mins
  ][115]

## Show me more

PopularArticlesVideos
[
news

### US tells OpenAI to restrict access to its most powerful AI model

By Maxwell Cooter
Jun 26, 20262 mins
Artificial Intelligence
[Image]
][116]
[
news

### pgEdge joins rush to merge OLTP and OLAP storage to support AI

By Anirban Ghoshal
Jun 26, 20266 mins
AnalyticsDatabasesTransaction Processing
[Image]
][117]
[
opinion

### Why private AI is the smarter bet

By David Linthicum
Jun 26, 20266 mins
Artificial IntelligenceData ArchitectureHybrid Cloud
[Image]
][118]
[
video

### 4 things that need to change about gen-AI

Jun 23, 20264 mins
Python
[Image]
][119]
[
video

### Gemma 4 runs general-purpose AI locally (and quickly)

Jun 18, 20265 mins
Python
[Image]
][120]
[
video

### A first look at Pyrefly 1.0

Jun 9, 20265 mins
Python
[Image]
][121]

### About
1. [About Us][122]
2. [Advertise][123]
3. [Contact Us][124]
4. [Editorial Ethics Policy][125]
5. [Foundry Careers][126]
6. [Newsletters][127]
7. [Reprints][128]
8. [Add InfoWorld as a Preferred Source in Google Search][129]

### Policies
1. [Terms of Service][130]
2. [Privacy Policy][131]
3. [Cookie Policy][132]
4. [Copyright Notice][133]
5. [Member Preferences][134]
6. [About AdChoices][135]
7. [Your California Privacy Rights][136]
8. Privacy Settings

### More
1. [News][137]
2. [Features][138]
3. [Blogs][139]
4. [BrandPosts][140]
5. [Events][141]
6. [Videos][142]
7. [Enterprise Buyer’s Guides][143]

### Our Network
1. [CIO][144]
2. [Computerworld][145]
3. [CSO][146]
4. [Network World][147]
* [Facebook][148]
* [X][149]
* [YouTube][150]
* [Google News][151]
* [LinkedIn][152]

[© 2026 FoundryCo, Inc. All Rights Reserved.][153]

[1]: https://www.infoworld.com/news/
[2]: /newsletters/signup/
[3]: https://us.resources.infoworld.com/
[4]: https://www.infoworld.com/enterprise-buyers-guide/
[5]: https://www.infoworld.com/events/
[6]: https://www.infoworld.com/search/
[7]: https://www.infoworld.com/analytics/
[8]: https://www.infoworld.com/artificial-intelligence/
[9]: https://www.infoworld.com/careers/
[10]: https://www.infoworld.com/cloud-computing/
[11]: https://www.infoworld.com/data-management/
[12]: https://www.infoworld.com/database/
[13]: https://www.infoworld.com/development-tools/
[14]: https://www.infoworld.com/devops/
[15]: https://www.infoworld.com/emerging-technology/
[16]: https://www.infoworld.com/enterprise-buyers-guide/
[17]: https://www.infoworld.com/generative-ai/
[18]: https://www.infoworld.com/it-leadership/
[19]: https://www.infoworld.com/java/
[20]: https://www.infoworld.com/javascript/
[21]: https://www.infoworld.com/microsoft-net/
[22]: https://www.infoworld.com/open-source/
[23]: https://www.infoworld.com/programming-languages/
[24]: https://www.infoworld.com/python/
[25]: https://www.infoworld.com/security/
[26]: https://www.infoworld.com/software-development/
[27]: https://www.infoworld.com/technology-business/
[28]: https://www.infoworld.com/search/
[29]: https://www.infoworld.com/news/
[30]: /newsletters/signup/
[31]: https://us.resources.infoworld.com/
[32]: https://www.infoworld.com/enterprise-buyers-guide/
[33]: https://www.infoworld.com/features/
[34]: https://www.infoworld.com/blogs/
[35]: https://www.infoworld.com/brandposts/
[36]: https://www.infoworld.com/videos/
[37]: https://www.infoworld.com/analytics/
[38]: https://www.infoworld.com/artificial-intelligence/
[39]: https://www.infoworld.com/careers/
[40]: https://www.infoworld.com/cloud-computing/
[41]: https://www.infoworld.com/data-management/
[42]: https://www.infoworld.com/database/
[43]: https://www.infoworld.com/development-tools/
[44]: https://www.infoworld.com/devops/
[45]: https://www.infoworld.com/emerging-technology/
[46]: https://www.infoworld.com/enterprise-buyers-guide/
[47]: https://www.infoworld.com/generative-ai/
[48]: https://www.infoworld.com/it-leadership/
[49]: https://www.infoworld.com/java/
[50]: https://www.infoworld.com/javascript/
[51]: https://www.infoworld.com/microsoft-net/
[52]: https://www.infoworld.com/open-source/
[53]: https://www.infoworld.com/programming-languages/
[54]: https://www.infoworld.com/python/
[55]: https://www.infoworld.com/security/
[56]: https://www.infoworld.com/software-development/
[57]: https://www.infoworld.com/technology-business/
[58]: https://www.infoworld.com/
[59]: https://www.infoworld.com/software-development/
[60]: https://www.infoworld.com/profile/serdar-yegulalp/
[61]: https://www.pexels.com/photo/rust-king-iron-bronze-2866/
[62]: https://www.infoworld.com/article/3204016/what-is-python-powerful-intuitive-programming.html
[63]: https://www.infoworld.com/article/3402023/why-the-c-programming-language-still-rules.html
[64]: https://www.infoworld.com/article/2253031/whats-the-go-language-really-good-for-3.html
[65]: https://www.infoworld.com/article/2262818/rust-vs-go-how-to-choose.html
[66]: https://www.infoworld.com/article/2514176/safety-off-programming-in-rust-with-unsafe.html
[67]: https://www.infoworld.com/article/3550526/rust-resumes-rise-in-popularity.html
[68]: https://www.infoworld.com/article/2336216/white-house-urges-developers-to-dump-c-and-c.html
[69]: https://www.infoworld.com/article/2252677/mozilla-binds-firefoxs-fate-to-the-rust-language.html
[70]: https://www.infoworld.com/article/2336661/rust-memory-safety-explained.html
[71]: https://www.infoworld.com/article/2337816/what-is-garbage-collection-automated-memory-management-for-your-programs
.html
[72]: https://www.infoworld.com/article/2514176/safety-off-programming-in-rust-with-unsafe.html
[73]: https://rust-lang.github.io/rustup/cross-compilation.html
[74]: https://www.infoworld.com/article/2266609/whats-new-in-the-rust-language.html
[75]: https://doc.rust-lang.org/rust-by-example/trait.html
[76]: https://doc.rust-lang.org/reference/inline-assembly.html
[77]: https://doc.rust-lang.org/std/
[78]: https://www.gnu.org/software/libc/
[79]: https://docs.rs/mio
[80]: https://tokio.rs/
[81]: https://docs.rust-embedded.org/book/intro/no-std.html
[82]: https://crates.io/
[83]: https://www.infoworld.com/article/2250660/mozillas-rust-goes-real-time-with-code-feedback.html
[84]: https://www.infoworld.com/article/2254808/get-started-with-visual-studio-code.html
[85]: https://legacy-us-images.foundryco.app/images/article/2021/10/rust-visual-studio-code-100905729-orig.jpg?auto=webp
&quality=85,70
[86]: https://blog.rust-lang.org/2014/10/30/Stability.html
[87]: https://rust-lang.github.io/async-book
[88]: https://rust-lang.github.io/rust-project-goals/2024h2/async.html
[89]: https://rust-lang.github.io/async-book/08_ecosystem/00_chapter.html
[90]: https://www.infoworld.com/article/2258065/rust-language-is-too-hard-to-learn-and-use-says-user-survey.html
[91]: https://www.infoworld.com/article/2254205/how-rust-can-replace-c-with-pythons-help.html
[92]: https://github.com/rust-lang-nursery/rust-bindgen
[93]: https://www.infoworld.com/software-development/
[94]: https://www.infoworld.com/programming-languages/
[95]: https://www.infoworld.com/rust/
[96]: https://www.infoworld.com/article/4188967/making-windows-a-developer-platform-again.html
[97]: https://www.infoworld.com/article/4189074/building-a-state-of-the-art-development-platform-with-backstage.html
[98]: https://www.infoworld.com/article/4189176/ai-coding-token-costs-are-on-track-to-rival-human-payroll-2.html
[99]: https://www.infoworld.com/article/4188440/open-source-grapples-with-agentic-coding.html
[100]: https://us.resources.infoworld.com/
[101]: https://www.infoworld.com/videos/
[102]: https://www.infoworld.com/profile/serdar-yegulalp/
[103]: https://www.twitter.com/syegulalp
[104]: https://www.infoworld.com/
[105]: https://siia.net/neals/
[106]: https://siia.net/neals/
[107]: https://www.youtube.com/playlist?list=PLYaGSokOr0MNgQlbnq_qY7BJs9-TOAxHA
[108]: https://www.infoworld.com/article/4186817/using-visual-studio-codes-air-gapped-ai-model-mode.html
[109]: https://www.infoworld.com/article/4186455/write-cleaner-and-faster-python-code.html
[110]: https://www.infoworld.com/article/2260103/how-to-use-virtual-environments-in-python.html
[111]: https://www.infoworld.com/article/4179383/pyrefly-1-0-a-fast-forward-looking-python-linter.html
[112]: https://www.infoworld.com/article/4178850/plunge-into-python-profiling.html
[113]: https://www.infoworld.com/article/4177309/docker-sandboxes-and-microvms-explained.html
[114]: https://www.infoworld.com/article/4173158/first-look-mojo-1-0-mixes-python-and-rust.html
[115]: https://www.infoworld.com/article/4169474/first-look-lemonade-serves-up-local-ai-with-limitations.html
[116]: https://www.infoworld.com/article/4190089/us-tells-openai-to-restrict-access-to-its-most-powerful-ai-model-2.html
[117]: https://www.infoworld.com/article/4190042/pgedge-joins-rush-to-merge-oltp-and-olap-storage-to-support-ai.html
[118]: https://www.infoworld.com/article/4189649/why-private-ai-is-the-smarter-bet.html
[119]: https://www.infoworld.com/video/4188406/4-things-that-need-to-change-about-gen-ai.html
[120]: https://www.infoworld.com/video/4186962/gemma-4-runs-general-purpose-ai-locally-and-quickly.html
[121]: https://www.infoworld.com/video/4182963/a-first-look-at-pyrefly-1-0.html
[122]: https://www.infoworld.com/about-us/
[123]: https://foundryco.com/our-brands/infoworld/
[124]: https://www.infoworld.com/contact-us/
[125]: https://www.infoworld.com/editorial-ethics-policy/
[126]: https://foundryco.com/work-here/
[127]: /newsletters/signup/
[128]: https://www.infoworld.com/contact-us/#republication-permissions
[129]: https://www.google.com/preferences/source?q=infoworld.com
[130]: https://foundryco.com/terms-of-service-agreement/
[131]: https://foundryco.com/privacy-policy/
[132]: https://foundryco.com/cookie-policy/
[133]: https://foundryco.com/copyright-notice/
[134]: https://www.infoworld.com/member-preferences/
[135]: https://foundryco.com/ad-choices/
[136]: https://foundryco.com/ccpa/
[137]: https://www.infoworld.com/news/
[138]: https://www.infoworld.com/features/
[139]: https://www.infoworld.com/blogs/
[140]: https://www.infoworld.com/brandposts/
[141]: https://www.infoworld.com/events/
[142]: https://www.infoworld.com/videos/
[143]: https://www.infoworld.com/enterprise-buyers-guide/
[144]: https://www.cio.com/
[145]: https://www.computerworld.com/
[146]: https://www.csoonline.com/
[147]: https://www.networkworld.com/
[148]: https://www.facebook.com/InfoWorld
[149]: https://twitter.com/infoworld
[150]: https://www.youtube.com/@InfoWorld
[151]: https://news.google.com/publications/CAAqIggKIhxDQkFTRHdvSkwyMHZNRFY1ZEhaNUVnSmxiaWdBUAE
[152]: https://www.linkedin.com/company/164364
[153]: https://foundryco.com/terms-of-service-agreement/
```
