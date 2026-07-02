# Web source

- URL: https://github.com/rust-lang/rust/issues/90878
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:21:01.980395273+00:00

```text
[Skip to content][1]

## Navigation Menu

Toggle navigation
[ Sign in ][2]
Appearance settings
* Platform
  * AI CODE CREATION
    * [
      GitHub CopilotWrite better code with AI
      ][3]
    * [
      GitHub Copilot appDirect agents from issue to merge
      ][4]
    * [
      MCP Registry^{New}Integrate external tools
      ][5]
  * DEVELOPER WORKFLOWS
    * [
      ActionsAutomate any workflow
      ][6]
    * [
      CodespacesInstant dev environments
      ][7]
    * [
      IssuesPlan and track work
      ][8]
    * [
      Code ReviewManage code changes
      ][9]
  * APPLICATION SECURITY
    * [
      GitHub Advanced SecurityFind and fix vulnerabilities
      ][10]
    * [
      Code securitySecure your code as you build
      ][11]
    * [
      Secret protectionStop leaks before they start
      ][12]
  * EXPLORE
    * [Why GitHub][13]
    * [Documentation][14]
    * [Blog][15]
    * [Changelog][16]
    * [Marketplace][17]
  [View all features][18]
* Solutions
  * BY COMPANY SIZE
    * [Enterprises][19]
    * [Small and medium teams][20]
    * [Startups][21]
    * [Nonprofits][22]
  * BY USE CASE
    * [App Modernization][23]
    * [DevSecOps][24]
    * [DevOps][25]
    * [CI/CD][26]
    * [View all use cases][27]
  * BY INDUSTRY
    * [Healthcare][28]
    * [Financial services][29]
    * [Manufacturing][30]
    * [Government][31]
    * [View all industries][32]
  [View all solutions][33]
* Resources
  * EXPLORE BY TOPIC
    * [AI][34]
    * [Software Development][35]
    * [DevOps][36]
    * [Security][37]
    * [View all topics][38]
  * EXPLORE BY TYPE
    * [Customer stories][39]
    * [Events & webinars][40]
    * [Ebooks & reports][41]
    * [Business insights][42]
    * [GitHub Skills][43]
  * SUPPORT & SERVICES
    * [Documentation][44]
    * [Customer support][45]
    * [Community forum][46]
    * [Trust center][47]
    * [Partners][48]
  [View all resources][49]
* Open Source
  * COMMUNITY
    * [
      GitHub SponsorsFund open source developers
      ][50]
  * PROGRAMS
    * [Security Lab][51]
    * [Maintainer Community][52]
    * [Accelerator][53]
    * [GitHub Stars][54]
    * [Archive Program][55]
  * REPOSITORIES
    * [Topics][56]
    * [Trending][57]
    * [Collections][58]
* Enterprise
  * ENTERPRISE SOLUTIONS
    * [
      Enterprise platformAI-powered developer platform
      ][59]
  * AVAILABLE ADD-ONS
    * [
      GitHub Advanced SecurityEnterprise-grade security features
      ][60]
    * [
      Copilot for BusinessEnterprise-grade AI features
      ][61]
    * [
      Premium SupportEnterprise-grade 24/7 support
      ][62]
* [Pricing][63]
Search or jump to...

# Search code, repositories, users, issues, pull requests...

Search
Clear
[Search syntax tips][64]

# Provide feedback

We read every piece of feedback, and take your input very seriously.

Include my email address so I can be contacted
Cancel Submit feedback

# Saved searches

## Use saved searches to filter your results more quickly

Name
Query

To see all available qualifiers, see our [documentation][65].

Cancel Create saved search
[ Sign in ][66]
[ Sign up ][67]
Appearance settings
Resetting focus
You signed in with another tab or window. [Reload][68] to refresh your session. You signed out in another tab or window.
[Reload][69] to refresh your session. You switched accounts on another tab or window. [Reload][70] to refresh your
session. Dismiss alert

### Uh oh!


There was an error while loading. [Please reload this page][71].

[ rust-lang ][72] / ** [rust][73] ** Public
* ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][74].
* [ Notifications ][75] You must be signed in to change notification settings
* [ Fork 15k ][76]
* [ Star 114k ][77]
* [ Code ][78]
* [ Issues 5k+ ][79]
* [ Pull requests 1.2k ][80]
* [ Actions ][81]
* [ Projects ][82]
* [ Security and quality 6 ][83]
* [ Insights ][84]
Additional navigation options
* [ Code ][85]
* [ Issues ][86]
* [ Pull requests ][87]
* [ Actions ][88]
* [ Projects ][89]
* [ Security and quality ][90]
* [ Insights ][91]

# thread 'rustc' panicked at 'attempt to subtract with overflow',
# rust/compiler/rustc_resolve/src/diagnostics.rs:458:49 #90878

New issue
Copy link
New issue
Copy link
Closed
[#90930][92]
Closed
[thread 'rustc' panicked at 'attempt to subtract with overflow',
rust/compiler/rustc_resolve/src/diagnostics.rs:458:49][93]#90878
[#90930][94]
Copy link
Assignees
[[Noratrieb]][95]
Labels
[C-bugCategory: This is a bug.][96]Category: This is a bug.[I-ICEIssue: The compiler panicked, giving an Internal
Compilation Error (ICE) ❄️][97]Issue: The compiler panicked, giving an Internal Compilation Error (ICE)
❄️[T-compilerRelevant to the compiler team, which will review and decide on the PR/issue.][98]Relevant to the compiler
team, which will review and decide on the PR/issue.

## Description

[[@Badel2]][99]
[Badel2][100]
opened [on Nov 13, 2021][101]
Issue body actions

### Code

 fn main() {
    |x: usize| [0; x];
    // (note the space before "fn")
}

When rustc is compiled with debug assertions, this results in a subtract with overflow on this line:

[rust/compiler/rustc_resolve/src/diagnostics.rs][102]

Line 458 in [1b12d01][103]

─────────────────────────────────────────────────────────────
let sp = sp.with_lo(BytePos(sp.lo().0 - current.len() as     
u32));                                                       
─────────────────────────────────────────────────────────────

And if compiled without debug assertions, you get a warning "Invalid span".

Introduced in [#80801][104]

Affected versions: stable 1.56.1, nightly 2021-11-12

**Backtrace**


With debug assertions:

`thread 'rustc' panicked at 'attempt to subtract with overflow', rust/compiler/rustc_resolve/src/diagnostics.rs:458:49
stack backtrace:
   0: rust_begin_unwind
             at /rustc/46b8e7488eae116722196e8390c1bd2ea2e396cf/library/std/src/panicking.rs:498:5
   1: core::panicking::panic_fmt
             at /rustc/46b8e7488eae116722196e8390c1bd2ea2e396cf/library/core/src/panicking.rs:106:14
   2: core::panicking::panic
             at /rustc/46b8e7488eae116722196e8390c1bd2ea2e396cf/library/core/src/panicking.rs:47:5
   3: rustc_resolve::diagnostics::<impl rustc_resolve::Resolver>::into_struct_error
   4: rustc_resolve::diagnostics::<impl rustc_resolve::Resolver>::report_error
   5: rustc_resolve::Resolver::resolve_ident_in_lexical_scope
   6: rustc_resolve::Resolver::resolve_path_with_ribs::{{closure}}
   7: rustc_resolve::Resolver::resolve_path_with_ribs
   8: rustc_resolve::late::LateResolutionVisitor::resolve_qpath_anywhere
   9: rustc_resolve::late::LateResolutionVisitor::smart_resolve_path_fragment
  10: rustc_resolve::late::LateResolutionVisitor::resolve_expr
  11: rustc_resolve::late::LateResolutionVisitor::resolve_anon_const
  12: rustc_resolve::late::LateResolutionVisitor::resolve_expr
  13: <rustc_resolve::late::LateResolutionVisitor as rustc_ast::visit::Visitor>::visit_fn
  14: rustc_ast::visit::walk_expr
  15: rustc_resolve::late::LateResolutionVisitor::resolve_expr
  16: rustc_resolve::late::LateResolutionVisitor::resolve_block
  17: <rustc_resolve::late::LateResolutionVisitor as rustc_ast::visit::Visitor>::visit_fn
  18: rustc_ast::visit::walk_item
  19: <rustc_resolve::late::LateResolutionVisitor as rustc_ast::visit::Visitor>::visit_item
  20: rustc_resolve::Resolver::resolve_crate::{{closure}}
  21: rustc_resolve::Resolver::resolve_crate
  22: rustc_interface::passes::configure_and_expand
  23: rustc_interface::queries::Queries::expansion
  24: rustc_interface::interface::run_compiler::{{closure}}
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
`

Without debug assertions, it only shows a warning "Invalid span":

`   Compiling playground v0.0.1 (/playground)
WARN rustc_errors::emitter Invalid span src/main.rs:2:7: 292:4282133246 (#0), error=DistinctSources(DistinctSources { be
gin: (Real(LocalPath("src/main.rs")), BytePos(0)), end: (Real(Remapped { local_path: None, virtual_name: "/rustc/e90c5fb
bc5df5c81267747daeb937d4e955ce6ad/library/unwind/src/libunwind.rs" }), BytePos(12823298)) })
error[E0435]: attempt to use a non-constant value in a constant
   --> src/main.rs:2:20
    |
2   |       |x: usize| [0; x];
    |         -            ^ non-constant value
    |  _______|
    | |
3   | |     // (note the space before "fn")
4   | | }
...   |

For more information about this error, try `rustc --explain E0435`.
error: could not compile `playground` due to previous error
`

This issue was found thanks to [fuzz-rustc][105], but the actual minimized code was hard to understand (` #![l=|x|[b;x`)
so I unminimized it a bit. A related issue I found is that the span is wrong when there is whitespace between "let" and
"x" here:

fn main() {
    let          x = 0;
    [0; x];
}

`error[E0435]: attempt to use a non-constant value in a constant
 --> src/main.rs:3:9
  |
2 |     let          x = 0;
  |              ----- help: consider using `const` instead of `let`: `const x`
3 |     [0; x];
  |         ^ non-constant value
`

Reactions are currently unavailable

## Metadata

## Metadata

### Assignees
* [[@Noratrieb]
  Noratrieb
  ][106]

### Labels

[C-bugCategory: This is a bug.][107]Category: This is a bug.[I-ICEIssue: The compiler panicked, giving an Internal
Compilation Error (ICE) ❄️][108]Issue: The compiler panicked, giving an Internal Compilation Error (ICE)
❄️[T-compilerRelevant to the compiler team, which will review and decide on the PR/issue.][109]Relevant to the compiler
team, which will review and decide on the PR/issue.

### Type

No type

### Fields

[Give feedback][110]
No fields configured for issues without a type.

### Projects

No projects

### Milestone

No milestone

### Relationships

None yet

### Development

No branches or pull requests

## Issue actions

## Footer

© 2026 GitHub, Inc.

### Footer navigation
* [Terms][111]
* [Privacy][112]
* [Security][113]
* [Status][114]
* [Community][115]
* [Docs][116]
* [Contact][117]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Frust%2Fissues%2F90878
[3]: https://github.com/features/copilot
[4]: https://github.com/features/ai/github-app
[5]: https://github.com/mcp
[6]: https://github.com/features/actions
[7]: https://github.com/features/codespaces
[8]: https://github.com/features/issues
[9]: https://github.com/features/code-review
[10]: https://github.com/security/advanced-security
[11]: https://github.com/security/advanced-security/code-security
[12]: https://github.com/security/advanced-security/secret-protection
[13]: https://github.com/why-github
[14]: https://docs.github.com
[15]: https://github.blog
[16]: https://github.blog/changelog
[17]: https://github.com/marketplace
[18]: https://github.com/features
[19]: https://github.com/enterprise
[20]: https://github.com/team
[21]: https://github.com/enterprise/startups
[22]: https://github.com/solutions/industry/nonprofits
[23]: https://github.com/solutions/use-case/app-modernization
[24]: https://github.com/solutions/use-case/devsecops
[25]: https://github.com/solutions/use-case/devops
[26]: https://github.com/solutions/use-case/ci-cd
[27]: https://github.com/solutions/use-case
[28]: https://github.com/solutions/industry/healthcare
[29]: https://github.com/solutions/industry/financial-services
[30]: https://github.com/solutions/industry/manufacturing
[31]: https://github.com/solutions/industry/government
[32]: https://github.com/solutions/industry
[33]: https://github.com/solutions
[34]: https://github.com/resources/articles?topic=ai
[35]: https://github.com/resources/articles?topic=software-development
[36]: https://github.com/resources/articles?topic=devops
[37]: https://github.com/resources/articles?topic=security
[38]: https://github.com/resources/articles
[39]: https://github.com/customer-stories
[40]: https://github.com/resources/events
[41]: https://github.com/resources/whitepapers
[42]: https://github.com/solutions/executive-insights
[43]: https://skills.github.com
[44]: https://docs.github.com
[45]: https://support.github.com
[46]: https://github.com/orgs/community/discussions
[47]: https://github.com/trust-center
[48]: https://github.com/partners
[49]: https://github.com/resources
[50]: https://github.com/sponsors
[51]: https://securitylab.github.com
[52]: https://maintainers.github.com
[53]: https://github.com/accelerator
[54]: https://stars.github.com
[55]: https://archiveprogram.github.com
[56]: https://github.com/topics
[57]: https://github.com/trending
[58]: https://github.com/collections
[59]: https://github.com/enterprise
[60]: https://github.com/security/advanced-security
[61]: https://github.com/features/copilot/copilot-business
[62]: https://github.com/premium-support
[63]: https://github.com/pricing
[64]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[65]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Frust%2Fissues%2F90878
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fvoltron%2Fissues
_fragments%2Fissue_layout&source=header-repo&source_repo=rust-lang%2Frust
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /rust-lang
[73]: /rust-lang/rust
[74]: 
[75]: /login?return_to=%2Frust-lang%2Frust
[76]: /login?return_to=%2Frust-lang%2Frust
[77]: /login?return_to=%2Frust-lang%2Frust
[78]: /rust-lang/rust
[79]: /rust-lang/rust/issues
[80]: /rust-lang/rust/pulls
[81]: /rust-lang/rust/actions
[82]: /rust-lang/rust/projects
[83]: /rust-lang/rust/security
[84]: /rust-lang/rust/pulse
[85]: /rust-lang/rust
[86]: /rust-lang/rust/issues
[87]: /rust-lang/rust/pulls
[88]: /rust-lang/rust/actions
[89]: /rust-lang/rust/projects
[90]: /rust-lang/rust/security
[91]: /rust-lang/rust/pulse
[92]: https://github.com/rust-lang/rust/pull/90930
[93]: #top
[94]: https://github.com/rust-lang/rust/pull/90930
[95]: /Noratrieb
[96]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22C-bug%22
[97]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22I-ICE%22
[98]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22T-compiler%22
[99]: https://github.com/Badel2
[100]: https://github.com/Badel2
[101]: https://github.com/rust-lang/rust/issues/90878#issue-1052760666
[102]: https://github.com/rust-lang/rust/blob/1b12d01903293453dd94aa170c82caf94415629f/compiler/rustc_resolve/src/diagno
stics.rs#L458
[103]: /rust-lang/rust/commit/1b12d01903293453dd94aa170c82caf94415629f
[104]: https://github.com/rust-lang/rust/pull/80801
[105]: https://github.com/dwrensha/fuzz-rustc
[106]: /Noratrieb
[107]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22C-bug%22
[108]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22I-ICE%22
[109]: https://github.com/rust-lang/rust/issues?q=state%3Aopen%20label%3A%22T-compiler%22
[110]: https://github.com/orgs/community/discussions/189141
[111]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[112]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[113]: https://github.com/security
[114]: https://www.githubstatus.com/
[115]: https://github.community/
[116]: https://docs.github.com/
[117]: https://support.github.com?tags=dotcom-footer
```
