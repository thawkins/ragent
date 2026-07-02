# Web source

- URL: https://users.rust-lang.org/t/caesar-cipher-wrap-around-error-attempt-to-subtract-with-overflow/97361
- Title: [The Rust Programming Language Forum][1]
- Captured (UTC): 2026-06-29T16:20:59.228215695+00:00

```text
[The Rust Programming Language Forum][1]

# [Caesar Cipher Wrap-Around Error: attempt to subtract with overflow][2]

[ help ][3]
[saurabh][4] July 22, 2023, 7:41pm 1

I'm trying to implement Caesar Cipher in Rust. My tests pass for the encode function, but for the decode function, I get
a warning:

`   |
34 |             if translated_index < 0 {
   |                ^^^^^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(unused_comparisons)]` on by default
`

and an error

`panicked at 'attempt to subtract with overflow', src/cryptography/caesar_cipher.rs:32:40
`

I'm not sure what's wrong with the wrap-around logic of the `decode` function, because I implemented `encode` in the
same manner and it works.

This is the complete code:

`fn caesar_encode(message: &str, key: usize) -> String {
        let supported_characters = "abcdefghijklmnopqrstuvwxyz";
        let mut encoded_message = String::new();

        for letter in message.chars() {
                if supported_characters.contains(letter) {
                        let letter_index = supported_characters.find(letter).unwrap();
                        let mut translated_index = letter_index + key;

                        if translated_index >= supported_characters.len() {
                                translated_index = translated_index - supported_characters.len()    // wrap-around
                        }

                        let translated_letter = supported_characters.chars().nth(translated_index).unwrap();
                        encoded_message.push(translated_letter)

                } else {
                        encoded_message.push(letter)
                }
        }

        encoded_message
}

fn caesar_decode(message: &str, key: usize) -> String {
        let supported_characters = "abcdefghijklmnopqrstuvwxyz";
        let mut decoded_message = String::new();

        for letter in message.chars() {
                if supported_characters.contains(letter) {
                        let letter_index = supported_characters.find(letter).unwrap();
                        let mut translated_index = letter_index - key;

                        if translated_index < 0 {
                                translated_index = translated_index + supported_characters.len()    // wrap-around
                        }

                        let translated_letter = supported_characters.chars().nth(translated_index).unwrap();
                        decoded_message.push(translated_letter)

                } else {
                        decoded_message.push(letter)
                }

        }

        decoded_message
}


#[cfg(test)]
mod tests {

        use super::*;

        #[test]
        fn test_caesar_encode() {
                let data = [
                        ("veni vidi vici", 3, "yhql ylgl ylfl"),
                        ("hello", 10, "rovvy"),
                        ("hello world!", 0, "hello world!"),
                        (
                                "you can show black is white by argument, but you will never convince me.",
                                8,
                                "gwc kiv apwe jtiks qa epqbm jg izocumvb, jcb gwc eqtt vmdmz kwvdqvkm um."
                        ),
                        ("", 5, ""),
                        ("a", 1, "b"),
                        ("x", 7, "e"),
                        ("z", 1, "a"),
                ];

                for (plaintext_message, key, expected) in data {
                        assert_eq!(caesar_encode(plaintext_message, key), expected)
                }
        }

        #[test]
        fn test_caesar_decode() {
                let data = [
                        ("a", 1, "z"),
                        
("wkh txlfn eurzq ira mxpsv ryhu wkh odcb grj", 3, "the quick brown fox jumps over the lazy dog"),
                        ("aol lultf pz ulhy", 7, "the enemy is near"),
                ];

                for (cryptic_message, key, expected) in data {
                        assert_eq!(caesar_decode(cryptic_message, key), expected)
                }
        }
}
`

[jameseb7][5] July 22, 2023, 8:13pm 2

Both `letter_index` and `key` are of type `usize`, which cannot be smaller than 0, because it is unsigned. Thus,
subtracting `key` from `letter_index` will fail if `letter_index` is smaller than `key`. The easiest way to deal with
this is probably to convert to `isize`, which will allow negatives, since you are not going to be dealing with numbers
above 26 here, so the different maximum ranges for `usize` and `isize` don't matter. You can then convert back to
`usize` for indexing. You may want to reduce `key` using the modulus operator (`%`), to ensure it fits within
`supported_characters.len()` (and hence `isize::MAX`).

[quinedot][6] July 22, 2023, 8:23pm 3
jameseb7:

> You may want to reduce `key` using the modulus operator (`%`)

If you do this as `isize`, you'll want [`rem_euclid`.][7] (As `usize`, either works.)

[saurabh][8] July 22, 2023, 8:53pm 4

Thanks [@jameseb7][9] and [@quinedot][10] !

I replaced:

`let mut translated_index = letter_index - key;

if translated_index < 0 {
        translated_index = translated_index + supported_characters.len()    // wrap-around
}
`

with:

`let translated_index = (letter_index + supported_characters.len() - key) % supported_characters.len();
`

from

> You may want to reduce `key` using the modulus operator (`%`), to ensure it fits within `supported_characters.len()`
> (and hence `isize::MAX`).

[quinedot][11] July 22, 2023, 9:03pm 5

Some notes on the implementation:

The modulus `%` or `mod_euclid` mentioned in the previous replies corresponds to [modular arithmetic,][12] which "wraps
around" from the end to the beginning or from the beginning to the end. For this particular problem, you're effectively
working with indices `mod 26`.

So for example, if you pass in a key that's `30`, it will work the same as a key that's `4`. You wrap around completely
once (`26`) and then advance 4 more (`26 + 4 = 30`). And `30 % 26 == 4`.

