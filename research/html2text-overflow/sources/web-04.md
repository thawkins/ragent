# Web source

- URL: https://github.com/rust-lang/cargo/issues/4460
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:21:03.754974566+00:00

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

[ rust-lang ][72] / ** [cargo][73] ** Public
* ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][74].
* [ Notifications ][75] You must be signed in to change notification settings
* [ Fork 3k ][76]
* [ Star 15.2k ][77]
* [ Code ][78]
* [ Issues 1.5k ][79]
* [ Pull requests 92 ][80]
* [ Actions ][81]
* [ Projects ][82]
* [ Wiki ][83]
* [ Security and quality 7 ][84]
* [ Insights ][85]
Additional navigation options
* [ Code ][86]
* [ Issues ][87]
* [ Pull requests ][88]
* [ Actions ][89]
* [ Projects ][90]
* [ Wiki ][91]
* [ Security and quality ][92]
* [ Insights ][93]

# Panic - Attempt to subtract with overflow #4460

New issue
Copy link
New issue
Copy link
Closed
Closed
[Panic - Attempt to subtract with overflow][94]#4460
Copy link
Labels
[A-cargo-apiArea: cargo-the-library API and internal code issues][95]Area: cargo-the-library API and internal code
issues[A-dependency-resolutionArea: dependency resolution and the resolver][96]Area: dependency resolution and the
resolver[C-bugCategory: bug][97]Category: bug

## Description

[[@rrichardson]][98]
[rrichardson][99]
opened [on Sep 1, 2017][100]
Issue body actions

