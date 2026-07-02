# Web source

- URL: https://users.rust-lang.org/t/attempt-to-subtract-with-overflow-panic-depending-on-order/114497
- Title: [The Rust Programming Language Forum][1]
- Captured (UTC): 2026-06-29T16:21:13.604419484+00:00

```text
[The Rust Programming Language Forum][1]

# ["attempt to subtract with overflow" panic depending on order][2]

[ help ][3]
[hannes][4] July 16, 2024, 2:00pm 1

I never came across this in any other language, so I'm just curious. If running in debug mode, this is panicking with
"attempt to subtract with overflow"

`let mut x:usize = 1;
x = x - 2 + 1;
`

but this is not

`let mut x:usize = 1;
x = x + 1 - 2;
`

So it seems like the expression on the right hand side is evaluated from left to right and whenever the intermediate
result is negative it will panic? What would be the problem with first evaluating the whole right hand side and then
checking if it's negative? If somebody could point me to some more info about this, that'd be great.

[khimru][5] July 16, 2024, 2:08pm 2
hannes:

> What would be the problem with first evaluating the whole right hand side and then checking if it's negative?

So the idea is to accept all calculations that would produce no problems if calculations always produce correct results
and reject them otherwise?

That property, like most other properties, [is undecidable][6], so there would be no problems, except you would never
know whether compiler would accept correct program (in that definition) or not.

[richardscollin][7] July 16, 2024, 2:09pm 3

[https://doc.rust-lang.org/reference/expressions.html#expression-precedence][8]

> The precedence of Rust operators and expressions is ordered as follows, going from strong to weak. Binary Operators at
> the same precedence level are grouped in the order given by their associativity.

So, because it's left associative it would be:

`x = (x - 2) + 1;
`

vs

`x = (x + 1) - 2;
`

[jdahlstrom][9] July 16, 2024, 2:09pm 4

Left-to-right evaluation order of operations of equal precedence is used in Rust like in any other language with infix
operators. Under the so-called "as if" rule, the compiler is perfectly free to exploit associativity or commutativity
*iff* it does not change the semantics of the code. And panicking is a part of the semantics of the language, so if some
code panics in some condition, making that panic not happen in that condition is not a change the compiler is permitted
to make.

[tczajka][10] July 16, 2024, 2:28pm 5
hannes:

> What would be the problem with first evaluating the whole right hand side and then checking if it's negative?

Evaluating a whole expression before bounds checking would require larger-precision arithmetic, for instance to evaluate
`x * 1000 / 1001` without overflow in the intermediate values would require more bits than `x` has.

[khimru][11] July 16, 2024, 2:43pm 6

But in some cases that doesn't happen (original could would work fine if you would just use wrapping arithmetic and may
even be optimized into `x - 1`).

I wonder if such a language where expressions that could be calculated that way are accepted while others, that require
arbitrary precision arithmetic, are rejected even exists, though.

And [@hannes][12] tells us that such approach is not just typical, but common (*I never came across this in any other
language*) which kinda doesn't match my experience at all.

I know languages that detect overflow ([Pascal][13] with range checking is one example and it existed for more that half
century), I know languages that ignore overflow (C or JavaScript), I even know of languages that have
arbitrary-precision arithmetic (Python or Scheme), but languages that topicstarter asserts as common and typical… no
idea! Not a single example comes to mind!

[scottmcm][14] July 16, 2024, 2:47pm 7
hannes:

> What would be the problem with first evaluating the whole right hand side and then checking if it's negative?

That would be a very different system of arithmetic types. I would love it if we moved from types that need every
operation checked to types that return sufficiently-large results that only need checking on truncation. But that's not
a trivial thing to do at all. For example, quick, what's `569936821221962380720³ + (−569936821113563493509)³ +
(−472715493453327032)³`? How much space do you need to evaluate it?

If you want to just do something in infinite precision, there's always [num-bigint — Rust math library // Lib.rs][15]

[hannes][16] July 16, 2024, 2:59pm 8

Thanks for all the remarks, they make good sense to me!

To clarify, I just put a trivial example to show the point. In my actual code there was a complicated expression and it
was not immediately clear which terms are positive and which terms are negative. I had to rearrange the terms, starting
with the positive ones for it to run in debug mode. That just surprised me [:grin:]

[tczajka][17] July 16, 2024, 3:11pm 9
scottmcm:

> I would love it if we moved from types that need every operation checked to types that return sufficiently-large
> results that only need checking on truncation.

I don't see how anything like this could possibly work in Rust. Presumably this would mean that `x + 1` (where `x` is
`u32`) would no longer have type `u32` which seems totally incompatible with the way it works now.

[scottmcm][18] July 16, 2024, 3:47pm 10
tczajka:

> Presumably this would mean that `x + 1` (where `x` is `u32`) would no longer have type `u32`

There's a reason it's not an RFC. I wish more languages did that, but it's probably too drastic for Rust.

And yes, it would. But it'd mean that, for example, `(x + y) / 2` gets you back to the type you started with, without
overflow issues without needing a special [https://doc.rust-lang.org/std/primitive.u32.html#method.midpoint][19].

[khimru][20] July 16, 2024, 4:42pm 11
scottmcm:

> But it'd mean that, for example, `(x + y) / 2` gets you back to the type you started with, without overflow issues
> without needing a special [https://doc.rust-lang.org/std/primitive.u32.html#method.midpoint ][21].

It would be great, but how would you actually implement that on machine code level?

Note that `midpoint` is nothing special, it just calculates not `(x + y) / 2`, but `((x ^ y) >> 1) + (x & y)` instead.

Converting one expression into another is quite non-trivial and, as usual, undecidable, in general, task. Especially if
you add things like special instructions that allow you to calculate `modpoint` more efficiently (yes, modern CPUs have
them, although usually only for SIMD).

[scottmcm][22] July 16, 2024, 5:20pm 12
khimru:

> but how would you actually implement that on machine code level?

The same way that LLVM already does it if you just use a wider intermediate type in LLVM:
[https://llvm.godbolt.org/z/5nT48YW7o][23].

It's just an optimization issue. Even cranelift will optimize out unnecessary large intermediate results in some cases:
[https://github.com/bytecodealliance/wasmtime/blob/50d82f22e5ee5e0e9ef70a1e86d963e0fbc1cc04/cranelift/filetests/filetest
s/egraph/arithmetic.clif#L342-L357][24].

[khimru][25] July 16, 2024, 5:34pm 13
scottmcm:

> It's just an optimization issue.

Yes and no. The fact that you can do **some** calculations that way but so very few while the majority would lead to
huge slowdown would be immensely confusing.

We have chicken and egg problem here: introducing such a radical transformation is very scary till we know how often
this would make people write extremely inefficient code and we wouldn't know it till change would actually be
implemented.

[scottmcm][26] July 16, 2024, 7:16pm 14
khimru:

> The fact that you can do **some** calculations that way but so very few while the majority would lead to huge slowdown
> would be immensely confusing.

I'd rather the default to be to actually give something *correct*, even in release mode. I shouldn't need to know tricks
just because I want to do `(a[0] + 2 * a[1] + a[2]) / 4`.

It'd always be possible to truncate the intermediate results if you want fast results instead of correct ones, because
if you truncate at each step then it'll never take "huge slowdown". And the profiler would clearly show the cost if it's
a problem.

I don't think it'd be that surprising, either. It's only things like division that cause intermediates to grow. If you
do a wrapping conversion at the end, you can do as much multiplication/addition/subtraction as you want without needing
larger intermediate results.

[paramagnetic][27] July 16, 2024, 9:00pm 15
hannes:

> To clarify, I just put a trivial example to show the point. In my actual code there was a complicated expression and
> it was not immediately clear which terms are positive and which terms are negative. I had to rearrange the terms,
> starting with the positive ones for it to run in debug mode. That just surprised me [:grin:]

I'm not sure why this surprised you? What do you think should have happened instead?

First off, subtraction is **not** commutative or associative.

Second, the whole "checking overflow" feature is in place to catch bugs. If you are writing operations that produce
invalid intermediate results, the only place to catch those errors is right there and then.

If you expect the compiler to "wait" until "the end" of evaluation, then in all seriousness, when should it stop? Should
it wait for the entire execution of `main` and then trace back meticulously to the point where *some* error happened?
Should it arbitrarily decide the scope where it's "the best"? That doesn't make a modicum of sense to me.

[hannes][28] July 17, 2024, 4:34am 16
paramagnetic:

> First off, subtraction is **not** commutative or associative.

I don't see how this is related to the topic. This is about the addition of integers, which is commutative and
associative, `-2 + 1` is equivalent to `1 - 2`, and further, integers are closed under subtraction.
The point is that the compiler expects a natural number (including 0) and not an integer.

paramagnetic:

> If you expect the compiler to "wait" until "the end" of evaluation, then in all seriousness, when should it stop?

Well, one possibility would be to wait until the end of the evaluation of that expression on the RHS of the statement?
If you tell me that the compiler checking after every single arithmetic operation is the only reasonable thing to do,
then I accept that because I have no time to deeply study about what a compiler should or should not do. But I'm sure
people could come up with a hundred different things what a compiler could do in this situation, but they might all be
highly inefficient/complex/undecidable or whatnot.

[Cerber-Ursi][29] July 17, 2024, 4:58am 17
hannes:

> Well, one possibility would be to wait until the end of the evaluation of that expression on the RHS of the statement?

And this means that statement boundaries are suddenly becoming semantically meaningful, in particular, they start to
influence possible side-effects. Therefore, for example, it would be illegal to "inline" (internally) some such
statement into the next one, since this will remove the possible panicking spot.

[paramagnetic][30] July 17, 2024, 5:51am 18
hannes:

> I don't see how this is related to the topic. This is about the addition of integers

No it's not. `x - 2` is parsed as a subtraction, not as the addition of (-2).

hannes:

> integers are closed under subtraction.

Unsigned integers are *not* closed under subtraction. `usize` is unsigned.

[stonerfish][31] July 17, 2024, 6:01am 19
richardscollin:

> So, because it's left associative it would be:
> 
> `x = (x - 2) + 1;
> `
> 
> vs
> 
> `x = (x + 1) - 2;
> `

I thought that the Rust compiler would reduce both down to x = x - 1,
I thought the -2 and + 1 parts are just constant and would fold away.

I think that is what "gcc" would do with "c" but have no reference besides my fading memories.

hannes:

> this is panicking with "attempt to subtract with overflow"
> 
> `let mut x:usize = 1;
> x = x - 2 + 1;
> `

Does this math happen at runtime? Or does maybe the math all optimize away and only the panic code is left in the final
binary?
Oh the simple little things can be so confusing.

[paramagnetic][32] July 17, 2024, 6:06am 20
stonerfish:

> I thought that the Rust compiler would reduce both down to x = x - 1,
> I thought the -2 and + 1 parts are just constant and would fold away.
> 
> I think that is what "gcc" would do with "c" but have no reference besides my fading memories.

You are confusing operational semantics with optimizations.

**[next page →][33]**

### Related topics

─────────────────────────────────────────────────────────────┬───┬───────┬──────────────────┬────────
Topic                                                        │   │Replies│Views             │Activity
─────────────────────────────────────────────────────────────┼───┼───────┼──────────────────┴────────
[Rust / x86_64, underflow / overflow][34]                    │52 │5938   │August 2, 2021    
─────────────────────────────────────────────────────────────┼───┼───────┼──────────────────
[Is there a clean way to write overflowing adds?][35]        │50 │18421  │January 13, 2022  
[ help ][36]                                                 │   │       │                  
─────────────────────────────────────────────────────────────┼───┼───────┼──────────────────
[Vector len() - 1 comparison for usize][37]                  │32 │1016   │December 22, 2024 
[ help ][38]                                                 │   │       │                  
─────────────────────────────────────────────────────────────┼───┼───────┼──────────────────
[Increment operator in Rust][39]                             │57 │5855   │January 12, 2023  
[ help ][40]                                                 │   │       │                  
─────────────────────────────────────────────────────────────┼───┼───────┼──────────────────
[Negative views on Rust: panicking][41]                      │148│8458   │January 15, 2022  
─────────────────────────────────────────────────────────────┴───┴───────┴──────────────────
* [Home ][42]
* [Categories ][43]
* [Guidelines ][44]
* [Terms of Service ][45]

Powered by [Discourse][46], best viewed with JavaScript enabled

[1]: /
[2]: /t/attempt-to-subtract-with-overflow-panic-depending-on-order/114497
[3]: /c/help/5
[4]: https://users.rust-lang.org/u/hannes
[5]: https://users.rust-lang.org/u/khimru
[6]: https://en.wikipedia.org/wiki/Rice%27s_theorem
[7]: https://users.rust-lang.org/u/richardscollin
[8]: https://doc.rust-lang.org/reference/expressions.html#expression-precedence
[9]: https://users.rust-lang.org/u/jdahlstrom
[10]: https://users.rust-lang.org/u/tczajka
[11]: https://users.rust-lang.org/u/khimru
[12]: /u/hannes
[13]: https://en.wikipedia.org/wiki/Pascal_(programming_language)
[14]: https://users.rust-lang.org/u/scottmcm
[15]: https://lib.rs/crates/num-bigint
[16]: https://users.rust-lang.org/u/hannes
[17]: https://users.rust-lang.org/u/tczajka
[18]: https://users.rust-lang.org/u/scottmcm
[19]: https://doc.rust-lang.org/std/primitive.u32.html#method.midpoint
[20]: https://users.rust-lang.org/u/khimru
[21]: https://doc.rust-lang.org/std/primitive.u32.html#method.midpoint
[22]: https://users.rust-lang.org/u/scottmcm
[23]: https://llvm.godbolt.org/z/5nT48YW7o
[24]: https://github.com/bytecodealliance/wasmtime/blob/50d82f22e5ee5e0e9ef70a1e86d963e0fbc1cc04/cranelift/filetests/fil
etests/egraph/arithmetic.clif#L342-L357
[25]: https://users.rust-lang.org/u/khimru
[26]: https://users.rust-lang.org/u/scottmcm
[27]: https://users.rust-lang.org/u/paramagnetic
[28]: https://users.rust-lang.org/u/hannes
[29]: https://users.rust-lang.org/u/Cerber-Ursi
[30]: https://users.rust-lang.org/u/paramagnetic
[31]: https://users.rust-lang.org/u/stonerfish
[32]: https://users.rust-lang.org/u/paramagnetic
[33]: /t/attempt-to-subtract-with-overflow-panic-depending-on-order/114497?page=2
[34]: https://users.rust-lang.org/t/rust-x86-64-underflow-overflow/63017
[35]: https://users.rust-lang.org/t/is-there-a-clean-way-to-write-overflowing-adds/70267
[36]: /c/help/5
[37]: https://users.rust-lang.org/t/vector-len-1-comparison-for-usize/122632
[38]: /c/help/5
[39]: https://users.rust-lang.org/t/increment-operator-in-rust/86684
[40]: /c/help/5
[41]: https://users.rust-lang.org/t/negative-views-on-rust-panicking/69796
[42]: /
[43]: /categories
[44]: /guidelines
[45]: /tos
[46]: https://www.discourse.org
```
