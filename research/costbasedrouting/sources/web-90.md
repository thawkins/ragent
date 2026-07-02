# Web source

- URL: https://vllm-semantic-router.com/docs/tutorials/signal/learned/complexity
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T15:44:17.729586055+00:00

```text
[Skip to main content][1]
[
[vLLM Semantic Router Logo][vLLM Semantic Router Logo]
**vLLM-SR**][2][Docs][3]
[Research][4]
* [Paper & Talks][5]
* [White Paper][6]
* [Vision Paper][7]
[Blog][8]
[Community][9]
* [Governance][10]
* [Working Group][11]
* [Contributing Guide][12]
* [Code of Conduct][13]
* [GitHub Issues][14]
[Leaderboard][15]
[English][16]
* [English][17]
* [简体中文][18]
[Latest][19]
* [Latest][20]
* [v0.3][21]
* [v0.2][22]
* [v0.1][23]
[GitHub][24][Models][25]
* [vLLM Semantic Router][26]
* [Overview][27]
* [Installation][28]
* [Capacities][29]
  * [Signals][30]
    * [Signal][31]
    * [Heuristic][32]
    * [Learned][33]
      * [Complexity Signal][34]
      * [Domain Signal][35]
      * [Embedding Signal][36]
      * [Embedding Anchor Design Principles][37]
      * [Modality Signal][38]
      * [Fact Check Signal][39]
      * [Jailbreak Signal][40]
      * [PII Signal][41]
      * [Preference Signal][42]
      * [Reask Signal][43]
      * [Knowledge Base Signal][44]
      * [User Feedback Signal][45]
  * [Projections][46]
  * [Decisions][47]
  * [Algorithms][48]
  * [Learning][49]
  * [Plugins][50]
  * [Global][51]
* [Fleet Simulator][52]
* [Proposals][53]
* [Model Training][54]
* [API Reference][55]
* [Troubleshooting][56]
* [Contributing][57]
Documentation

# Complexity Signal

Overview

[Docs index][58][Edit page][59]
* Capacities
* Signals
* Learned
* Complexity Signal
Version: Latest
On this page

# Complexity Signal

## Overview[][60]

`complexity` estimates whether a prompt needs a harder reasoning path or a cheaper easy path. It maps to
`config/signal/complexity/` and is declared under `routing.signals.complexity`.

This family is learned: the classifier compares requests against hard and easy examples using embedding similarity, and
can optionally use multimodal candidates.

## Key Advantages[][61]
* Separates reasoning escalation from domain classification.
* Reuses one complexity policy across multiple decisions.
* Supports hard/easy examples that are easy to tune over time.
* Lets simple prompts stay on cheaper models while hard prompts escalate.

## What Problem Does It Solve?[][62]

Topic alone does not tell you whether a prompt needs strong reasoning. Two questions in the same domain can have very
different reasoning depth.

`complexity` solves that by estimating task difficulty directly from example-driven signal rules.

## When to Use[][63]

Use `complexity` when:
* some prompts need stronger reasoning or longer chains of thought
* easy traffic should stay on cheaper models
* you want escalation policies that are independent of domain
* multimodal reasoning requests need different handling from simple prompts

## Configuration[][64]

Source fragment family: `config/signal/complexity/`

`global:
  model_catalog:
    modules:
      complexity:
        prototype_scoring:
          enabled: true
          cluster_similarity_threshold: 0.9
          max_prototypes: 8
          best_weight: 0.75
          top_m: 2
          margin_threshold: 0.0
routing:
  signals:
    complexity:
      - name: needs_reasoning
        threshold: 0.75
        description: Escalate multi-step reasoning or synthesis-heavy prompts.
        hard:
          candidates:
            - solve this step by step
            - compare multiple tradeoffs
            - analyze the root cause
        easy:
          candidates:
            - answer briefly
            - quick summary
            - simple rewrite
