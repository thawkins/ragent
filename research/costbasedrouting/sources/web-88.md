# Web source

- URL: https://github.com/irthomasthomas/undecidability/issues/626
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T15:44:15.678037079+00:00

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
[ irthomasthomas ][71] / ** [undecidability][72] ** Public
* [ Notifications ][73] You must be signed in to change notification settings
* [ Fork 2 ][74]
* [ Star 24 ][75]
* [ Code ][76]
* [ Issues 581 ][77]
* [ Pull requests 0 ][78]
* [ Actions ][79]
* [ Projects ][80]
* [ Security and quality 0 ][81]
* [ Insights ][82]
Additional navigation options
* [ Code ][83]
* [ Issues ][84]
* [ Pull requests ][85]
* [ Actions ][86]
* [ Projects ][87]
* [ Security and quality ][88]
* [ Insights ][89]

# Fast Classifiers for Prompt Routing #626

New issue
Copy link
New issue
Copy link
Open
Open
[Fast Classifiers for Prompt Routing][90]#626
Copy link
Labels
[AI-AgentsAutonomous AI agents using LLMs][91]Autonomous AI agents using LLMs[AlgorithmsSorting, Learning or
Classifying. All algorithms go here.][92]Sorting, Learning or Classifying. All algorithms go here.[MachineLearningML
Models, Training and Inference][93]ML Models, Training and Inference[New-LabelChoose this option if the existing labels
are insufficient to describe the content accurately][94]Choose this option if the existing labels are insufficient to
describe the content accurately[Researchpersonal research notes for a topic][95]personal research notes for a
topic[finetuningTools for finetuning of LLMs e.g. SFT or RLHF][96]Tools for finetuning of LLMs e.g. SFT or
RLHF[source-codeCode snippets][97]Code snippets

## Description

[[@irthomasthomas]][98]
[irthomasthomas][99]
opened [on Feb 27, 2024][100]
Issue body actions
* [classifiers/README.md at main · blockentropy/classifiers][101]

# classifiers/README.md

## Fast Classifiers for Prompt Routing

Routing and controlling the information flow is a core component in optimizing machine learning tasks. While some
architectures focus on internal routing of data within a model, we focus on the external routing of data between models.
This enables the combination of open source, proprietary, API based, and software based approaches to work together
behind a smart router. We investigate three different ways of externally routing the prompt - cosine similarity via
embeddings, zero-shot classification, and small classifiers.

## Implementation of Fast Classifiers

The `code-class.ipynb` Jupyter notebook walks through the process of creating a fast prompt classifier for smart
routing. For the fast classifiers, we utilize the model [DistilBERT][102], a smaller language representation model
designed for efficient on-the-edge operation and training under computational constraints. DistilBERT is not only less
costly to pre-train but also well-suited for on-device computations, as demonstrated through experiments and comparative
studies.

We quantize the model using [Optimum][103], enabling the model to run extremely fast on a CPU router. Each classifier
takes 5-8ms to run. An ensemble of 8 prompt classifiers takes about 50ms in total. Thus, each endpoint can route about
20 requests per second.

In the example `code-class`, we are deciding between prompts of code and not code prompts. The two datasets used are the
52K [instruction-following data][104] generated by GPT-4 with prompts in Alpaca. And the 20K instruction-following data
used for fine-tuning the [Code Alpaca][105] model.

Train test split of 80/20 yields an accuracy of 95.49% and f1 score of 0.9227.
[[Train Test]][106]

## Comparison vs other Routing methods

The most popular alternative to routing is via embedding similarity. For example, if one were to try to route a
programming question, one might set up the set of target classes as ["coding", "not coding"]. Each one of these strings
is then transformed into an embedding and compared against a prompt query like, "write a bubble sort in python". Given
the computed pair-wise cosine similarity between the query and class, we can then label the prompt as a coding question
and route the prompt to a coding-specific model. These do not scale well with larger numbers of embeddings. Nor are they
able to capture non-semantic type classes (like is the response likely to be more or less than 200 tokens). However,
they are adaptable and comparably fast and thus provide a good alternative to the trained fast classifiers.

[[Train Test]][107]

Quantifying different methods of routing in terms of execution time. As the prompt size increases, the query time also
increases as shown in (a). There is also a close to linear increase in the time as the number of classes increase as
shown in (b). However, the small classifiers do not increase in time as the class examples increase in the number of
tokens (c). This is due to the upfront cost of training the binary classifier, reducing cost at inference.

