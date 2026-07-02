# Web source

- URL: https://stackoverflow.com/questions/73224410/how-to-catch-a-panic
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T16:20:29.911372568+00:00

```text
[Skip to main content][1]
1. [ About ][2]
2. Products
3. [ Stack Internal ][3]
1. [ Stack Internal Implement a knowledge platform layer to power your enterprise and AI tools. ][4]
2. [ Stack Data Licensing Get access to top-class technical expertise with trusted & attributed content. ][5]
3. [ Stack Ads Connect your brand to the world’s most trusted technologist communities. ][6]
4. [ Releases Keep up-to-date on features we add to Stack Overflow and Stack Internal. ][7]
5. [About the company][8] [Visit the blog][9]
Loading…
1. * [ Tour Start here for a quick overview of the site ][10]
   * [ Help Center Detailed answers to any questions you might have ][11]
   * [ Meta Discuss the workings and policies of this site ][12]
   * [ About Us Learn more about Stack Overflow the company, and our products ][13]
3. ### [current community][14]
   * [ Stack Overflow ][15]
     [help][16] [chat][17]
   * [ Meta Stack Overflow ][18]
   
   ### your communities
   
   [Sign up][19] or [log in][20] to customize your list.
   
   ### [more stack exchange communities][21]
   
   [company blog][22]
5. [Log in][23]
6. [Sign up][24]
The 2026 Annual Developer Survey is live— [take the Survey today!][25]
1. 1.  [
       Home
       ][26]
   2.  [
       Questions
       ][27]
   3.  [
       AI Assist
       ][28]
   4.  [
       Tags
       ][29]
   6.  [
       Stack Overflow for Agents
       ][30]
   8.  [
       Challenges
       ][31]
   9.  [
       Chat ][32]
   10. [
       Articles
       ][33]
   11. [
       Users
       ][34]
   13. [
       Companies
       ][35]
   14. [
       Collectives
       ][36]
   15. Communities for your favorite technologies. [Explore all Collectives][37]
2. Stack Internal
   
   Stack Overflow for Teams is now called **Stack Internal**. Bring the best of human thought and AI automation together
   at your work.
   
   [Try for free][38] [Learn more][39]
3. [
   Stack Internal
   ][40]
4. Bring the best of human thought and AI automation together at your work. [Learn more][41]

##### Collectives™ on Stack Overflow

Find centralized, trusted content and collaborate around the technologies you use most.

[ Learn more about Collectives ][42]

**Stack Internal**

Knowledge at work

Bring the best of human thought and AI automation together at your work.

[ Explore Stack Internal ][43]

# [How to catch a panic?][44]

[ Ask Question ][45]
Asked 3 years, 11 months ago
Modified [1 year, 10 months ago][46]
Viewed 21k times
24

I need to catch a panic so that it doesn't exit the the program. For example, how to catch a panic here and print
"Hello, World"?:

`fn main() {
    let v = vec![1, 2, 3];

    v[99];
    println!("Hello, World");
}
`
* [rust][47]

[Share][48]
[Improve this question][49]
Follow
[edited Aug 22, 2024 at 20:27][50]
[
[kmdreko's user avatar]
][51]
[kmdreko][52]
66.3k66 gold badges109109 silver badges179179 bronze badges
asked Aug 3, 2022 at 16:03
[
[fish0fqwerty's user avatar]
][53]
[fish0fqwerty][54]
29711 gold badge33 silver badges1010 bronze badges
[ Add a comment ][55]  | 

## 2 Answers 2

Sorted by: [ Reset to default ][56]
Highest score (default) Trending (recent votes count more) Date modified (newest first) Date created (oldest first)
40

You can use `std::panic::catch_unwind` to, well, catch unwinding panics, but do make sure to read [the
documentation][57] first:

`fn main() {
    let v = vec![1, 2, 3];
    let panics = std::panic::catch_unwind(|| v[99]).is_err();
    assert!(panics);
    println!("Hello, World");
}
`

[Share][58]
[Improve this answer][59]
Follow
answered Aug 3, 2022 at 16:20
[
[isaactfa's user avatar]
][60]
[isaactfa][61]
6,84511 gold badge1919 silver badges2828 bronze badges
Sign up to request clarification or add additional context in comments.

## Comments

Add a comment

7

If you don't know that the index is valid at run time, you can use [`get`][62] instead of `[]` indexing. This will
return an option, either `Some()` if the element exists, or `None` if it does not:

`if let Some(x) = v.get(99) {

}
`

or:

`match v.get(99) {
  Some(x) => {

  },
  None => {
  }
}
`

If this is a more general question and the given snippet is just an example, then the correct answer is you don't --
[panics are not for control flow][63]. If you need to recover from an error, you should use a method that returns a
`Result` or `Option`.

[Share][64]
[Improve this answer][65]
Follow
[edited Aug 3, 2022 at 16:25][66]
answered Aug 3, 2022 at 16:10
[
[John Ledbetter's user avatar]
][67]
[John Ledbetter][68]
14.3k11 gold badge6565 silver badges8282 bronze badges

## 4 Comments

Add a comment

fish0fqwerty
[fish0fqwerty][69] [Over a year ago][70]
Is panic not for the control flow? I don't understand you. Have link to rust docs?
2022-08-03T16:21:48.5Z+00:00
0
Reply
* Copy link
John Ledbetter
[John Ledbetter][71] [Over a year ago][72]
Sure, see the link to the rust book section on "to panic or not to panic". The whole section on Error handling may be
useful too.
2022-08-03T16:26:16.273Z+00:00
1
Reply
* Copy link
speciesUnknown
[speciesUnknown][73] [Over a year ago][74]
This answer doesn't really help unfortunately. If you are using a library which panics when it shouldn't, you only
recourse is to totally rewrite that method from scratch. It might be simple, or you might have to effectively fork that
library and start from the beginning. Its not an answer. We can preach "panics are not for control flow" until we're
blue in the face but if no human is listening, its not going to help.
2023-08-07T10:22:28.967Z+00:00
7
Reply
* Copy link
Craig McQueen
[Craig McQueen][75] [Over a year ago][76]
Good point @speciesUnknown, I've come across this issue — a library that would panic for certain inputs. Its API should
have been designed to return a `Result` instead. I can't remember what the library was unfortunately.
2024-06-18T06:37:42.273Z+00:00
1
Reply
* Copy link
Add a comment

## Your Answer

**Reminder:** Answers generated by AI tools are not allowed due to Stack Overflow's [artificial intelligence policy][77]

Thanks for contributing an answer to Stack Overflow!
* Please be sure to *answer the question*. Provide details and share your research!

But *avoid* …
* Asking for help, clarification, or responding to other answers.
* Making statements based on opinion; back them up with references or personal experience.

To learn more, see our [tips on writing great answers][78].

Draft saved
Draft discarded

### Sign up or [log in][79]

Sign up using Google
Sign up using Email and Password
Submit

### Post as a guest

Name
Email

Required, but never shown

### Post as a guest

Name
Email

Required, but never shown

Post Your Answer Discard

By clicking “Post Your Answer”, you agree to our [terms of service][80] and acknowledge you have read our [privacy
policy][81].

Start asking to get answers

Find the answer to your question by asking.

[Ask question][82]

Explore related questions
* [rust][83]

See similar questions with these tags.
* The Overflow Blog
* [Code isn’t the only thing causing your production...][84]
* [Paging Charity! How can engineering leaders avoid becoming Bond...][85]
* Featured on Meta
* [Partnering with Communities to Modernize Policies & Norms][86]
* [Policy: Generative AI (e.g., ChatGPT) is banned][87]
* [The 2026 Developer Survey is Live][88]

### Linked

[
17
][89] [When to use std::expected instead of exceptions][90]

### Related

[
2
][91] [How can I detect an error rather than have this Rust program abort?][92]
[
187
][93] [How do I write a Rust unit test that ensures that a panic has occurred?][94]
[
15
][95] [Catching panic! when Rust called from C FFI, without spawning threads][96]
[
3
][97] [What is the right way to capture panics?][98]
[
2
][99] [How to panic! in production][100]
[
5
][101] [How can I silently catch panics in QuickCheck tests?][102]
[
10
][103] [How to use panic=abort with external dependencies?][104]
[
6
][105] [Is it possible to specify `panic = "abort"` for a specific target?][106]
[
16
][107] [Is it possible to check if `panic` is set to `abort` while a library is compiling?][108]
[
10
][109] [Handling panics of external libraries][110]

#### [ Hot Network Questions ][111]
* [ Across the galaxy ][112]
* [ Client frequently paying late. Manager claiming I make mistakes but doesn't provide specifics ][113]
* [ Who invented the dot product and cross product and how? ][114]
* [ How to draw a diagram in math-mode or in-line LaTeX?, for example, the following kind of diagram? What packages do I
  require? ][115]
* [ About the usage of 'embarrassed': with a preposition, and/or without? ][116]
* [ According to Catholicism, what constitues a "practicing" Catholic? ][117]
* [ Fpqc-morphisms are epimorphisms ][118]
* [ How to get the upper wheel off a vintage craftsman bandsaw? ][119]
* [ Do Shudras have the right to take part in Tantric rituals? ][120]
* [ Larmor precession frequency approximation justified ][121]
* [ Identifying a Device Wired into my Prius C ][122]
* [ Is reliable and reasonably fast Internet only achieved with Starlink dishes in Dili, Timor-Leste? ][123]
* [ How to apply F12 corrections across a basis set series? ][124]
* [ How are graduate applicants from under-resourced universities evaluated? ][125]
* [ How to remove the `:n` specifier when calling `\getdata:n { h }`? ][126]
* [ Does the rotation direction of a kitchen exhaust fan affect its ability to remove cooking odors ][127]
* [ Node label style in TikZ cd ][128]
* [ Formatting long division without the longdivision package ][129]
* [ Can I use 1x4 pine boards instead of plywood cabinet walls to support a cooktop? ][130]
* [ How to build a voltage shifter for HS driver? ][131]
* [ Revelations 4:8 Question on Biblical Greek ][132]
* [ What is the meaning of this mark in a bowl of porcelain and silver? (Characters identified: 天府紋銀) ][133]
* [ Fire elemental is intimidated by magnesium burning under water ][134]
* [ How much of ingredients of the Standard Model are described in the perturbative Algebraic Quantum Field Theory?
  ][135]

[ more hot questions ][136]
[ Question feed ][137]

# Subscribe to RSS

Question feed

To subscribe to this RSS feed, copy and paste this URL into your RSS reader.

lang-rust

##### [Stack Overflow][138]
* [Questions][139]
* [Help][140]
* [Chat][141]

##### [Business][142]
* [Stack Internal][143]
* [Stack Data Licensing][144]
* [Stack Ads][145]

##### [Company][146]
* [About][147]
* [Press][148]
* [Work Here][149]
* [Legal][150]
* [Privacy Policy][151]
* [Terms of Service][152]
* [Contact Us][153]
* Cookie Settings
* [Cookie Policy][154]

##### [Stack Exchange Network][155]
* [ Technology ][156]
* [ Culture & recreation ][157]
* [ Life & arts ][158]
* [ Science ][159]
* [ Professional ][160]
* [ Business ][161]
* [ API ][162]
* [ Data ][163]
* [Blog][164]
* [Facebook][165]
* [Twitter][166]
* [LinkedIn][167]
* [Instagram][168]

Site design / logo © 2026 Stack Exchange Inc; user contributions licensed under [CC BY-SA][169] . rev 2026.6.25.43791

[1]: #content
[2]: https://stackoverflow.co/
[3]: https://stackoverflow.co/internal/
[4]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-nav&utm_
content=stack-overflow-for-teams
[5]: https://stackoverflow.co/data-licensing/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-na
v&utm_content=overflow-api
[6]: https://stackoverflow.co/advertising/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-nav&u
tm_content=stack-overflow-advertising
[7]: https://stackoverflow.blog/releases/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-nav&ut
m_content=releases
[8]: https://stackoverflow.co/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-nav&utm_content=a
bout-the-company
[9]: https://stackoverflow.blog/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=top-nav&utm_content
=blog
[10]: /tour
[11]: /help
[12]: https://meta.stackoverflow.com
[13]: https://stackoverflow.co/
[14]: https://stackoverflow.com
[15]: https://stackoverflow.com
[16]: https://stackoverflow.com/help
[17]: https://chat.stackoverflow.com/?tab=explore
[18]: https://meta.stackoverflow.com
[19]: https://stackoverflow.com/users/signup?ssrc=site_switcher&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f
73224410%2fhow-to-catch-a-panic
[20]: https://stackoverflow.com/users/login?ssrc=site_switcher&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f7
3224410%2fhow-to-catch-a-panic
[21]: https://stackexchange.com/sites
[22]: https://stackoverflow.blog
[23]: https://stackoverflow.com/users/login?ssrc=head&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f73224410%2
fhow-to-catch-a-panic
[24]: https://stackoverflow.com/users/signup?ssrc=head&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f73224410%
2fhow-to-catch-a-panic
[25]: https://take.survey.stackoverflow.co/jfe/form/SV_4GHunpL3IfJ3rRc?utm_medium=referral&utm_source=stackoverflow-comm
unity&utm_campaign=dev-survey-2026&utm_content=announcement-banner
[26]: /
[27]: /questions
[28]: https://stackoverflow.com/ai-assist
[29]: /tags
[30]: http://agents.stackoverflow.com
[31]: /beta/challenges
[32]: https://chat.stackoverflow.com/?tab=explore
[33]: https://stackoverflow.blog/contributed?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=so-blog
&utm_content=experiment-articles
[34]: /users
[35]: https://stackoverflow.com/jobs/companies?so_medium=stackoverflow&so_source=SiteNav
[36]: javascript:void(0)
[37]: /collectives-all
[38]: https://stackoverflowteams.com/teams/create/free/?utm_medium=referral&utm_source=stackoverflow-community&utm_campa
ign=side-bar&utm_content=explore-teams
[39]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=side-bar&ut
m_content=explore-teams
[40]: javascript:void(0)
[41]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=side-bar&ut
m_content=explore-teams-compact
[42]: /collectives
[43]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=side-bar&ut
m_content=explore-teams-compact-popover
[44]: /questions/73224410/how-to-catch-a-panic
[45]: /questions/ask
[46]: ?lastactivity
[47]: /questions/tagged/rust
[48]: /q/73224410
[49]: /posts/73224410/edit
[50]: /posts/73224410/revisions
[51]: /users/2189130/kmdreko
[52]: /users/2189130/kmdreko
[53]: /users/19669053/fish0fqwerty
[54]: /users/19669053/fish0fqwerty
[55]: #
[56]: /questions/73224410/how-to-catch-a-panic?answertab=scoredesc#tab-top
[57]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html
[58]: /a/73224634
[59]: /posts/73224634/edit
[60]: /users/11423104/isaactfa
[61]: /users/11423104/isaactfa
[62]: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.get
[63]: https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html
[64]: /a/73224513
[65]: /posts/73224513/edit
[66]: /posts/73224513/revisions
[67]: /users/130641/john-ledbetter
[68]: /users/130641/john-ledbetter
[69]: /users/19669053/fish0fqwerty
[70]: #comment129320922_73224513
[71]: /users/130641/john-ledbetter
[72]: #comment129321016_73224513
[73]: /users/3931173/speciesunknown
[74]: #comment135481943_73224513
[75]: /users/60075/craig-mcqueen
[76]: #comment138638270_73224513
[77]: /help/gen-ai-policy
[78]: /help/how-to-answer
[79]: /users/login?ssrc=question_page&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f73224410%2fhow-to-catch-a-
panic%23new-answer
[80]: https://stackoverflow.com/legal/terms-of-service/public
[81]: https://stackoverflow.com/legal/privacy-policy
[82]: /questions/ask
[83]: /questions/tagged/rust
[84]: https://stackoverflow.blog/2026/06/25/code-isnt-causing-your-production-failures/
[85]: https://stackoverflow.blog/2026/06/26/paging-charity-how-can-engineering-leaders-avoid-becoming-bond-villains/
[86]: https://meta.stackexchange.com/questions/418826/partnering-with-communities-to-modernize-policies-norms
[87]: https://meta.stackoverflow.com/questions/421831/policy-generative-ai-e-g-chatgpt-is-banned
[88]: https://meta.stackoverflow.com/questions/439978/the-2026-developer-survey-is-live
[89]: /questions/76460649/when-to-use-stdexpected-instead-of-exceptions
[90]: /questions/76460649/when-to-use-stdexpected-instead-of-exceptions?noredirect=1
[91]: /questions/19720763/how-can-i-detect-an-error-rather-than-have-this-rust-program-abort
[92]: /questions/19720763/how-can-i-detect-an-error-rather-than-have-this-rust-program-abort
[93]: /questions/26469715/how-do-i-write-a-rust-unit-test-that-ensures-that-a-panic-has-occurred
[94]: /questions/26469715/how-do-i-write-a-rust-unit-test-that-ensures-that-a-panic-has-occurred
[95]: /questions/27384824/catching-panic-when-rust-called-from-c-ffi-without-spawning-threads
[96]: /questions/27384824/catching-panic-when-rust-called-from-c-ffi-without-spawning-threads
[97]: /questions/30232890/what-is-the-right-way-to-capture-panics
[98]: /questions/30232890/what-is-the-right-way-to-capture-panics
[99]: /questions/38116454/how-to-panic-in-production
[100]: /questions/38116454/how-to-panic-in-production
[101]: /questions/38514554/how-can-i-silently-catch-panics-in-quickcheck-tests
[102]: /questions/38514554/how-can-i-silently-catch-panics-in-quickcheck-tests
[103]: /questions/39844260/how-to-use-panic-abort-with-external-dependencies
[104]: /questions/39844260/how-to-use-panic-abort-with-external-dependencies
[105]: /questions/47663961/is-it-possible-to-specify-panic-abort-for-a-specific-target
[106]: /questions/47663961/is-it-possible-to-specify-panic-abort-for-a-specific-target
[107]: /questions/51860663/is-it-possible-to-check-if-panic-is-set-to-abort-while-a-library-is-compilin
[108]: /questions/51860663/is-it-possible-to-check-if-panic-is-set-to-abort-while-a-library-is-compilin
[109]: /questions/67328721/handling-panics-of-external-libraries
[110]: /questions/67328721/handling-panics-of-external-libraries
[111]: https://stackexchange.com/questions?tab=hot
[112]: https://puzzling.stackexchange.com/questions/138680/across-the-galaxy
[113]: https://workplace.stackexchange.com/questions/203501/client-frequently-paying-late-manager-claiming-i-make-mistak
es-but-doesnt-prov
[114]: https://hsm.stackexchange.com/questions/19511/who-invented-the-dot-product-and-cross-product-and-how
[115]: https://tex.stackexchange.com/questions/764233/how-to-draw-a-diagram-in-math-mode-or-in-line-latex-for-example-th
e-following
[116]: https://english.stackexchange.com/questions/640041/about-the-usage-of-embarrassed-with-a-preposition-and-or-witho
ut
[117]: https://christianity.stackexchange.com/questions/114175/according-to-catholicism-what-constitues-a-practicing-cat
holic
[118]: https://math.stackexchange.com/questions/5142084/fpqc-morphisms-are-epimorphisms
[119]: https://diy.stackexchange.com/questions/331112/how-to-get-the-upper-wheel-off-a-vintage-craftsman-bandsaw
[120]: https://hinduism.stackexchange.com/questions/70141/do-shudras-have-the-right-to-take-part-in-tantric-rituals
[121]: https://physics.stackexchange.com/questions/873816/larmor-precession-frequency-approximation-justified
[122]: https://mechanics.stackexchange.com/questions/102271/identifying-a-device-wired-into-my-prius-c
[123]: https://travel.stackexchange.com/questions/203961/is-reliable-and-reasonably-fast-internet-only-achieved-with-sta
rlink-dishes-in-d
[124]: https://mattermodeling.stackexchange.com/questions/14869/how-to-apply-f12-corrections-across-a-basis-set-series
[125]: https://academia.stackexchange.com/questions/227097/how-are-graduate-applicants-from-under-resourced-universities
-evaluated
[126]: https://tex.stackexchange.com/questions/764231/how-to-remove-the-n-specifier-when-calling-getdatan-h
[127]: https://engineering.stackexchange.com/questions/65769/does-the-rotation-direction-of-a-kitchen-exhaust-fan-affect
-its-ability-to-remov
[128]: https://tex.stackexchange.com/questions/764259/node-label-style-in-tikz-cd
[129]: https://tex.stackexchange.com/questions/764230/formatting-long-division-without-the-longdivision-package
[130]: https://diy.stackexchange.com/questions/331118/can-i-use-1x4-pine-boards-instead-of-plywood-cabinet-walls-to-supp
ort-a-cooktop
[131]: https://electronics.stackexchange.com/questions/770279/how-to-build-a-voltage-shifter-for-hs-driver
[132]: https://hermeneutics.stackexchange.com/questions/117159/revelations-48-question-on-biblical-greek
[133]: https://chinese.stackexchange.com/questions/64157/what-is-the-meaning-of-this-mark-in-a-bowl-of-porcelain-and-sil
ver-characters
[134]: https://scifi.stackexchange.com/questions/305028/fire-elemental-is-intimidated-by-magnesium-burning-under-water
[135]: https://physics.stackexchange.com/questions/873792/how-much-of-ingredients-of-the-standard-model-are-described-in
-the-perturbative
[136]: #
[137]: /feeds/question/73224410
[138]: https://stackoverflow.com
[139]: /questions
[140]: /help
[141]: https://chat.stackoverflow.com/?tab=explore
[142]: https://stackoverflow.co/
[143]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=footer&utm
_content=teams
[144]: https://stackoverflow.co/data-licensing/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=foot
er&utm_content=data-licensing
[145]: https://stackoverflow.co/advertising/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=footer&
utm_content=advertising
[146]: https://stackoverflow.co/
[147]: https://stackoverflow.co/
[148]: https://stackoverflow.co/company/press/
[149]: https://stackoverflow.co/company/work-here/
[150]: https://stackoverflow.com/legal
[151]: https://stackoverflow.com/legal/privacy-policy
[152]: https://stackoverflow.com/legal/terms-of-service/public
[153]: /contact
[154]: https://policies.stackoverflow.co/stack-overflow/cookie-policy
[155]: https://stackexchange.com
[156]: https://stackexchange.com/sites#technology
[157]: https://stackexchange.com/sites#culturerecreation
[158]: https://stackexchange.com/sites#lifearts
[159]: https://stackexchange.com/sites#science
[160]: https://stackexchange.com/sites#professional
[161]: https://stackexchange.com/sites#business
[162]: https://api.stackexchange.com/
[163]: https://data.stackexchange.com/
[164]: https://stackoverflow.blog?blb=1
[165]: https://www.facebook.com/officialstackoverflow/
[166]: https://twitter.com/stackoverflow
[167]: https://linkedin.com/company/stack-overflow
[168]: https://www.instagram.com/thestackoverflow
[169]: https://stackoverflow.com/help/licensing
```
