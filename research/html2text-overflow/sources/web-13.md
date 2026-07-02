# Web source

- URL: https://rust.code-maven.com/how-to-handle-overflow
- Title: [Home][1] [Tags][2] [Newsletter][3] [Projects][4] [Training][5]
- Captured (UTC): 2026-06-29T16:21:18.518462357+00:00

```text
[Home][1] [Tags][2] [Newsletter][3] [Projects][4] [Training][5]
[Archive][6] [About][7]

# 3 ways to handle number overflow or underflow in Rust

[overflow][8] [underflow][9] [saturating_add][10] [checked_add][11] [overflow_add][12] [unchecked_add][13]
[add_with_overflow][14]

In the web application in some field I am expecting a non-negative integer number. Obviously I need to verify that the
user does not supply a string or a floating point number. Back in my previous life when I was writing mostly Perl and
Python I would probably use a regex for this. Now with Rust I just use `parse()` into the appropriate unsigned integer
and check if it was successful or not. I need to decide which unsigned integer.

As this web application is supposed to be one of the [counter examples][15] I'd take the number, increment by one and
return to the client.

As I was thinking about this it occurred to me the incremented number might be too big for the selected unsigned
integer. In another article I listed the [minimum and maximum values of the numeric types of Rust][16]. So I thought I
joke about this and I wrote a post:

> Since I started to write #Rust I started to see edge cases where I've never seen earlier. e.g. What if the "like"
> count on my post reaches 340282366920938463463374607431768211455 and another person "likes" it? It will overflow the
> u128... How should I handle it? #rustlang makes you anxious

I though it is funny as it is quite unlikely (pun intended) that I will receive that many likes to my posts, but some
people responded seriously.

That's how I learned about **saturating add** then I found out about [saturation arithmetic][17].

## What should be the result?

My question did not make much sense, but in the general case one needs to decide what to do.
* Should MAX_VALUE+1 become 0?
* Should MAX_VALUE+1 stay MAX_VALUE?
* Should we warn the user?
* Should we return an error to the user?
* Should we report to the operator of the system?

There is no one right answer. It depends on the use-case.

For example with "likes" I'd probably stay on MAX_VALUE, maybe with an extra message or maybe even an extra flag saying
"more than MAX_VALUE". E.g. the way LinkedIn show the number of connections. Up till 500 it shows you the exact number,
above 500 it just says "more than 500".

In extreme case one can even have a second (and third) variable and act as if we are in base MAX_VALUE. We could even
have a vector of these numbers, but even then we'd have some limit...

## Let's see the possibilities in Rust

In order to make it easier to follow I am going to use `u8` variables, but the idea is the same with any [integer
type][18].
* Just increment without any special treatment

`let mut count: u8 = 254;
println!("count: {}", count);
for _ in 1..=3 {
    count += 1;
    println!("count: {}", count);
}
`

In debug mode we get a `panic!`:

`$ cargo -q run

count: 254
count: 255
thread 'main' panicked at src/main.rs:5:9:
attempt to add with overflow
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
`

In release mode it silently overflows and we get 0.

`$ cargo -q run --release

count: 254
count: 255
count: 0
count: 1
`

## Saturating add

There is a method called [saturating_add][19]

`let mut count: u8 = 254;
println!("count: {}", count);
for _ in 1..=3 {
    count = count.saturating_add(1);
    println!("count: {}", count);
}
`

It will return the the sum, or the max value of the given type:

`$ cargo -q run
count: 254
count: 255
count: 255
count: 255
`

Same result in release mode.

### Checked add

We can use the [checked_add][20] method to decide what to do and even do some extra work.

`let mut count: u8 = 254;
println!("count: {}", count);
for _ in 1..=3 {
    match count.checked_add(1) {
        Some(val) => count = val,
        None => eprintln!("Too much!"),
    };
    println!("count: {}", count);
}
`

Same result in release mode.

It does not change the value and it returns an [Option][21]. In the `Some` part we get the incremented number. In the
`None` part we can decide what to do. Keep the max value as we have here, set `count` to 0, warn on STDERR as we do
here.

`$ cargo -q run

count: 254
count: 255
Too much!
count: 255
Too much!
count: 255
`

## overflow_add

[overflow_add][22] is another strange way to handle the situation. I am not sure when would I use this.

It returns a tuple, the first element being the incremented number, the second element beeing a `bool` indicating if
there was an overflow.

`let mut count: u8 = 254;
println!("count: {}", count);
for _ in 1..=3 {
    let (new_count, overflow) = count.overflowing_add(1);
    count = new_count;
    println!("count: {:3} {}", count, overflow);
}
`

The result:

`$ cargo run -q

