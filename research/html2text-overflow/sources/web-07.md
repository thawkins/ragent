# Web source

- URL: https://users.rust-lang.org/t/what-is-the-error-thread-main-panicked-at-attempt-to-subtract-with-overflow/84927
- Title: [The Rust Programming Language Forum][1]
- Captured (UTC): 2026-06-29T16:21:10.872268514+00:00

```text
[The Rust Programming Language Forum][1]

# [What is the error "thread 'main' panicked at 'attempt to subtract with overflow"?][2]

[ help ][3]
[fgsfasrfASF][4] November 27, 2022, 7:44am 1

What is the error "thread 'main' panicked at 'attempt to subtract with overflow"?

`use std::io;
fn read<T: std::str::FromStr>() -> Vec<T> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().split(' ').flat_map(str::parse).collect()
}
fn solve(i:usize, m:usize, a:&Vec<usize>) -> bool{
    if m == 0{
        return true;
    }
    if i >= m{
        return false;
    }
    return solve(i + 1, m, a) || solve(i + 1, m - a[i], a);
}
fn main() {
    let _n = read::<usize>();
    let a = read::<usize>();
    let q = read::<usize>();
    let m = read::<usize>();
    for i in 0..q[0]{
        if solve(0, m[i],&a) == true{
            println!("yes");
        }else{
            println!("no");
        }
    } 
}
`

input
5
1 5 7 10 21
4
2 4 17 8

[H2CO3][5] November 27, 2022, 8:03am 2

It means exactly what it says. One of the subtraction operations resulted in a value that over- or underflowed. For
example, `int32::MIN - 1` can cause such a panic in debug mode. The only subtraction I see in your code is `m - a[i]`,
so you will need to check what this does and fix the error there.

[afetisov][6] November 27, 2022, 1:09pm 3

A value `x: usize` can only hold integer values from `0` to `2^64 - 1`, including the ends (on 64-bit CPU architectures,
which you use now). If the result of an arithmetic operation cannot fit in this range, you get an overflow error in
debug mode or an arithmetic wrapping in release mode. Wrapping means that the result is the same as if you did the
operation on infinite precision integers, and then take the remainder of division by `2^64`, producing the value in the
range above.

So `0_usize - 1` will either panic in dev build with overflow error, or wrap around and produce `2^64 - 1` in release
mode (more generally, whenever overflow checks are disabled). Most of the time wrapping is a major bug which can cause
all kinds of problems, but overflow checks can inhibit optimizations. For this reason the checks are, by default,
performed only in dev builds. You can manually enable or disable overflow checks in any build using the
[overflow-checks][7] Cargo profile option. You can also explicitly perform wrapping arithmetics, if you need to, using
the `wrapping_add`, `wrapping_sub` etc family of operations.

### Related topics

────────────────────────────────────────────────────────────────────┬──┬───────┬───────────────────┬────────
Topic                                                               │  │Replies│Views              │Activity
────────────────────────────────────────────────────────────────────┼──┼───────┼───────────────────┴────────
[Is checking overflow disabled in method?][8]                       │2 │732    │April 4, 2016      
[ help ][9]                                                         │  │       │                   
────────────────────────────────────────────────────────────────────┼──┼───────┼───────────────────
[Error in Rust execution on official Web Page][10]                  │1 │638    │February 22, 2017  
[ help ][11]                                                        │  │       │                   
────────────────────────────────────────────────────────────────────┼──┼───────┼───────────────────
[When can usize or u64 overflow?][12]                               │31│3092   │November 25, 2022  
[ help ][13]                                                        │  │       │                   
────────────────────────────────────────────────────────────────────┼──┼───────┼───────────────────
[Potential overflowing expression not checked by rustc?][14]        │3 │304    │December 17, 2023  
[ help ][15]                                                        │  │       │                   
────────────────────────────────────────────────────────────────────┼──┼───────┼───────────────────
[Division by nonzero][16]                                           │6 │1015   │September 11, 2017 
────────────────────────────────────────────────────────────────────┴──┴───────┴───────────────────
* [Home ][17]
* [Categories ][18]
* [Guidelines ][19]
* [Terms of Service ][20]

Powered by [Discourse][21], best viewed with JavaScript enabled

[1]: /
[2]: /t/what-is-the-error-thread-main-panicked-at-attempt-to-subtract-with-overflow/84927
[3]: /c/help/5
[4]: https://users.rust-lang.org/u/fgsfasrfASF
[5]: https://users.rust-lang.org/u/H2CO3
[6]: https://users.rust-lang.org/u/afetisov
[7]: https://doc.rust-lang.org/cargo/reference/profiles.html#overflow-checks
[8]: https://users.rust-lang.org/t/is-checking-overflow-disabled-in-method/5273
[9]: /c/help/5
[10]: https://users.rust-lang.org/t/error-in-rust-execution-on-official-web-page/9597
[11]: /c/help/5
[12]: https://users.rust-lang.org/t/when-can-usize-or-u64-overflow/84752
[13]: /c/help/5
[14]: https://users.rust-lang.org/t/potential-overflowing-expression-not-checked-by-rustc/104073
[15]: /c/help/5
[16]: https://users.rust-lang.org/t/division-by-nonzero/12822
[17]: /
[18]: /categories
[19]: /guidelines
[20]: /tos
[21]: https://www.discourse.org
```
