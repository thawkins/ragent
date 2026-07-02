# Web source

- URL: https://community.deeplearning.ai/t/i-m-trying-to-move-beyond-simple-ai-agents-what-makes-an-agentic-system-actually-useful/893024
- Title: [DeepLearning.AI][1]
- Captured (UTC): 2026-06-30T09:42:12.206586381+00:00

```text
[DeepLearning.AI][1]

# [I’m trying to move beyond simple AI agents — what makes an agentic system actually useful?][2]

[ AI Discussions ][3]
[ai-discussions][4]
[Shubham_Luxkar][5] June 22, 2026, 6:16pm 1

I have been experimenting with multi-agent systems and recently started building a DeepResearch-style agent workflow to
understand what goes into a real Agentic AI application.

The first version is intentionally simple, but the goal is not just another “LLM + tool call” demo. I’m trying to learn
the engineering patterns behind reliable agents.

Current workflow:
* Planner → breaks a complex goal into tasks
* Research agents → gather information and evidence
* Reflection → identify missing information or weak results
* Synthesis → combine findings into a report
* Critique/refinement → improve the final output

While building this, I realized the hard parts are not just calling an LLM. The interesting challenges seem to be:
* How should agents communicate?
* When should an agent decide to use tools?
* How do you evaluate whether an agent is actually improving?
* What is the right memory/state architecture?
* When do multiple agents help vs just add complexity?

I’m currently improving this project and would love feedback from people building agent systems:
* What architecture patterns have worked well for you?
* What features separate a demo agent from a useful agent?
* What mistakes should I avoid while scaling this?

I’m sharing the code in the comments for anyone interested in reviewing or experimenting with it.

Would appreciate any thoughts from this community.

GitHub:

[github.com][6]

### [GitHub - shubham4576/DeepResearch_CreateAgent: A multi-agent DeepResearch system that plans...][7]

A multi-agent DeepResearch system that plans complex queries, runs parallel web research, reflects on evidence quality,
synthesizes findings, critiques drafts, and produces citation-backed research reports.

I’m especially interested in feedback on:

* architecture decisions

* agent orchestration patterns

* evaluation methods

* ideas for making this closer to a production-style agent system

Open to suggestions and contributions.

### Related topics

─────────────────────────────────────────────────────────────────────────────────────────────────┬─┬────┬──────────┬────
Topic                                                                                            │ │Repl│Views     │Acti
                                                                                                 │ │ies │          │vity
─────────────────────────────────────────────────────────────────────────────────────────────────┼─┼────┼──────────┴────
[🌟 New Course! Enroll in Multi AI Agent Systems with crewAI][8]                                 │2│1093│May 20,   
[ News and Announcements ][9]                                                                    │ │    │2024      
[dl-ai-learning-platform][10]                                                                    │ │    │          
─────────────────────────────────────────────────────────────────────────────────────────────────┼─┼────┼──────────
[AI Multi-Agents][11]                                                                            │0│167 │April 23, 
[ AI Discussions ][12]                                                                           │ │    │2024      
[ai-discussions][13]                                                                             │ │    │          
─────────────────────────────────────────────────────────────────────────────────────────────────┼─┼────┼──────────
[Visualizing and Building Multi-Agent Systems + Free Interactive Sandbox                         │0│67  │June 7,   
https://agentswarms.fyi][14]                                                                     │ │    │2026      
[ AI Discussions ][15]                                                                           │ │    │          
[ai-discussions][16] ,  [project][17]                                                            │ │    │          
─────────────────────────────────────────────────────────────────────────────────────────────────┼─┼────┼──────────
[Agentic Design Patterns — open-source reference [feedback welcome]][18]                         │1│88  │May 17,   
[ AI Discussions ][19]                                                                           │ │    │2026      
[feedback][20] ,  [ai-discussions][21] ,  [introductions][22] ,  [project][23]                   │ │    │          
─────────────────────────────────────────────────────────────────────────────────────────────────┼─┼────┼──────────
[Sytem vs user prompt for different agents][24]                                                  │4│102 │December  
[ Agentic AI ][25]                                                                               │ │    │1, 2025   
[week-module-5][26] ,  [course][27]                                                              │ │    │          
─────────────────────────────────────────────────────────────────────────────────────────────────┴─┴────┴──────────
* [Home ][28]
* [Categories ][29]
* [Guidelines ][30]
* [Terms of Service ][31]
* [Privacy Policy ][32]

Powered by [Discourse][33], best viewed with JavaScript enabled

[1]: /
[2]: /t/i-m-trying-to-move-beyond-simple-ai-agents-what-makes-an-agentic-system-actually-useful/893024
[3]: /c/ai-discussions/408
[4]: https://community.deeplearning.ai/tag/ai-discussions
[5]: https://community.deeplearning.ai/u/Shubham_Luxkar
[6]: https://github.com/shubham4576/DeepResearch_CreateAgent
[7]: https://github.com/shubham4576/DeepResearch_CreateAgent
[8]: https://community.deeplearning.ai/t/new-course-enroll-in-multi-ai-agent-systems-with-crewai/628066
[9]: /c/news-and-announcements/24
[10]: https://community.deeplearning.ai/tag/dl-ai-learning-platform/179
[11]: https://community.deeplearning.ai/t/ai-multi-agents/615000
[12]: /c/ai-discussions/408
[13]: https://community.deeplearning.ai/tag/ai-discussions/19
[14]: https://community.deeplearning.ai/t/visualizing-and-building-multi-agent-systems-free-interactive-sandbox-https-ag
entswarms-fyi/892697
[15]: /c/ai-discussions/408
[16]: https://community.deeplearning.ai/tag/ai-discussions/19
[17]: https://community.deeplearning.ai/tag/project/126
[18]: https://community.deeplearning.ai/t/agentic-design-patterns-open-source-reference-feedback-welcome/892261
[19]: /c/ai-discussions/408
[20]: https://community.deeplearning.ai/tag/feedback/3
[21]: https://community.deeplearning.ai/tag/ai-discussions/19
[22]: https://community.deeplearning.ai/tag/introductions/56
[23]: https://community.deeplearning.ai/tag/project/126
[24]: https://community.deeplearning.ai/t/sytem-vs-user-prompt-for-different-agents/882075
[25]: /c/course-q-a/agentic-ai/567
[26]: https://community.deeplearning.ai/tag/week-module-5/158
[27]: https://community.deeplearning.ai/tag/course/182
[28]: /
[29]: /categories
[30]: /guidelines
[31]: https://community.deeplearning.ai/c/faq/terms-and-services/396
[32]: https://community.deeplearning.ai/t/about-the-privacy-category/522460
[33]: https://www.discourse.org
```