count: 254
count: 255 false
count:   0 true
count:   1 false
`

I could also totally desregard the second value:

`let mut count: u8 = 254;
println!("count: {}", count);
for _ in 1..=3 {
    (count, _) = count.overflowing_add(1);
    println!("count: {}", count);
}
`

The result:

`$ cargo run -q

count: 254
count: 255
count:   0
count:   1
`

## Unchecked add

There is also an [unchecked_add][23], but it is experimental so I won't try it now.

## Add with overflow

There is also [add_with_overflow][24] which is also experimental.

## All the source code:

**[examples/overflow/src/main.rs][25]**

`fn main() {
    let mut count: u8 = 254;
    println!("count: {}", count);
    for _ in 1..=3 {
        count += 1;
        println!("count: {}", count);
    }

    let mut count: u8 = 254;
    println!("count: {}", count);
    for _ in 1..=3 {
        match count.checked_add(1) {
            Some(val) => count = val,
            None => eprintln!("Too much!"),
        };
        println!("count: {}", count);
    }

    let mut count: u8 = 254;
    println!("count: {}", count);
    for _ in 1..=3 {
        count = count.saturating_add(1);
        println!("count: {}", count);
    }

    let mut count: u8 = 254;
    println!("count: {}", count);
    for _ in 1..=3 {
        let (new_count, overflow) = count.overflowing_add(1);
        count = new_count;
        println!("count: {:3} {}", count, overflow);
    }

    let mut count: u8 = 254;
    println!("count: {}", count);
    for _ in 1..=3 {
        (count, _) = count.overflowing_add(1);
        println!("count: {}", count);
    }
}

`

## The 20-year-old bug in binary search caused by overflow

John Corbett pointed at the "Implementation Issues" section of [Binary search algorithm][26] where it is discussed that
an overflow bug has existed in most of the implementations of the Binary search for way too many years. Very interesting
discussion.

## Conclusion

One needs to think hard what **should** happen when a variable holding an integer is changed to an unsupported value and
then there are several ways to handle them.

I have not checked what is the runtime impact of using either of the above solutions.

### Related Pages

[Rocket - multi-counter using cookies][27]
[An almost infinite Fibonacci Iterator][28]

### Author

Gabor Szabo (szabgab)

[Gabor Szabo][29], the author of the Rust Maven web site maintains several [Open source projects in Rust][30] and while
he still feels he has tons of new things to learn about Rust he already offers [training courses in Rust][31] and still
teaches Python, Perl, git, GitHub, GitLab, CI, and testing.

[Gabor Szabo]

Get extra content and notifications in the [Rust Maven newsletter][32]!. [source][33]

[1]: /
[2]: /tags/
[3]: /subscribe
[4]: /projects
[5]: /training-course
[6]: /archive
[7]: /about
[8]: /tags/overflow
[9]: /tags/underflow
[10]: /tags/saturating_add
[11]: /tags/checked_add
[12]: /tags/overflow_add
[13]: /tags/unchecked_add
[14]: /tags/add_with_overflow
[15]: https://code-maven.com/counter
[16]: /minimum-and-maximum-values-of-numeric-types
[17]: https://en.wikipedia.org/wiki/Saturation_arithmetic
[18]: /minimum-and-maximum-values-of-numeric-types
[19]: https://doc.rust-lang.org/std/intrinsics/fn.saturating_add.html
[20]: https://doc.rust-lang.org/std/primitive.u8.html#method.checked_add
[21]: https://doc.rust-lang.org/std/option/enum.Option.html
[22]: https://doc.rust-lang.org/std/primitive.i8.html#method.overflowing_add
[23]: https://doc.rust-lang.org/std/primitive.u8.html?search=Some#method.unchecked_add
[24]: https://doc.rust-lang.org/std/intrinsics/fn.add_with_overflow.html
[25]: https://github.com/szabgab/rust.code-maven.com/tree/main/examples/overflow/src/main.rs
[26]: https://en.wikipedia.org/wiki/Binary_search_algorithm
[27]: /rocket-multi-counter-using-cookies
[28]: /fibonacci-iterator
[29]: https://szabgab.com/
[30]: /projects
[31]: /training-course
[32]: /subscribe
[33]: https://github.com/szabgab/rust.code-maven.com/blob/main/pages/how-to-handle-overflow.md
```