## Reproducibility

The `timing_tests.js` and `complexity.js` files can be used for reproducibility. Note that only the code classifier is
currently available in this repo. One will need to install the appropriate models from the [Transformers.js][108] repo.

[View on GitHub][109]

#### Suggested labels

#### {'label-name': 'Prompt-Routing', 'label-description': 'Focuses on external routing of data between models to
#### optimize machine learning tasks.', 'confidence': 50.24}

Reactions are currently unavailable

## Metadata

## Metadata

### Assignees

No one assigned

### Labels

[AI-AgentsAutonomous AI agents using LLMs][110]Autonomous AI agents using LLMs[AlgorithmsSorting, Learning or
Classifying. All algorithms go here.][111]Sorting, Learning or Classifying. All algorithms go here.[MachineLearningML
Models, Training and Inference][112]ML Models, Training and Inference[New-LabelChoose this option if the existing labels
are insufficient to describe the content accurately][113]Choose this option if the existing labels are insufficient to
describe the content accurately[Researchpersonal research notes for a topic][114]personal research notes for a
topic[finetuningTools for finetuning of LLMs e.g. SFT or RLHF][115]Tools for finetuning of LLMs e.g. SFT or
RLHF[source-codeCode snippets][116]Code snippets

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
* [Terms][117]
* [Privacy][118]
* [Security][119]
* [Status][120]
* [Community][121]
* [Docs][122]
* [Contact][123]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Firthomasthomas%2Fundecidability%2Fissues%2F626
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
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Firthomasthomas%2Fundecidability%2Fissues%2F626
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fvoltron%2Fissues
_fragments%2Fissue_layout&source=header-repo&source_repo=irthomasthomas%2Fundecidability
[68]: 
[69]: 
[70]: 
[71]: /irthomasthomas
[72]: /irthomasthomas/undecidability
[73]: /login?return_to=%2Firthomasthomas%2Fundecidability
[74]: /login?return_to=%2Firthomasthomas%2Fundecidability
[75]: /login?return_to=%2Firthomasthomas%2Fundecidability
[76]: /irthomasthomas/undecidability
[77]: /irthomasthomas/undecidability/issues
[78]: /irthomasthomas/undecidability/pulls
[79]: /irthomasthomas/undecidability/actions
[80]: /irthomasthomas/undecidability/projects
[81]: /irthomasthomas/undecidability/security
[82]: /irthomasthomas/undecidability/pulse
[83]: /irthomasthomas/undecidability
[84]: /irthomasthomas/undecidability/issues
[85]: /irthomasthomas/undecidability/pulls
[86]: /irthomasthomas/undecidability/actions
[87]: /irthomasthomas/undecidability/projects
[88]: /irthomasthomas/undecidability/security
[89]: /irthomasthomas/undecidability/pulse
[90]: #top
[91]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22AI-Agents%22
[92]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22Algorithms%22
[93]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22MachineLearning%22
[94]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22New-Label%22
[95]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22Research%22
[96]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22finetuning%22
[97]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22source-code%22
[98]: https://github.com/irthomasthomas
[99]: https://github.com/irthomasthomas
[100]: https://github.com/irthomasthomas/undecidability/issues/626#issue-2157361582
[101]: https://github.com/blockentropy/classifiers/blob/main/README.md?plain=1
[102]: https://huggingface.co/docs/transformers/en/model_doc/distilbert
[103]: https://huggingface.co/docs/optimum/index
[104]: https://arxiv.org/abs/2304.03277
[105]: https://github.com/sahil280114/codealpaca
[106]: ./traintest.png
[107]: ./graphs.png
[108]: https://huggingface.co/docs/transformers.js/en/index
[109]: https://github.com/blockentropy/classifiers/blob/main/README.md?plain=1
[110]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22AI-Agents%22
[111]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22Algorithms%22
[112]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22MachineLearning%22
[113]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22New-Label%22
[114]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22Research%22
[115]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22finetuning%22
[116]: https://github.com/irthomasthomas/undecidability/issues?q=state%3Aopen%20label%3A%22source-code%22
[117]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[118]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[119]: https://github.com/security
[120]: https://www.githubstatus.com/
[121]: https://github.community/
[122]: https://docs.github.com/
[123]: https://support.github.com?tags=dotcom-footer
```
