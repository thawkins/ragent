# Web source

- URL: https://github.com/linebender/vello/issues/788
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:21:12.174829073+00:00

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

[ linebender ][72] / ** [vello][73] ** Public
* [ Notifications ][74] You must be signed in to change notification settings
* [ Fork 262 ][75]
* [ Star 4.1k ][76]
* [ Code ][77]
* [ Issues 121 ][78]
* [ Pull requests 69 ][79]
* [ Actions ][80]
* [ Security and quality 0 ][81]
* [ Insights ][82]
Additional navigation options
* [ Code ][83]
* [ Issues ][84]
* [ Pull requests ][85]
* [ Actions ][86]
* [ Security and quality ][87]
* [ Insights ][88]

# `attempt to subtract with overflow` in vello-encoding when drawing MANY images, resulting in potential OOB access #788

New issue
Copy link
New issue
Copy link
Open
Open
[`attempt to subtract with overflow` in vello-encoding when drawing MANY images, resulting in potential OOB
access][89]#788
Copy link

## Description

[[@BloodStainedCrow]][90]
[BloodStainedCrow][91]
opened [on Jan 14, 2025][92]
Issue body actions

Vello Version: 0.3.0
Rust Version: 1.86.0-nightly (2025-01-11)

Adapting the example in `examples/simple` with

`fn add_shapes_to_scene(scene: &mut Scene) {
    let blob = Blob::new(Arc::new(vec![
        200;
        vello::peniko::Format::Rgba8
            .size_in_bytes(100, 100)
            .unwrap()
    ]));

    let img = Image::new(blob, vello::peniko::Format::Rgba8, 100, 100);

    for i in 0..1_000_000 {
        scene.draw_image(&img, Affine::IDENTITY);
    }
}
`

results in a overflow panic in debug mode:

Details

`thread 'main' panicked at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello_encoding-0.3.0/src/confi
g.rs:186:31:
attempt to subtract with overflow
stack backtrace:
   0: rust_begin_unwind
             at /rustc/eb54a50837ad4bcc9842924f27e7287ca66e294c/library/std/src/panicking.rs:692:5
   1: core::panicking::panic_fmt
             at /rustc/eb54a50837ad4bcc9842924f27e7287ca66e294c/library/core/src/panicking.rs:75:14
   2: core::panicking::panic_const::panic_const_sub_overflow
             at /rustc/eb54a50837ad4bcc9842924f27e7287ca66e294c/library/core/src/panicking.rs:178:21
   3: vello_encoding::config::RenderConfig::new
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello_encoding-0.3.0/src/config.rs:186:31
   4: vello::render::Render::render_encoding_coarse
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello-0.3.0/src/render.rs:160:13
   5: vello::render::render_encoding_full
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello-0.3.0/src/render.rs:100:25
   6: vello::render::render_full
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello-0.3.0/src/render.rs:85:5
   7: vello::Renderer::render_to_texture
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello-0.3.0/src/lib.rs:423:13
   8: vello::Renderer::render_to_surface
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vello-0.3.0/src/lib.rs:467:9
   9: <factory::rendering::SimpleVelloApp as winit::application::ApplicationHandler>::window_event
             at ./src/rendering/mod.rs:141:17
  10: winit::event_loop::dispatch_event_for_app
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/event_loop.rs:642:52
  11: winit::event_loop::EventLoop<T>::run_app::{{closure}}
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/event_loop.rs:265:49
  12: core::ops::function::impls::<impl core::ops::function::FnMut<A> for &mut F>::call_mut
             
at /nix/store/cl98jzwbz3rblf4sx7cif8l5qmax2kzy-rust-default-1.86.0-nightly-2025-01-12/lib/rustlib/src/rust/library/core/
src/ops/function.rs:294:13
  13: winit::platform_impl::linux::wayland::event_loop::EventLoop<T>::single_iteration
             
at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/platform_impl/linux/wayland/event_loo
p/mod.rs:469:17
  14: winit::platform_impl::linux::wayland::event_loop::EventLoop<T>::pump_events
             
at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/platform_impl/linux/wayland/event_loo
p/mod.rs:211:13
  15: winit::platform_impl::linux::wayland::event_loop::EventLoop<T>::run_on_demand
             
at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/platform_impl/linux/wayland/event_loo
p/mod.rs:181:19
  16: winit::platform_impl::linux::EventLoop<T>::run_on_demand
             
at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/platform_impl/linux/mod.rs:819:56
  17: winit::platform_impl::linux::EventLoop<T>::run
             
at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/platform_impl/linux/mod.rs:812:9
  18: winit::event_loop::EventLoop<T>::run_app
             at /home/tim/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/winit-0.30.8/src/event_loop.rs:265:9
`

The underflow happens while calculating `binning_size: buffer_sizes.bin_data.len() - layout.bin_data_start`.

In Release mode the program does not panic (since overflow checks are disabled), and the images do not appear.

If I understand this correctly, this value is used as the size to some buffer, it under-/overflowing will result in
potential OOB accesses on the gpu, which would explain why nothing gets rendered.

While I doubt drawing `1_000_000` images in this way is a typical usecase, it seems to me like something fishy is
happening here.

Reactions are currently unavailable

## Metadata

## Metadata

### Assignees

No one assigned

### Labels

No labels
No labels

### Type

No type

### Fields

[Give feedback][93]
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
* [Terms][94]
* [Privacy][95]
* [Security][96]
* [Status][97]
* [Community][98]
* [Docs][99]
* [Contact][100]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Flinebender%2Fvello%2Fissues%2F788
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
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Flinebender%2Fvello%2Fissues%2F788
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fvoltron%2Fissues
_fragments%2Fissue_layout&source=header-repo&source_repo=linebender%2Fvello
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /linebender
[73]: /linebender/vello
[74]: /login?return_to=%2Flinebender%2Fvello
[75]: /login?return_to=%2Flinebender%2Fvello
[76]: /login?return_to=%2Flinebender%2Fvello
[77]: /linebender/vello
[78]: /linebender/vello/issues
[79]: /linebender/vello/pulls
[80]: /linebender/vello/actions
[81]: /linebender/vello/security
[82]: /linebender/vello/pulse
[83]: /linebender/vello
[84]: /linebender/vello/issues
[85]: /linebender/vello/pulls
[86]: /linebender/vello/actions
[87]: /linebender/vello/security
[88]: /linebender/vello/pulse
[89]: #top
[90]: https://github.com/BloodStainedCrow
[91]: https://github.com/BloodStainedCrow
[92]: https://github.com/linebender/vello/issues/788#issue-2788076390
[93]: https://github.com/orgs/community/discussions/189141
[94]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[95]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[96]: https://github.com/security
[97]: https://www.githubstatus.com/
[98]: https://github.community/
[99]: https://docs.github.com/
[100]: https://support.github.com?tags=dotcom-footer
```