This works with negative numbers too, so if your key was `-4`, this would be the same as a key that was `26 + (-4) == 26
- 4 == 22`. And `-4.mod_euclid(26) == 22`.

Why did I mention negative keys when your functions take `usize`, which doesn't allow negatives? Well, let's consider
the difference between your two functions:

`-fn caesar_encode(message: &str, key: usize) -> String {
+fn caesar_decode(message: &str, key: usize) -> String {
     let supported_characters = "abcdefghijklmnopqrstuvwxyz";
     let mut encoded_message = String::new();

     for letter in message.chars() {
         if supported_characters.contains(letter) {
             let letter_index = supported_characters.find(letter).unwrap();
-            let mut translated_index = letter_index + key;
+            let mut translated_index = letter_index - key;

-                if translated_index >= supported_characters.len() {
-                    translated_index = translated_index - supported_characters.len()    // wrap-around
+                if translated_index < 0 {
+                    translated_index = translated_index + supported_characters.len()    // wrap-around
`

The last part of the difference is just "manually" performing modular arithmetic to stay in range. But this is the
key^{[[1]][13]} difference:

`-            let mut translated_index = letter_index + key;
+            let mut translated_index = letter_index - key;
`

When using modular arithmetic, decoding with `key` is the same as encoding with `-key`.

So you can rewrite this to only have logic in `caesar_encode`, and then for `caesar_decode`, just calculate the
appropriate key and call `caesar_encode`.

There are other ways you could tighten up the code, but instead of just writing them out I'll leave some clues you can
choose to follow up with or not.
* Your supported characters are the ASCII lowecase letters. You can test if a `char` is an ASCII lowercase letter with
  [`is_ascii_lowercase.`][14]
* Those lowercase letters are also an in-order and contiguous (no gaps) range within UTF8 (and ASCII), so you can
  convert them to numbers and figure out the `letter_index` with arithmetic
* Inside the loop, you basically transform each letter into another letter (maybe itself if it's not an ASCII lowercase
  letter), and then push that into a new `String`. Then you return the `String` after the loop. That's the same as this
  pattern:
  
  `fn caesar_encode(message: &str, key: usize) -> String {
      // ...
      message.chars().map(|letter| {
          // ...
      }).collect()
  }
  `
1. sorry for the pun [↩︎][15]

[saurabh][16] July 22, 2023, 9:14pm 6

Thank you so much for breaking it down!

### Related topics

─────────────────────────────────────────────────────────────────────┬──┬───────┬──────────────────┬────────
Topic                                                                │  │Replies│Views             │Activity
─────────────────────────────────────────────────────────────────────┼──┼───────┼──────────────────┴────────
[Why won't this code work?][17]                                      │5 │639    │May 26, 2019      
[ help ][18]                                                         │  │       │                  
─────────────────────────────────────────────────────────────────────┼──┼───────┼──────────────────
[Good / Needs Work? Atbash Cipher for Exercism][19]                  │13│1268   │October 23, 2019  
[ help ][20]                                                         │  │       │                  
─────────────────────────────────────────────────────────────────────┼──┼───────┼──────────────────
[Rust RSA implementation weird behaviours][21]                       │9 │1508   │February 23, 2019 
[ help ][22]                                                         │  │       │                  
─────────────────────────────────────────────────────────────────────┼──┼───────┼──────────────────
[Extern crate crypto: error: source trait is private][23]            │2 │1085   │January 30, 2016  
[ help ][24]                                                         │  │       │                  
─────────────────────────────────────────────────────────────────────┼──┼───────┼──────────────────
[How to disect OpenSSL error returned from decrypt_aead?][25]        │5 │1087   │April 13, 2018    
[ help ][26]                                                         │  │       │                  
─────────────────────────────────────────────────────────────────────┴──┴───────┴──────────────────
* [Home ][27]
* [Categories ][28]
* [Guidelines ][29]
* [Terms of Service ][30]

Powered by [Discourse][31], best viewed with JavaScript enabled

[1]: /
[2]: /t/caesar-cipher-wrap-around-error-attempt-to-subtract-with-overflow/97361
[3]: /c/help/5
[4]: https://users.rust-lang.org/u/saurabh
[5]: https://users.rust-lang.org/u/jameseb7
[6]: https://users.rust-lang.org/u/quinedot
[7]: https://doc.rust-lang.org/std/primitive.isize.html#method.rem_euclid
[8]: https://users.rust-lang.org/u/saurabh
[9]: /u/jameseb7
[10]: /u/quinedot
[11]: https://users.rust-lang.org/u/quinedot
[12]: https://en.wikipedia.org/wiki/Modular_arithmetic
[13]: #footnote-408603-1
[14]: https://doc.rust-lang.org/std/primitive.char.html#method.is_ascii_lowercase
[15]: #footnote-ref-408603-1
[16]: https://users.rust-lang.org/u/saurabh
[17]: https://users.rust-lang.org/t/why-wont-this-code-work/28595
[18]: /c/help/5
[19]: https://users.rust-lang.org/t/good-needs-work-atbash-cipher-for-exercism/33839
[20]: /c/help/5
[21]: https://users.rust-lang.org/t/rust-rsa-implementation-weird-behaviours/25529
[22]: /c/help/5
[23]: https://users.rust-lang.org/t/extern-crate-crypto-error-source-trait-is-private/4469
[24]: /c/help/5
[25]: https://users.rust-lang.org/t/how-to-disect-openssl-error-returned-from-decrypt-aead/16785
[26]: /c/help/5
[27]: /
[28]: /categories
[29]: /guidelines
[30]: /tos
[31]: https://www.discourse.org
```
