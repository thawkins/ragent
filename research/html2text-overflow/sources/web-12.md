# Web source

- URL: https://offlinemark.com/int-overflow
- Title: # [offlinemark][1]
- Captured (UTC): 2026-06-29T16:21:17.709983093+00:00

```text
# [offlinemark][1]

## Life, art, and systems programming

Menu [Skip to content][2]
* [Home][3]
* [About][4]
* [Writing][5]
* [Favorites][6]
* [Subscribe][7]
* [Shop][8]
* [Links][9]
* [Now][10]
[[offlinemark]][11]

# WIP: Integers, safe arithmetic, and handling overflow (lab notes)

[Leave a reply][12]
January 9, 2026: [_Lab Notes 🧪][13], [C][14], [C++][15]

Some **rough** lab notes on how to do integer math with computers. Surprisingly deep topic! Usual caveat applies, this
is just what I’ve learned so far and there may be inaccuracies. This is mainly from a C/C++ perspective, though there is
a small discussion of Rust included.

It all starts with a simple prompt:

`return_type computeBufferSize(integer_type headerLen, integer_type numElements, integer_type elementSize)
{
  // We conceptually want to do: headerLen + (numElements * elementSize)
}`

How do we safely do something as simple as this arithmetic expression in C/C++? Furthermore, what type do we choose for
`integer_type` and `return_type`?

# Overflow
* You can’t just do `headerLen + (numElements * elementSize)` literally, because that can overflow for valid input
  arguments
* Why is overflow bad?
* Reason 1: Logic errors. The result of the expression will wrap around, producing incorrect values. At best they will
  make the program behave incorrectly, at medium they will crash the program, and at worst, they will be security
  vulnerabilities.
* Reason 2: Undefined behavior. This only applies to overflow on signed types. Signed overflow is UB, which means the
  fundamental soundness of the program is compromised, and it can behave unpredictably at runtime. Apparently this
  allows the compiler to perform useful optimizations, but can also cause critical miscompiles. (e.g. if guards later
  being compiled out of the program, causing security issues)
* (Note: Apparently there are compiler flags to force signed overflow to have defined behavior. `-fno-strict-overflow`
  and `-fwrapv`)

# How to handle overflow
* Ok, overflow is bad. What does one do about it?
* Even for the simplest operations of multiplication and addition, you need to be careful of how large the input
  operands can be, and whether overflow is possible.
* For application code, it might often be the case that numbers are so small that this is not a concern, but if you’re
  dealing with untrusted input, buffer sizes, or memory offsets, then you need to be more careful, as these numbers can
  be larger.
* In situations where you need to defend against overflow, you cannot just inline the whole arithmetic expression the
  natural way.
* You need to evaluate the expression operation by operation, at each step checking if overflow would occur, and failing
  the function if so.
* For example, here you’d first evaluate the multiplication in a checked fashion. And then if that succeeds, do the
  arithmetic.
* For addition `a + b`, the typical check for size_t looks like: `bool willOverflow = (a > SIZE_MAX - b);`
* For multiplication `a * b`, it looks like `bool willOverflow = (b != 0 && a > SIZE_MAX / b);`
* For signed ints, the checks are more involved. This is a reason to prefer unsigned types in situations where overflow
  checking is needed.

# Helper libraries

Manually adding inline overflow checking code everywhere is tedious, error prone, and harms readability. It would be
nice if one could just write the arithmetic and spend less time worrying about the subtleties of overflow.

GCC and Clang offer builtins for doing checked arithmetic. They also have the benefit of checking signed integer
overflow without UB.
* [https://clang.llvm.org/docs/LanguageExtensions.html#checked-arithmetic-builtins][16]

In practice, it seems like some industrial C/C++ projects use helper utilities to make overflow-safe programming more
ergonomic.
* Linux kernel: Has [overflow.h][17]
* Chromium: Has [base/numerics][18]

# When to think about overflow

So when coding, when do you actually need to care about this?

You do not literally have to use checked-arithmetic for every single addition or multiplication in the program. Checked
arithmetic clutters the code and can impact runtime performance, so you should only use it when it’s relevant.

It’s relevant when coding on the “boundary”. Arithmetic on untrusted or weakly trusted input data, e.g. from the
network, filesystem, hardware, etc. Or where you’re implementing an API to be called by external users.

But for many normal, everyday math operations in your code, you probably don’t need it. (Unless you are particularly
dealing with numbers so large that overflow is a possibility. So a certain amount of vigilance is needed.)

# Rust
* Rust has significant differences from C/C++ for its overflow behavior, which make it much less error prone.
* Difference 1 – All overflow is defined, unlike signed overflow being UB in C/C++. This prevents critical miscompiles
  if signed overflow happens.
* What about logic bugs where numbers wrap around and produce dangerously large values that are mishandled down the
  line?
* Rust **sometimes** protects against these.
* Rust protects against these in debug builds by injecting overflow checks, which panic if this happens. This impacts
  performance, but debug builds are slow anyway, and this helps find bugs faster in development.
* In release builds, wrap-around logic bugs are still possible. Rust prevents undefined behavior and memory unsafety,
  but incorrect arithmetic can still cause panics, allocation failures, or incorrect program behavior.
* Difference 2 – Rust has first-class support for checked arithmetic operations in the form of “methods” one can call on
  an integer. These are a clean and ergonomic way to safely handle overflow if that is relevant for code.

# Choosing an integer type

There are a lot of int types. Which do you choose?
* int
* unsigned int
* size_t
* ssize_t
* uint32_t, uint64_t
* off_t
* ptrdiff_t
* uintptr_t

Here’s my current understanding.

First, split off the specific-purpose types:
* off_t: (Signed) For file offsets
* ptrdiff_t: (Signed) for pointer differences
* uintptr_t: (Unsigned) For representing a pointer as an integer, in order to do math on it

Next, split off the explicit sized types: uint32_t, uint64_t, etc

These are mainly used when interfacing with external data: the network, filesystem, hardware, etc. (From C/C++! In Rust,
it’s idiomatic to use explicit sized types in more situations).

That mostly leaves int vs size_t.

This is hotly debated. Here’s my rule of thumb:
* For application code, dealing with “small” integers, counts: Use `int`. This lets you assert for non-negative. The
  downside is signed int overflow is UB, and thus dangerous, but the idea is that you should not be anywhere near
  overflowing since the integers are small. This is approximately the position of the [Google Style C++ guide][19].
* For “systems” code dealing with buffer sizes, offsets, raw memory: Use size_t. `int` is often too small for cases like
  this. size_t is “safer” in that overflow is defined. This is approximately the position of the [Chromium C++ style
  guide][20].

In the above code that’s computing the buffer size, there’s a few reasons to avoid int.

First of all, int might be too small, depending on the usage case (e.g. generic libraries).

Secondly, it’s risky to use `int` because it’s easier to invoke dangerous UB. Since `int` overflow is UB, you risk
triggering miscompiles (e.g. `if` guards being compiled out) if you accidentally overflow. This has caused many subtle
security bugs.

In general, reasoning about overflow is more complicated with `int` than with size_t, since size_t has fully defined
behavior.

However there are downsides of size_t. Notably, you can’t assert it as non-negative.

What about `unsigned int`?

Rule of thumb: Don’t use this. It’s a worse size_t because it’s smaller/undefined width, and a worse int because it
can’t be asserted for non-negative.

What about ssize_t?

This is a size_t that can also contain negative values for error codes.

It seems somewhat useful, but is also error-prone to use, as it must always be checked for negativity before use as a
size_t, otherwise you’ll have the classic issue where it produces a dangerously large unsigned variable.

In general, it seems like one should prefer explicit distinction between error state and integer state (e.g. bool
return, and size_t out parameter, or std::optional<size_t>) over this.

# Which types to choose for the example

The input arguments should be size_t, since we’re dealing with buffers, offsets, and raw memory.

The return value is a bit of a trick question. Any function like this must be able to return either a value, or an error
if overflow happened.

The core return value should be size_t. But then how to express error in case of overflow?

std::optional<size_t> is a good choice if using C++.

ssize_t is an option, if using C.

But changing the signature to return a bool, and then having an out size_t parameter might be even better, and is the
pattern used by the compiler builtins.

Related:
* [Google style guide][21]
* [Chromium style guide][22]
* [https://chromium.googlesource.com/chromium/src/+/main/docs/security/integer-semantics.md][23]
  * Mitigating Integer Overflow in C – Kees Cook, Google [https://www.youtube.com/watch?v=PLcZkgHCk90][24]
  * Vulns1001 06 IntegerOverflow 03 Prevention 01 SafeMath – [https://www.youtube.com/watch?v=QJ-LAhlLdsw][25]

## About Mark

Ten years into my journey towards becoming a pro systems programmer, sharing what I learn along the way. Also on
Twitter: [@offlinemark][26].

If you're reading this, I'd love to meet you. Please email [mark@offlinemark.com][27] and introduce yourself!

[ View all posts by Mark → ][28]

### Post navigation

[← osdev journal: The virtual memory unit test experiment][29] [Are you practicing, or just maintaining? →][30]

### Any thoughts?[Cancel reply][31]

### Most Popular
* [Linux Internals: How /proc/self/mem writes to unwritable memory][32]
* [Resources for learning systems programming][33]
* [WIP: What's the deal with memory ordering? (seq_cst, acquire, release, etc)][34]
* [x86 kernel development lab notes][35]
* [Libraries for freestanding C++][36]

### Favorites
* [You don’t even need to be successful][37]
* [Life is a business; life is a game; life is art][38]
* [Sometimes, you just need to be willing][39]
* [How to be happy][40]
* [Linux Internals: How /proc/self/mem writes to unwritable memory][41]
* [Double fetches, scheduling algorithms, and onion rings][42]
* [What they don’t tell you about demand paging in school][43]
* [How setjmp and longjmp work (2016)][44]

### Recent Posts
* [Libraries for freestanding C++][45]
* [What I learned from yoga][46]
* [How to sharpen the saw][47]
* [Devtools update 2026: Ghostty, just, flash.nvim, and more][48]
* [How to ask for feedback at work][49]

### Hear about new posts:

### RSS

Subscribe via [RSS][50]

### Categories
* [_Deep Dive 🔍][51] (4)
* [_Essay 📝][52] (32)
* [_Lab Notes 🧪][53] (16)
* [_Living Doc 🌱][54] (2)
* [_Micropost 🍪][55] (75)
* [_Tech 💻][56] (31)
* [_Twitter Archive 🐤][57] (28)
* [AI][58] (4)
* [Assembly][59] (7)
* [Blogging][60] (9)
* [Books][61] (2)
* [C][62] (9)
* [C++][63] (18)
* [Career][64] (20)
* [CMake][65] (4)
* [Computer Architecture][66] (5)
* [Computers][67] (4)
* [Concurrency][68] (3)
* [Creativity][69] (20)
* [Decision Making][70] (1)
* [Entrepreneurship][71] (12)
* [Favorite][72] (8)
* [Git][73] (2)
* [Language Learning][74] (1)
* [Legal][75] (1)
* [Life][76] (25)
* [Linux][77] (14)
* [Linux Kernel][78] (9)
* [macOS][79] (2)
* [Meta][80] (2)
* [Open Source][81] (2)
* [Operating Systems][82] (5)
* [Other][83] (19)
* [Poetry][84] (2)
* [Productivity][85] (6)
* [Software Development][86] (10)
* [Streaming][87] (6)
* [Web Dev][88] (2)
* [Writing][89] (13)

### Archives
* [June 2026][90] (1)
* [April 2026][91] (1)
* [March 2026][92] (2)
* [February 2026][93] (2)
* [January 2026][94] (5)
* [December 2025][95] (3)
* [August 2025][96] (2)
* [July 2025][97] (1)
* [June 2025][98] (5)
* [April 2025][99] (1)
* [March 2025][100] (1)
* [January 2025][101] (3)
* [November 2024][102] (6)
* [October 2024][103] (2)
* [September 2024][104] (1)
* [August 2024][105] (7)
* [June 2024][106] (1)
* [May 2024][107] (8)
* [April 2024][108] (4)
* [March 2024][109] (2)
* [February 2024][110] (2)
* [January 2024][111] (3)
* [December 2023][112] (19)
* [November 2023][113] (4)
* [October 2023][114] (5)
* [September 2023][115] (4)
* [August 2023][116] (5)
* [July 2023][117] (10)
* [May 2023][118] (2)
* [April 2023][119] (1)
* [February 2023][120] (13)
* [January 2023][121] (4)
* [December 2022][122] (2)
* [October 2022][123] (2)
* [November 2021][124] (1)
* [October 2021][125] (1)
* [September 2021][126] (1)
* [August 2021][127] (1)
* [May 2021][128] (1)
* [April 2021][129] (2)
* [March 2021][130] (1)
* [January 2021][131] (3)
* [December 2020][132] (3)
* [November 2020][133] (4)
* [October 2020][134] (1)
* [September 2020][135] (7)
* [August 2020][136] (3)
* [July 2020][137] (2)
* [June 2020][138] (5)
* [May 2020][139] (1)
* [December 2019][140] (2)
* [November 2019][141] (1)
* [April 2017][142] (1)
* [May 2016][143] (1)
* [February 2016][144] (1)
* [June 2015][145] (1)
* [April 2015][146] (1)
* [November 2014][147] (1)
* [October 2014][148] (1)
* [March 2014][149] (1)
* [October 2013][150] (1)

[ Proudly powered by WordPress ][151]

[1]: https://offlinemark.com/
[2]: #content
[3]: https://offlinemark.com
[4]: https://offlinemark.com/about/
[5]: https://offlinemark.com/archive/
[6]: https://offlinemark.com/favorites/
[7]: https://offlinemark.com/subscribe/
[8]: https://shop.offlinemark.com
[9]: https://offlinemark.com/links/
[10]: https://offlinemark.com/now/
[11]: https://offlinemark.com/
[12]: https://offlinemark.com/int-overflow/#respond
[13]: https://offlinemark.com/category/lab-notes/
[14]: https://offlinemark.com/category/c/
[15]: https://offlinemark.com/category/cpp/
[16]: https://clang.llvm.org/docs/LanguageExtensions.html#checked-arithmetic-builtins
[17]: https://github.com/torvalds/linux/blob/master/tools/include/linux/overflow.h
[18]: https://chromium.googlesource.com/chromium/src/base/+/master/numerics/
[19]: https://google.github.io/styleguide/cppguide.html#Integer_Types
[20]: https://chromium.googlesource.com/chromium/src/+/main/styleguide/c++/c++.md#types
[21]: https://google.github.io/styleguide/cppguide.html#Integer_Types
[22]: https://chromium.googlesource.com/chromium/src/+/main/styleguide/c++/c++.md
[23]: https://chromium.googlesource.com/chromium/src/+/main/docs/security/integer-semantics.md
[24]: https://www.youtube.com/watch?v=PLcZkgHCk90
[25]: https://www.youtube.com/watch?v=QJ-LAhlLdsw
[26]: https://twitter.com/offlinemark
[27]: mailto:mark@offlinemark.com
[28]: https://offlinemark.com/author/offlinemark/
[29]: https://offlinemark.com/osdev-journal-the-virtual-memory-unit-test-experiment/
[30]: https://offlinemark.com/practice/
[31]: /int-overflow/#respond
[32]: https://offlinemark.com/an-obscure-quirk-of-proc/
[33]: https://offlinemark.com/resources/
[34]: https://offlinemark.com/memory-orderings/
[35]: https://offlinemark.com/x86/
[36]: https://offlinemark.com/freestanding-cpp/
[37]: https://offlinemark.com/successful/
[38]: https://offlinemark.com/life-is-a-business-game-art/
[39]: https://offlinemark.com/willing/
[40]: https://offlinemark.com/happy/
[41]: https://offlinemark.com/an-obscure-quirk-of-proc/
[42]: https://offlinemark.com/double-fetches-scheduling-algorithms-onion-rings/
[43]: https://offlinemark.com/demand-paging/
[44]: https://offlinemark.com/lets-understand-setjmp-longjmp/
[45]: https://offlinemark.com/freestanding-cpp/
[46]: https://offlinemark.com/shavasana/
[47]: https://offlinemark.com/sharpen/
[48]: https://offlinemark.com/devtools2026/
[49]: https://offlinemark.com/feedback/
[50]: https://offlinemark.com/feed/
[51]: https://offlinemark.com/category/deep-dive/
[52]: https://offlinemark.com/category/essay/
[53]: https://offlinemark.com/category/lab-notes/
[54]: https://offlinemark.com/category/living-doc/
[55]: https://offlinemark.com/category/micropost/
[56]: https://offlinemark.com/category/tech/
[57]: https://offlinemark.com/category/twitter-archive/
[58]: https://offlinemark.com/category/ai/
[59]: https://offlinemark.com/category/assembly/
[60]: https://offlinemark.com/category/blogging/
[61]: https://offlinemark.com/category/books/
[62]: https://offlinemark.com/category/c/
[63]: https://offlinemark.com/category/cpp/
[64]: https://offlinemark.com/category/career/
[65]: https://offlinemark.com/category/cmake/
[66]: https://offlinemark.com/category/computer-architecture/
[67]: https://offlinemark.com/category/computers/
[68]: https://offlinemark.com/category/concurrency/
[69]: https://offlinemark.com/category/creativity/
[70]: https://offlinemark.com/category/decision-making/
[71]: https://offlinemark.com/category/entrepreneurship/
[72]: https://offlinemark.com/category/favorite/
[73]: https://offlinemark.com/category/git/
[74]: https://offlinemark.com/category/language-learning/
[75]: https://offlinemark.com/category/legal/
[76]: https://offlinemark.com/category/life/
[77]: https://offlinemark.com/category/linux/
[78]: https://offlinemark.com/category/linux-kernel/
[79]: https://offlinemark.com/category/macos/
[80]: https://offlinemark.com/category/meta/
[81]: https://offlinemark.com/category/open-source/
[82]: https://offlinemark.com/category/operating-systems/
[83]: https://offlinemark.com/category/uncategorized/
[84]: https://offlinemark.com/category/poetry/
[85]: https://offlinemark.com/category/productivity/
[86]: https://offlinemark.com/category/software-development/
[87]: https://offlinemark.com/category/streaming/
[88]: https://offlinemark.com/category/web-dev/
[89]: https://offlinemark.com/category/writing/
[90]: https://offlinemark.com/2026/06/
[91]: https://offlinemark.com/2026/04/
[92]: https://offlinemark.com/2026/03/
[93]: https://offlinemark.com/2026/02/
[94]: https://offlinemark.com/2026/01/
[95]: https://offlinemark.com/2025/12/
[96]: https://offlinemark.com/2025/08/
[97]: https://offlinemark.com/2025/07/
[98]: https://offlinemark.com/2025/06/
[99]: https://offlinemark.com/2025/04/
[100]: https://offlinemark.com/2025/03/
[101]: https://offlinemark.com/2025/01/
[102]: https://offlinemark.com/2024/11/
[103]: https://offlinemark.com/2024/10/
[104]: https://offlinemark.com/2024/09/
[105]: https://offlinemark.com/2024/08/
[106]: https://offlinemark.com/2024/06/
[107]: https://offlinemark.com/2024/05/
[108]: https://offlinemark.com/2024/04/
[109]: https://offlinemark.com/2024/03/
[110]: https://offlinemark.com/2024/02/
[111]: https://offlinemark.com/2024/01/
[112]: https://offlinemark.com/2023/12/
[113]: https://offlinemark.com/2023/11/
[114]: https://offlinemark.com/2023/10/
[115]: https://offlinemark.com/2023/09/
[116]: https://offlinemark.com/2023/08/
[117]: https://offlinemark.com/2023/07/
[118]: https://offlinemark.com/2023/05/
[119]: https://offlinemark.com/2023/04/
[120]: https://offlinemark.com/2023/02/
[121]: https://offlinemark.com/2023/01/
[122]: https://offlinemark.com/2022/12/
[123]: https://offlinemark.com/2022/10/
[124]: https://offlinemark.com/2021/11/
[125]: https://offlinemark.com/2021/10/
[126]: https://offlinemark.com/2021/09/
[127]: https://offlinemark.com/2021/08/
[128]: https://offlinemark.com/2021/05/
[129]: https://offlinemark.com/2021/04/
[130]: https://offlinemark.com/2021/03/
[131]: https://offlinemark.com/2021/01/
[132]: https://offlinemark.com/2020/12/
[133]: https://offlinemark.com/2020/11/
[134]: https://offlinemark.com/2020/10/
[135]: https://offlinemark.com/2020/09/
[136]: https://offlinemark.com/2020/08/
[137]: https://offlinemark.com/2020/07/
[138]: https://offlinemark.com/2020/06/
[139]: https://offlinemark.com/2020/05/
[140]: https://offlinemark.com/2019/12/
[141]: https://offlinemark.com/2019/11/
[142]: https://offlinemark.com/2017/04/
[143]: https://offlinemark.com/2016/05/
[144]: https://offlinemark.com/2016/02/
[145]: https://offlinemark.com/2015/06/
[146]: https://offlinemark.com/2015/04/
[147]: https://offlinemark.com/2014/11/
[148]: https://offlinemark.com/2014/10/
[149]: https://offlinemark.com/2014/03/
[150]: https://offlinemark.com/2013/10/
[151]: https://wordpress.org/
```