at [https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/mod.rs#L453][101]

Running in release mode this just spins in a loop forever, since it doesn't panic on the arithmetic.

The issue arrises when I attempt to depend upon both blake2 and postgres.

*update 11:58*
Both crates depend on different versions of generic array and typenum, and it seems to break when trying to resolve the
dependencies.

*update 14:39"
Here is a minimal Cargo.toml that repros the problem :
[#4460 (comment)][102]

Running the latest nightly as of Sep 1 2017.

` ../../cargo/target/debug/cargo --version
cargo 0.23.0
`

### cargo stack trace

`rick@ubantoo:~/Projects/userhub$ RUST_BACKTRACE=1 ../../cargo/target/debug/cargo update
    Updating registry `https://github.com/rust-lang/crates.io-index`
    Updating git repository `https://github.com/pingcap/rust-prometheus`
    Updating git repository `https://github.com/QuiverMedia/rumqtt`
    Updating git repository `https://github.com/AlexPikalov/cdrs`
thread 'main' panicked at 'attempt to subtract with overflow', src/cargo/core/resolver/mod.rs:453:9
stack backtrace:
   0: std::sys::imp::backtrace::tracing::imp::unwind_backtrace
             at /checkout/src/libstd/sys/unix/backtrace/tracing/gcc_s.rs:49
   1: std::sys_common::backtrace::_print
             at /checkout/src/libstd/sys_common/backtrace.rs:71
   2: std::panicking::default_hook::{{closure}}
             at /checkout/src/libstd/sys_common/backtrace.rs:60
             at /checkout/src/libstd/panicking.rs:381
   3: std::panicking::default_hook
             at /checkout/src/libstd/panicking.rs:397
   4: std::panicking::rust_panic_with_hook
             at /checkout/src/libstd/panicking.rs:611
   5: std::panicking::begin_panic
             at /checkout/src/libstd/panicking.rs:572
   6: std::panicking::begin_panic_fmt
             at /checkout/src/libstd/panicking.rs:522
   7: rust_begin_unwind
             at /checkout/src/libstd/panicking.rs:498
   8: core::panicking::panic_fmt
             at /checkout/src/libcore/panicking.rs:71
   9: core::panicking::panic
             at /checkout/src/libcore/panicking.rs:51
  10: <cargo::core::resolver::RcVecIter<T>>::cur_index
             at src/cargo/core/resolver/mod.rs:453
  11: cargo::core::resolver::find_candidate
             at src/cargo/core/resolver/mod.rs:716
  12: cargo::core::resolver::activate_deps_loop
             at src/cargo/core/resolver/mod.rs:655
  13: cargo::core::resolver::resolve
             at src/cargo/core/resolver/mod.rs:355
  14: cargo::ops::resolve::resolve_with_previous
             at src/cargo/ops/resolve.rs:265
  15: cargo::ops::cargo_generate_lockfile::update_lockfile
             at src/cargo/ops/cargo_generate_lockfile.rs:77
  16: cargo::update::execute
             at src/bin/update.rs:81
  17: cargo::call_main_without_stdin
             at /home/rick/Projects/cargo/src/cargo/lib.rs:128
  18: cargo::try_execute_builtin_command
             at src/bin/cargo.rs:264
  19: cargo::execute
             at src/bin/cargo.rs:228
  20: cargo::call_main_without_stdin
             at /home/rick/Projects/cargo/src/cargo/lib.rs:128
  21: cargo::main::{{closure}}
             at src/bin/cargo.rs:95
  22: cargo::main
             at src/bin/cargo.rs:86
  23: __rust_maybe_catch_panic
             at /checkout/src/libpanic_unwind/lib.rs:99
  24: std::rt::lang_start
             at /checkout/src/libstd/panicking.rs:459
             at /checkout/src/libstd/panic.rs:361
             at /checkout/src/libstd/rt.rs:61
  25: main
  26: __libc_start_main
  27: _start
`

### Cargo.toml

`[dependencies]
blake2 = "^0.6"
chan = "^0.1"
clap = "2.26.0"
config = "0.7.0"
nix = "0.8.1"
num_cpus = "1.6.2"
quick-error = "1.2.0"
r2d2 = "0.7.2"
serde = "1.0.9"
serde_derive = "1.0.9"
slog = "2.0.9"
slog-json = "2.0.2"
threadpool = "^1.4"
time = "^0.1"
toml = "0.4"
chrono = "0.4"

[dependencies.cdrs]
git = "https://github.com/AlexPikalov/cdrs"
optional = true
rev = "1577cf7"

[dependencies.mqtt-protocol]
optional = true
version = "0.3"

[dependencies.prometheus]
git = "https://github.com/pingcap/rust-prometheus"
rev = "e4d7878"

[dependencies.proto]
path = "proto-rust"

[dependencies.protobuf]
optional = true
version = "^1.4"

[dependencies.r2d2_postgres]
optional = true
version = "0.13"

[dependencies.rumqtt]
optional = true
git = "https://github.com/QuiverMedia/rumqtt"

[dependencies.uuid]
features = ["v1"]
version = "^0.5.1"

[features]
cassandra = ["cdrs"]
default = [
    "cassandra",
    "postgres",
    "mqtt",
]
postgres = ["r2d2_postgres"]
mqtt = [
    "rumqtt",
    "mqtt-protocol",
    "protobuf",
]
[target."cfg(unix)".dependencies]
chan-signal = "^0.2"
`

Reactions are currently unavailable

## Metadata

## Metadata

### Assignees

No one assigned

### Labels

[A-cargo-apiArea: cargo-the-library API and internal code issues][103]Area: cargo-the-library API and internal code
issues[A-dependency-resolutionArea: dependency resolution and the resolver][104]Area: dependency resolution and the
resolver[C-bugCategory: bug][105]Category: bug

### Type

No type

### Fields

[Give feedback][106]
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
* [Terms][107]
* [Privacy][108]
* [Security][109]
* [Status][110]
* [Community][111]
* [Docs][112]
* [Contact][113]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Fcargo%2Fissues%2F4460
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
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Fcargo%2Fissues%2F4460
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fvoltron%2Fissues
_fragments%2Fissue_layout&source=header-repo&source_repo=rust-lang%2Fcargo
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /rust-lang
[73]: /rust-lang/cargo
[74]: 
[75]: /login?return_to=%2Frust-lang%2Fcargo
[76]: /login?return_to=%2Frust-lang%2Fcargo
[77]: /login?return_to=%2Frust-lang%2Fcargo
[78]: /rust-lang/cargo
[79]: /rust-lang/cargo/issues
[80]: /rust-lang/cargo/pulls
[81]: /rust-lang/cargo/actions
[82]: /rust-lang/cargo/projects
[83]: /rust-lang/cargo/wiki
[84]: /rust-lang/cargo/security
[85]: /rust-lang/cargo/pulse
[86]: /rust-lang/cargo
[87]: /rust-lang/cargo/issues
[88]: /rust-lang/cargo/pulls
[89]: /rust-lang/cargo/actions
[90]: /rust-lang/cargo/projects
[91]: /rust-lang/cargo/wiki
[92]: /rust-lang/cargo/security
[93]: /rust-lang/cargo/pulse
[94]: #top
[95]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22A-cargo-api%22
[96]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22A-dependency-resolution%22
[97]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22C-bug%22
[98]: https://github.com/rrichardson
[99]: https://github.com/rrichardson
[100]: https://github.com/rust-lang/cargo/issues/4460#issue-254698384
[101]: https://github.com/rust-lang/cargo/blob/master/src/cargo/core/resolver/mod.rs#L453
[102]: https://github.com/rust-lang/cargo/issues/4460#issuecomment-326689444
[103]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22A-cargo-api%22
[104]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22A-dependency-resolution%22
[105]: https://github.com/rust-lang/cargo/issues?q=state%3Aopen%20label%3A%22C-bug%22
[106]: https://github.com/orgs/community/discussions/189141
[107]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[108]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[109]: https://github.com/security
[110]: https://www.githubstatus.com/
[111]: https://github.community/
[112]: https://docs.github.com/
[113]: https://support.github.com?tags=dotcom-footer
```
