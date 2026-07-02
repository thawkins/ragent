# Web source

- URL: https://www.reddit.com/r/rust/comments/1305ihw/rusts_philosophy_of_panic_recovering
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T16:20:27.654697176+00:00

```text
[ Skip to main content ][1]
Open menu Open navigation [ ][2]Go to Reddit Home
[ Sign Up ][3]Sign up for Reddit [ Log In ][4]Log in to Reddit
Expand user menu Open settings menu
[
Go to rust ][5]
[r/rust][6]
• 3y ago
[deleted]

# Rust's philosophy of panic recovering

Rust book says that when the program panics there will be no ways to recover and suggests a preference to use `Result`
as the return value instead of panicking. Although the `std` library provides `catch_unwind` to allow the program to
continue executing when panicking, the document mentions the same that this method shouldn't be used for a try/catch
like pattern.

Go also has the idea of panic but it has a convenient `recover` method to recover from panic. One useful case for
recover I can think of is that if you are writing a web server framework where you let users pass custom handler
functions, you can recover any panics from the custom handlers in the framework so that the server program won't exit
because of it.

Should I use `catch_unwind` for this purpose in Rust? Or does Rust has a better alternative or a different philosophy
for such requirements?

Share

# People also ask about section

People also ask about
[
Rust panic handling versus using Result
][7]
[
Rust documentation on remainder by zero panic
][8]
[
Top Rust libraries for web development
][9]
[
Best practices for Rust error handling
][10]
[
How to optimize Rust code performance
][11]
Public

Anyone can view, post, and comment to this community

0 0

## Top Posts
* [
  Reddit
  reReddit: Top posts of April 27, 2023
  ][12]
* [
  Reddit
  reReddit: Top posts of April 2023
  ][13]
* [
  Reddit
  reReddit: Top posts of 2023
  ][14]
* [Home][15]
* [Popular][16]
* [News][17]
* [Explore][18]
* [Best of Reddit][19]
* [Best of Reddit in Portuguese][20]
* [Best of Reddit in German][21]
* [Reddit Rules][22]
* [Privacy Policy][23]
* [User Agreement][24]
* [Accessibility][25]
* [Reddit, Inc. © 2026. All rights reserved.][26]

Join the most real place on the internet

Continue with Phone Number
Continue with Email

By continuing, you agree to our [User Agreement][27] and acknowledge that you understand the [Privacy Policy][28].

[1]: #main-content
[2]: https://www.reddit.com/
[3]: https://www.reddit.com/register/
[4]: https://www.reddit.com/login/
[5]: https://www.reddit.com/r/rust/
[6]: https://www.reddit.com/r/rust/
[7]: https://www.reddit.com/answers/d2c6f63d-34ae-4789-943b-711c4bbadd46/?q=Rust+panic+handling+versus+using+Result&sour
ce=PDP
[8]: https://www.reddit.com/answers/adb823d1-b4a1-4327-b44e-059bdce52b0c/?q=Rust+documentation+on+remainder+by+zero+pani
c&source=PDP
[9]: https://www.reddit.com/answers/ed32bb44-44b0-4ba7-a6c5-7c3129538e85/?q=Top+Rust+libraries+for+web+development&sourc
e=PDP
[10]: https://www.reddit.com/answers/8050b9bd-2765-4284-8971-df008a8d248e/?q=Best+practices+for+Rust+error+handling&sour
ce=PDP
[11]: https://www.reddit.com/answers/022a5c8a-3978-4eab-b5fa-6e37953eb883/?q=How+to+optimize+Rust+code+performance&sourc
e=PDP
[12]: https://www.reddit.com/posts/2023/april-27-1/global/
[13]: https://www.reddit.com/posts/2023/april/global/
[14]: https://www.reddit.com/posts/2023/global/
[15]: /?feed=home
[16]: /r/popular/
[17]: /news/
[18]: /explore/
[19]: https://www.reddit.com/posts/2026/global/
[20]: https://www.reddit.com/posts/2026/tl-pt-BR/
[21]: https://www.reddit.com/posts/2026/tl-de/
[22]: https://www.redditinc.com/policies/content-policy
[23]: https://www.reddit.com/policies/privacy-policy
[24]: https://www.redditinc.com/policies/user-agreement
[25]: https://support.reddithelp.com/hc/sections/38303584022676-Accessibility
[26]: https://redditinc.com
[27]: https://www.redditinc.com/policies/user-agreement
[28]: https://www.redditinc.com/policies/privacy-policy
```
