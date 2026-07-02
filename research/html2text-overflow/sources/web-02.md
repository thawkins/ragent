# Web source

- URL: https://stackoverflow.com/questions/41301485/panicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T16:21:00.043462806+00:00

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

# [Panicked at 'attempt to subtract with overflow' when cycling backwards though a list][44]

[ Ask Question ][45]
Asked 9 years, 6 months ago
Modified [2 years, 2 months ago][46]
Viewed 59k times
44

I am writing a cycle method for a list that moves an index either forwards or backwards. The following code is used to
cycle backwards:

`(i-1)%list_length
`

In this case, `i` is of the type `usize`, meaning it is unsigned. If `i` is equal to 0, this leads to an 'attempt to
subtract with overflow' error. I tried to use the correct casting methods to work around this problem:

`((i as isize)-1)%(list_length as isize)) as usize
`

This results in an integer overflow.

I understand why the errors happen, and at the moment I've solved the problem by checking if the index is equal to 0,
but I was wondering if there was some way to solve it by casting the variables to the correct types.
* [rust][47]
* [integer-overflow][48]
* [integer-arithmetic][49]

[Share][50]
[Improve this question][51]
Follow
[edited Dec 23, 2016 at 14:54][52]
[
[Shepmaster's user avatar]
][53]
[Shepmaster][54]
442k119119 gold badges1.3k1.3k silver badges1.5k1.5k bronze badges
asked Dec 23, 2016 at 12:24
[
[lmartens's user avatar]
][55]
[lmartens][56]
1,51222 gold badges1313 silver badges2020 bronze badges
2
* 9
  As an aside: I don't think you want to do that at all. `(-1 % 10)` is `-1`, not `9`. `-1isize as usize` is
  `18446744073709551615` (on 64-bit architectures).
  DK.
  –  [DK.][57]
  2016-12-23 12:33:47 +00:00
  Commented Dec 23, 2016 at 12:33
* 1
  Ok, I didn't know. I thought it worked like described in [this post][58], but I see now that it is implemented like
  described in [this post][59]. That clears it up!
  lmartens
  –  [lmartens][60]
  2016-12-23 14:03:12 +00:00
  Commented Dec 23, 2016 at 14:03
[ Add a comment ][61]  | 

## 3 Answers 3

Sorted by: [ Reset to default ][62]
Highest score (default) Trending (recent votes count more) Date modified (newest first) Date created (oldest first)
30

As [DK. points out][63], you don't want wrapping semantics at the integer level:

`fn main() {
    let idx: usize = 0;
    let len = 10;

    let next_idx = idx.wrapping_sub(1) % len;
    println!("{}", next_idx) // Prints 5!!!
}
`

Instead, you want to use modulo logic to wrap around:

`let next_idx = (idx + len - 1) % len;
`

This only works if `len` + `idx` is less than the max of the type — this is much easier to see with a `u8` instead of
`usize`; just set `idx` to 200 and `len` to 250.

If you can't guarantee that the sum of the two values will always be less than the maximum value, I'd probably use the
"checked" family of operations. This does the same level of conditional checking you mentioned you already have, but is
neatly tied into a single line:

`let next_idx = idx.checked_sub(1).unwrap_or(len - 1);
`

[Share][64]
[Improve this answer][65]
Follow
[edited May 23, 2017 at 12:16][66]
[
[Community's user avatar]
][67]
[Community][68]Bot
111 silver badge
answered Dec 23, 2016 at 14:17
[
[Shepmaster's user avatar]
][69]
[Shepmaster][70]
442k119119 gold badges1.3k1.3k silver badges1.5k1.5k bronze badges
Sign up to request clarification or add additional context in comments.

## Comments

Add a comment

10

If your code can have overflowing operations, I would suggest using [`Wrapping`][71]. You don't need to worry about
casting or overflow panics when you allow it:

`use std::num::Wrapping;

let zero = Wrapping(0u32);
let one = Wrapping(1u32);

assert_eq!(std::u32::MAX, (zero - one).0);
`

[Share][72]
[Improve this answer][73]
Follow
[edited Dec 23, 2016 at 14:06][74]
[
[Shepmaster's user avatar]
][75]
[Shepmaster][76]
442k119119 gold badges1.3k1.3k silver badges1.5k1.5k bronze badges
answered Dec 23, 2016 at 12:31
[
[ljedrz's user avatar]
][77]
[ljedrz][78]
22.8k55 gold badges7878 silver badges110110 bronze badges

## 1 Comment

Add a comment

Shepmaster
[Shepmaster][79] [Over a year ago][80]
There's also [inherent methods on each type][81] for wrapping arithmetic.
2016-12-23T14:06:58.047Z+00:00
7
Reply
* Copy link
-1

Had a similar issue, the equivalent of

`(i+list_length-1)%list_length
`

solved it for me

[Share][82]
[Improve this answer][83]
Follow
answered Apr 12, 2024 at 21:15
[
[EmF's user avatar]
][84]
[EmF][85]
1

## 1 Comment

Add a comment

mfluehr
[mfluehr][86] [Over a year ago][87]
This doesn't add anything to Shepmaster's answer.
2024-05-03T15:10:15.393Z+00:00
0
Reply
* Copy link

## Your Answer

**Reminder:** Answers generated by AI tools are not allowed due to Stack Overflow's [artificial intelligence policy][88]

Thanks for contributing an answer to Stack Overflow!
* Please be sure to *answer the question*. Provide details and share your research!

But *avoid* …
* Asking for help, clarification, or responding to other answers.
* Making statements based on opinion; back them up with references or personal experience.

To learn more, see our [tips on writing great answers][89].

Draft saved
Draft discarded

### Sign up or [log in][90]

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

By clicking “Post Your Answer”, you agree to our [terms of service][91] and acknowledge you have read our [privacy
policy][92].

Start asking to get answers

Find the answer to your question by asking.

[Ask question][93]

Explore related questions
* [rust][94]
* [integer-overflow][95]
* [integer-arithmetic][96]

See similar questions with these tags.
* The Overflow Blog
* [Code isn’t the only thing causing your production...][97]
* [Paging Charity! How can engineering leaders avoid becoming Bond...][98]
* Featured on Meta
* [Partnering with Communities to Modernize Policies & Norms][99]
* [Policy: Generative AI (e.g., ChatGPT) is banned][100]
* [The 2026 Developer Survey is Live][101]

### Linked

[
154
][102] [Is there a modulus (not remainder) function / operation?][103]
[
0
][104] [Rust-lang thread 'main' panicked at 'attempt to subtract with overflow' when .collect with usize type][105]
[
1
][106] [Why does insertion sort algorithm give overflow error?][107]

### Related

[
0
][108] [Unsigned integer overflow in Rust][109]
[
3
][110] [How to resolve a possible multiplicative overflow to get correct modulus operation?][111]
[
0
][112] [Arithmetic overflow expected but does not occur][113]
[
3
][114] [Integer operation with boundary when overflow in Rust][115]
[
2
][116] [My test fails at "attempt to subtract with overflow"][117]
[
10
][118] [What happens in Rust programming language when an integer arithmetic operation overflows?][119]
[
1
][120] [Attempt to subtract with overflow][121]
[
4
][122] [Why does repeated multiplication panic due to overflow in debug mode, when it outputs only zeroes in release
mode?][123]
[
1
][124] [How do you check if an arithmetic operation will overflow?][125]
[
0
][126] [Multiply with overflow: Rust Runtime error][127]

#### [ Hot Network Questions ][128]
* [ Is there a way to calculate pointwise mutual information on a public corpus? ][129]
* [ What is the definition of "excessive amount" in DELTARUNE Chapter 5: Festival Day? ][130]
* [ Who invented the dot product and cross product and how? ][131]
* [ Larmor precession frequency approximation justified ][132]
* [ How are graduate applicants from under-resourced universities evaluated? ][133]
* [ 2000 years of Torah ][134]
* [ A Line showing only on colored background ][135]
* [ What is Bitcoin’s objective definition of transaction neutrality? ][136]
* [ Is Intel Thread Director (ITD) actually used by the scheduler on Linux? ][137]
* [ Is there a subset of the plane whose intersection with every line is countable, yet it has positive Lebesgue outer
  measure? ][138]
* [ Equation of an ellipse from the intersection of a cone and a plane ][139]
* [ At what age is it optimal to repot plants? ][140]
* [ How do I reset the Claude Code VS Code extension to use my Claude subscription instead of a deleted API key's
  deployment? ][141]
* [ Apophatic mathematics ][142]
* [ Some groups admit of only two epimorphisms? ][143]
* [ How to properly insert wire into this Eaton breaker? ][144]
* [ How to fix model parameter estimates MPlus bi-factor CFA ][145]
* [ On the sum of divisors of powers ][146]
* [ Can the epistemic warrant to never be certain be justified without circularity? ][147]
* [ Was Ms. Ellen really a lesbian? ][148]
* [ Do Shudras have the right to take part in Tantric rituals? ][149]
* [ Gaps and overlapping faces after mirroring ][150]
* [ How to build a voltage shifter for HS driver? ][151]
* [ What is the meaning of this mark in a bowl of porcelain and silver? (Characters identified: 天府紋銀) ][152]

[ more hot questions ][153]
[ Question feed ][154]

# Subscribe to RSS

Question feed

To subscribe to this RSS feed, copy and paste this URL into your RSS reader.

lang-rust

##### [Stack Overflow][155]
* [Questions][156]
* [Help][157]
* [Chat][158]

##### [Business][159]
* [Stack Internal][160]
* [Stack Data Licensing][161]
* [Stack Ads][162]

##### [Company][163]
* [About][164]
* [Press][165]
* [Work Here][166]
* [Legal][167]
* [Privacy Policy][168]
* [Terms of Service][169]
* [Contact Us][170]
* Cookie Settings
* [Cookie Policy][171]

##### [Stack Exchange Network][172]
* [ Technology ][173]
* [ Culture & recreation ][174]
* [ Life & arts ][175]
* [ Science ][176]
* [ Professional ][177]
* [ Business ][178]
* [ API ][179]
* [ Data ][180]
* [Blog][181]
* [Facebook][182]
* [Twitter][183]
* [LinkedIn][184]
* [Instagram][185]

Site design / logo © 2026 Stack Exchange Inc; user contributions licensed under [CC BY-SA][186] . rev 2026.6.25.43791

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
41301485%2fpanicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
[20]: https://stackoverflow.com/users/login?ssrc=site_switcher&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f4
1301485%2fpanicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
[21]: https://stackexchange.com/sites
[22]: https://stackoverflow.blog
[23]: https://stackoverflow.com/users/login?ssrc=head&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f41301485%2
fpanicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
[24]: https://stackoverflow.com/users/signup?ssrc=head&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f41301485%
2fpanicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
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
[44]: /questions/41301485/panicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a
[45]: /questions/ask
[46]: ?lastactivity
[47]: /questions/tagged/rust
[48]: /questions/tagged/integer-overflow
[49]: /questions/tagged/integer-arithmetic
[50]: /q/41301485
[51]: /posts/41301485/edit
[52]: /posts/41301485/revisions
[53]: /users/155423/shepmaster
[54]: /users/155423/shepmaster
[55]: /users/5536887/lmartens
[56]: /users/5536887/lmartens
[57]: /users/42353/dk
[58]: http://math.stackexchange.com/questions/519845/modulo-of-a-negative-number
[59]: http://stackoverflow.com/questions/31210357/is-there-a-modulus-not-remainder-function-operation
[60]: /users/5536887/lmartens
[61]: #
[62]: /questions/41301485/panicked-at-attempt-to-subtract-with-overflow-when-cycling-backwards-though-a?answertab=scored
esc#tab-top
[63]: https://stackoverflow.com/questions/41301485/rust-panicked-at-attempt-to-subtract-with-overflow#comment69805526_41
301485
[64]: /a/41303083
[65]: /posts/41303083/edit
[66]: /posts/41303083/revisions
[67]: /users/-1/community
[68]: /users/-1/community
[69]: /users/155423/shepmaster
[70]: /users/155423/shepmaster
[71]: https://doc.rust-lang.org/stable/std/num/struct.Wrapping.html
[72]: /a/41301585
[73]: /posts/41301585/edit
[74]: /posts/41301585/revisions
[75]: /users/155423/shepmaster
[76]: /users/155423/shepmaster
[77]: /users/1870153/ljedrz
[78]: /users/1870153/ljedrz
[79]: /users/155423/shepmaster
[80]: #comment69808080_41301585
[81]: https://doc.rust-lang.org/std/primitive.usize.html#method.wrapping_sub
[82]: /a/78318749
[83]: /posts/78318749/edit
[84]: /users/15854311/emf
[85]: /users/15854311/emf
[86]: /users/3469273/mfluehr
[87]: #comment138262139_78318749
[88]: /help/gen-ai-policy
[89]: /help/how-to-answer
[90]: /users/login?ssrc=question_page&returnurl=https%3a%2f%2fstackoverflow.com%2fquestions%2f41301485%2fpanicked-at-att
empt-to-subtract-with-overflow-when-cycling-backwards-though-a%23new-answer
[91]: https://stackoverflow.com/legal/terms-of-service/public
[92]: https://stackoverflow.com/legal/privacy-policy
[93]: /questions/ask
[94]: /questions/tagged/rust
[95]: /questions/tagged/integer-overflow
[96]: /questions/tagged/integer-arithmetic
[97]: https://stackoverflow.blog/2026/06/25/code-isnt-causing-your-production-failures/
[98]: https://stackoverflow.blog/2026/06/26/paging-charity-how-can-engineering-leaders-avoid-becoming-bond-villains/
[99]: https://meta.stackexchange.com/questions/418826/partnering-with-communities-to-modernize-policies-norms
[100]: https://meta.stackoverflow.com/questions/421831/policy-generative-ai-e-g-chatgpt-is-banned
[101]: https://meta.stackoverflow.com/questions/439978/the-2026-developer-survey-is-live
[102]: /questions/31210357/is-there-a-modulus-not-remainder-function-operation
[103]: /questions/31210357/is-there-a-modulus-not-remainder-function-operation?noredirect=1
[104]: /questions/72586683/rust-lang-thread-main-panicked-at-attempt-to-subtract-with-overflow-when-co
[105]: /questions/72586683/rust-lang-thread-main-panicked-at-attempt-to-subtract-with-overflow-when-co?noredirect=1
[106]: /questions/44600404/why-does-insertion-sort-algorithm-give-overflow-error
[107]: /questions/44600404/why-does-insertion-sort-algorithm-give-overflow-error?noredirect=1
[108]: /questions/53568885/unsigned-integer-overflow-in-rust
[109]: /questions/53568885/unsigned-integer-overflow-in-rust
[110]: /questions/54487936/how-to-resolve-a-possible-multiplicative-overflow-to-get-correct-modulus-operati
[111]: /questions/54487936/how-to-resolve-a-possible-multiplicative-overflow-to-get-correct-modulus-operati
[112]: /questions/63055397/arithmetic-overflow-expected-but-does-not-occur
[113]: /questions/63055397/arithmetic-overflow-expected-but-does-not-occur
[114]: /questions/63719869/integer-operation-with-boundary-when-overflow-in-rust
[115]: /questions/63719869/integer-operation-with-boundary-when-overflow-in-rust
[116]: /questions/66754977/my-test-fails-at-attempt-to-subtract-with-overflow
[117]: /questions/66754977/my-test-fails-at-attempt-to-subtract-with-overflow
[118]: /questions/68807024/what-happens-in-rust-programming-language-when-an-integer-arithmetic-operation-o
[119]: /questions/68807024/what-happens-in-rust-programming-language-when-an-integer-arithmetic-operation-o
[120]: /questions/69155800/attempt-to-subtract-with-overflow
[121]: /questions/69155800/attempt-to-subtract-with-overflow
[122]: /questions/71196238/why-does-repeated-multiplication-panic-due-to-overflow-in-debug-mode-when-it-ou
[123]: /questions/71196238/why-does-repeated-multiplication-panic-due-to-overflow-in-debug-mode-when-it-ou
[124]: /questions/71899179/how-do-you-check-if-an-arithmetic-operation-will-overflow
[125]: /questions/71899179/how-do-you-check-if-an-arithmetic-operation-will-overflow
[126]: /questions/74845088/multiply-with-overflow-rust-runtime-error
[127]: /questions/74845088/multiply-with-overflow-rust-runtime-error
[128]: https://stackexchange.com/questions?tab=hot
[129]: https://linguistics.stackexchange.com/questions/51768/is-there-a-way-to-calculate-pointwise-mutual-information-on
-a-public-corpus
[130]: https://gaming.stackexchange.com/questions/419106/what-is-the-definition-of-excessive-amount-in-deltarune-chapter
-5-festival-da
[131]: https://hsm.stackexchange.com/questions/19511/who-invented-the-dot-product-and-cross-product-and-how
[132]: https://physics.stackexchange.com/questions/873816/larmor-precession-frequency-approximation-justified
[133]: https://academia.stackexchange.com/questions/227097/how-are-graduate-applicants-from-under-resourced-universities
-evaluated
[134]: https://judaism.stackexchange.com/questions/156441/2000-years-of-torah
[135]: https://mathematica.stackexchange.com/questions/319655/a-line-showing-only-on-colored-background
[136]: https://bitcoin.stackexchange.com/questions/130849/what-is-bitcoin-s-objective-definition-of-transaction-neutrali
ty
[137]: https://unix.stackexchange.com/questions/806535/is-intel-thread-director-itd-actually-used-by-the-scheduler-on-li
nux
[138]: https://mathoverflow.net/questions/512707/is-there-a-subset-of-the-plane-whose-intersection-with-every-line-is-co
untable
[139]: https://math.stackexchange.com/questions/5142102/equation-of-an-ellipse-from-the-intersection-of-a-cone-and-a-pla
ne
[140]: https://gardening.stackexchange.com/questions/70857/at-what-age-is-it-optimal-to-repot-plants
[141]: https://superuser.com/questions/1938734/how-do-i-reset-the-claude-code-vs-code-extension-to-use-my-claude-subscri
ption-i
[142]: https://mathoverflow.net/questions/512702/apophatic-mathematics
[143]: https://math.stackexchange.com/questions/5142048/some-groups-admit-of-only-two-epimorphisms
[144]: https://diy.stackexchange.com/questions/331117/how-to-properly-insert-wire-into-this-eaton-breaker
[145]: https://stats.stackexchange.com/questions/676368/how-to-fix-model-parameter-estimates-mplus-bi-factor-cfa
[146]: https://mathoverflow.net/questions/512720/on-the-sum-of-divisors-of-powers
[147]: https://philosophy.stackexchange.com/questions/139375/can-the-epistemic-warrant-to-never-be-certain-be-justified-
without-circularity
[148]: https://movies.stackexchange.com/questions/132073/was-ms-ellen-really-a-lesbian
[149]: https://hinduism.stackexchange.com/questions/70141/do-shudras-have-the-right-to-take-part-in-tantric-rituals
[150]: https://blender.stackexchange.com/questions/347395/gaps-and-overlapping-faces-after-mirroring
[151]: https://electronics.stackexchange.com/questions/770279/how-to-build-a-voltage-shifter-for-hs-driver
[152]: https://chinese.stackexchange.com/questions/64157/what-is-the-meaning-of-this-mark-in-a-bowl-of-porcelain-and-sil
ver-characters
[153]: #
[154]: /feeds/question/41301485
[155]: https://stackoverflow.com
[156]: /questions
[157]: /help
[158]: https://chat.stackoverflow.com/?tab=explore
[159]: https://stackoverflow.co/
[160]: https://stackoverflow.co/internal/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=footer&utm
_content=teams
[161]: https://stackoverflow.co/data-licensing/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=foot
er&utm_content=data-licensing
[162]: https://stackoverflow.co/advertising/?utm_medium=referral&utm_source=stackoverflow-community&utm_campaign=footer&
utm_content=advertising
[163]: https://stackoverflow.co/
[164]: https://stackoverflow.co/
[165]: https://stackoverflow.co/company/press/
[166]: https://stackoverflow.co/company/work-here/
[167]: https://stackoverflow.com/legal
[168]: https://stackoverflow.com/legal/privacy-policy
[169]: https://stackoverflow.com/legal/terms-of-service/public
[170]: /contact
[171]: https://policies.stackoverflow.co/stack-overflow/cookie-policy
[172]: https://stackexchange.com
[173]: https://stackexchange.com/sites#technology
[174]: https://stackexchange.com/sites#culturerecreation
[175]: https://stackexchange.com/sites#lifearts
[176]: https://stackexchange.com/sites#science
[177]: https://stackexchange.com/sites#professional
[178]: https://stackexchange.com/sites#business
[179]: https://api.stackexchange.com/
[180]: https://data.stackexchange.com/
[181]: https://stackoverflow.blog?blb=1
[182]: https://www.facebook.com/officialstackoverflow/
[183]: https://twitter.com/stackoverflow
[184]: https://linkedin.com/company/stack-overflow
[185]: https://www.instagram.com/thestackoverflow
[186]: https://stackoverflow.com/help/licensing
```