`

Use `complexity` with representative hard and easy examples so the learned boundary matches your real routing cost
profile. `prototype_scoring` is a family-level config under `global.model_catalog.modules.complexity`, so every
complexity rule shares the same prototype-bank construction and label-scoring policy. Each rule still builds separate
hard and easy prototype banks before computing the hard-vs-easy margin.

[Edit this page][65]
[
Previous
Structure Signal
][66][
Next
Domain Signal
][67]
* [Overview][68]
* [Key Advantages][69]
* [What Problem Does It Solve?][70]
* [When to Use][71]
* [Configuration][72]
Documentation
* [Quick Start][73]
* [Installation][74]
* [Governance][75]
* [Contributing][76]
Community
* [GitHub][77]
* [Hugging Face][78]
* [GitHub Discussions][79]
* [Leaderboard][80]
More
* [Blog][81]
* [Publications][82]
* [White Paper][83]
* [Vision Paper][84]
* [License][85]
Copyright © 2026 vLLM Semantic Router Team. Built with Docusaurus.

[1]: #__docusaurus_skipToContent_fallback
[2]: /
[3]: /docs/intro
[4]: #
[5]: /publications
[6]: /white-paper
[7]: /vision-paper
[8]: /blog
[9]: #
[10]: /community/team
[11]: /community/work-groups
[12]: /community/contributing
[13]: /community/code-of-conduct
[14]: https://github.com/vllm-project/semantic-router/issues
[15]: /community/contributors
[16]: #
[17]: /docs/tutorials/signal/learned/complexity
[18]: /zh-Hans/docs/tutorials/signal/learned/complexity
[19]: /docs/tutorials/signal/learned/complexity
[20]: /docs/tutorials/signal/learned/complexity
[21]: /docs/v0.3/tutorials/signal/learned/complexity
[22]: /docs/v0.2/intro
[23]: /docs/v0.1/intro
[24]: https://github.com/vllm-project/semantic-router
[25]: https://huggingface.co/LLM-Semantic-Router
[26]: /docs/intro
[27]: /docs/overview/goals
[28]: /docs/installation/
[29]: /docs/tutorials/signal/overview
[30]: /docs/tutorials/signal/overview
[31]: /docs/tutorials/signal/overview
[32]: /docs/tutorials/signal/heuristic/authz
[33]: /docs/tutorials/signal/learned/complexity
[34]: /docs/tutorials/signal/learned/complexity
[35]: /docs/tutorials/signal/learned/domain
[36]: /docs/tutorials/signal/learned/embedding
[37]: /docs/tutorials/signal/learned/embedding-design-principles
[38]: /docs/tutorials/signal/learned/modality
[39]: /docs/tutorials/signal/learned/fact-check
[40]: /docs/tutorials/signal/learned/jailbreak
[41]: /docs/tutorials/signal/learned/pii
[42]: /docs/tutorials/signal/learned/preference
[43]: /docs/tutorials/signal/learned/reask
[44]: /docs/tutorials/signal/learned/kb
[45]: /docs/tutorials/signal/learned/user-feedback
[46]: /docs/tutorials/projection/overview
[47]: /docs/tutorials/decision/overview
[48]: /docs/tutorials/algorithm/overview
[49]: /docs/tutorials/learning/overview
[50]: /docs/tutorials/plugin/overview
[51]: /docs/tutorials/global/overview
[52]: /docs/fleet-sim/overview
[53]: /docs/proposals/unified-config-contract-v0-3
[54]: /docs/training/training-overview
[55]: /docs/api/router
[56]: /docs/troubleshooting/network-tips
[57]: /docs/community/overview
[58]: /docs/intro
[59]: https://github.com/vllm-project/semantic-router/edit/main/website/docs/tutorials/signal/learned/complexity.md
[60]: #overview
[61]: #key-advantages
[62]: #what-problem-does-it-solve
[63]: #when-to-use
[64]: #configuration
[65]: https://github.com/vllm-project/semantic-router/edit/main/website/docs/tutorials/signal/learned/complexity.md
[66]: /docs/tutorials/signal/heuristic/structure
[67]: /docs/tutorials/signal/learned/domain
[68]: #overview
[69]: #key-advantages
[70]: #what-problem-does-it-solve
[71]: #when-to-use
[72]: #configuration
[73]: /docs/intro
[74]: /docs/installation
[75]: /community/team
[76]: /community/contributing
[77]: https://github.com/vllm-project/semantic-router
[78]: https://huggingface.co/LLM-Semantic-Router
[79]: https://github.com/vllm-project/semantic-router/discussions
[80]: /community/contributors
[81]: /blog
[82]: /publications
[83]: /white-paper
[84]: /vision-paper
[85]: https://github.com/vllm-project/semantic-router/blob/main/LICENSE
```
