# Web source

- URL: https://doc.rust-lang.org/std/intrinsics/fn.sub_with_overflow.html
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T16:21:14.101341094+00:00

```text
[Skip to main content][1]

## [sub_with_overflow][2]

[[logo]][3]

## [std][4]1.96.0

(ac68faa20 2026-05-25)

## [In std::intrinsics][5]

[std][6]::[intrinsics][7]

# Function sub_with_overflow Copy item path

[Source][8]

`pub const fn sub_with_overflow<T>(x: T, y: T) -> (T, [bool][9])
where
    T: [Copy][10],
`

🔬This is a nightly-only experimental API. (`core_intrinsics`)
Expand description

Performs checked integer subtraction

Note that, unlike most intrinsics, this is safe to call; it does not require an `unsafe` block. Therefore,
implementations must not require the user to uphold any safety invariants.

The stabilized versions of this intrinsic are available on the integer primitives via the `overflowing_sub` method. For
example, [`u32::overflowing_sub`][11]

[1]: #main-content
[2]: #
[3]: ../../std/index.html
[4]: ../../std/index.html
[5]: index.html
[6]: ../index.html
[7]: index.html
[8]: ../../src/core/intrinsics/mod.rs.html#1885
[9]: ../primitive.bool.html
[10]: ../marker/trait.Copy.html
[11]: ../primitive.u32.html#method.overflowing_sub
```
