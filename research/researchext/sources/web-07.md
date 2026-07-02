# Web source

- URL: https://dev.to/kaustav_chowdhury_f3cdc47/from-wrappers-to-reasoners-building-an-iterative-research-agent-3j7l
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-30T09:39:09.437555466+00:00

```text
[Skip to content][1]
[Navigation menu] [ [DEV Community] ][2]
[Search] [ Powered by Algolia [Search] ][3]
[ Log in ][4] [ Create account ][5]

## DEV Community

[Close]
Add reaction
Like Unicorn Exploding Head Raised Hands Fire
Jump to Comments Save Boost
[More...]
Copy link [Copy link]
Copied to Clipboard
[ Share to X ][6] [ Share to LinkedIn ][7] [ Share to Facebook ][8] [ Share to Mastodon ][9]
[Report Abuse][10]
[[Kaustav Chowdhury]][11]
[Kaustav Chowdhury][12]

Posted on Dec 4, 2025

# From Wrappers to Reasoners: Building an Iterative Research Agent

[#googleaichallenge][13] [#ai][14] [#agents][15] [#devchallenge][16]

Submission for the AI Agents Intensive Course Writing Challenge

Introduction

I'll be honest—before taking the 5-Day AI Agents Intensive Course with Google and Kaggle, I thought I understood agents.
Spoiler: I didn't.

My mental model was basically wrapping an LLM in a while(true) loop, adding a few if-statements, and calling it
"agentic." Embarrassing in hindsight, but we all start somewhere, right?

The past five days have completely rewired that thinking. I've gone from building simple, reactive bots to trying to
architect "deliberative" systems that actually plan (and sometimes overthink) their next move.

Key Learnings & Insights
1. Reactive vs. Deliberative: The Wake-Up Call

Honestly? The distinction between Reactive and Deliberative agents hit me like a truck. I'd been building 'agents' for
months—turns out most were just glorified function callers.

Reactive Agents: Stimulus -> Response. Simple, but dumb.

Deliberative Agents: They maintain state. They reason. They hold a grudge (okay, not really, but they remember context).

Realizing that an agent isn't just a fancy API wrapper but a reasoning engine changed everything. It shifted my focus
from obsessing over prompt syntax to actually designing a system architecture.
1. Memory is Harder Than It Looks

Memory management was where I got stuck the longest. I kept thinking, "just pass the whole conversation history, context
windows are huge now!"

Day 3 taught me that's expensive and, frankly, lazy. Distinguishing between short-term memory (current task) and
episodic memory (remembering how we solved this last time) is the difference between a toy and a tool.

The Capstone Project: Iterative Research & Refinement Agent

For my capstone, I wanted to fix a pet peeve: simple research agents that just grab the first Google result and call it
a day. That's not research; that's confirmation bias.

I built the Iterative Research & Refinement Agent (IRRA).

The Architecture

Day 4's LangGraph session was where the state management architecture finally clicked for me—that's what made the
Critic's feedback loop possible.

Instead of a straight line, I used a Self-Correction Loop.

graph TD
User[User Query] --> Planner
Planner -->|Sub-questions| Researcher
Researcher -->|Findings| Critic
Critic -->|Approved| Writer
Critic -->|Rejected + Feedback| Planner
Writer --> FinalReport

The system has three "personas" arguing with each other:

The Planner: Breaks the query down.

The Researcher: Googles stuff.

The Critic: The novelty. It reviews findings before the final write-up.

How It Works (The "Trust But Verify" Loop)

If the Critic hates the findings, it kicks them back.

# The logic that kept me up at night

while iteration < max_retries:
findings = researcher.search(query)
critique = critic.review(findings)

`if critique.status == "APPROVED":
    final_report = writer.compile(findings)
    break
else:
    # "Do better." - The Critic
    print(f"Critique received: {critique.feedback}")
    query = planner.refine_query(original_query, critique.feedback)
    iteration += 1
`

[Enter fullscreen mode] [Exit fullscreen mode]

The "Aha!" Moment

I asked it to research "The benefits of coffee."

Round 1: It found a bunch of generic lifestyle blog fluff.

Critic: "These sources are weak. No clinical studies, and we haven't even mentioned anxiety or sleep disruption."

Round 2 (Automatic): The Planner pivoted to "Clinical studies coffee anxiety insomnia correlation."

I didn't explicitly ask for a balanced report—the agent just... decided a good report needed counter-arguments? That was
weird and incredibly cool.

Does it actually work?
In my tests with 20 research queries, IRRA cited an average of 5.3 sources (vs. 2.1 for baseline agents) and caught
contradictions in 65% of cases where a simple RAG pipeline would have missed them.

The Failure (Or: How I Accidentally Created a Hater)

Let me tell you about the time my agent gaslit itself into an infinite loop.

My first version of the Critic was way too picky. It rejected a perfectly good CDC source because "the writing style
seemed informal." I watched it loop 47 times over 23 minutes, burning through $2.47 in API calls, rejecting everything
until it hit the recursion limit and crashed.

I spent an entire evening adding guardrails like if rejection_count > 3: lower_standards(). Not my proudest code, but it
stopped the infinite loops of perfectionism. (Yes, I named the function that. No, I won't change it.)

What I'd Do Differently

If I rebuilt this tomorrow, I'd implement a "confidence score" for the Critic instead of a binary approve/reject system.
A nuanced score (e.g., "60% confidence - requires minor verification") would reduce those expensive loops while
maintaining quality.

Conclusion

This course broke my brain in the best way. I walked in an API consumer and walked out an architect (albeit a junior
one).

My next steps:

Fix the 14 TODOs I left in the notebook.

Give the agent persistent memory so it stops forgetting valid sources.

Figure out if the Critic needs its own Critic, or if that's just madness.

Huge thanks to the Google and Kaggle team for designing this course. If anyone wants to roast my code or suggest
improvements, I'm [@kaustav_chowdhury_f3cdc47][17] . I can take it. Probably.

## Top comments (0)

Subscribe
[pic]
Personal Trusted User
[ Create template ][18]

Templates let you quickly answer FAQs or store snippets for re-use.

Submit Preview [Dismiss][19]
[Code of Conduct][20] • [Report abuse][21]

Are you sure you want to hide this comment? It will become hidden in your post, but will still be visible via the
comment's [permalink][22].

Hide child comments as well

Confirm

For further actions, you may consider blocking this person and/or [reporting abuse][23]

[ Kaustav Chowdhury ][24]
Follow
* Joined
  Dec 4, 2025

### More from [Kaustav Chowdhury][25]

[ Beyond the Hype: Why Google Cloud's "Shift Down" Agent Security Changes Everything
#devchallenge #cloudnextchallenge #googlecloud
][26]

💎 DEV Diamond Sponsors

Thank you to our Diamond Sponsors for supporting the DEV Community

[ [Google AI - Official AI Model and Platform Partner] ][27]

Google AI is the official AI Model and Platform Partner of DEV

[ [Neon - Official Database Partner] ][28]

Neon is the official database partner of DEV

[ [Algolia - Official Search Partner] ][29]

Algolia is the official search partner of DEV

[DEV Community][30] — A space to discuss and keep up software development and manage your software career
* [ Home ][31]
* [ DEV Challenges ][32]
* [ DEV++ ][33]
* [ Videos ][34]
* [ DEV Education Tracks ][35]
* [ DEV Help ][36]
* [ Advertise on DEV ][37]
* [ Organization Accounts ][38]
* [ DEV Showcase ][39]
* [ About ][40]
* [ Contact ][41]
* [ Free Postgres Database ][42]
* [ DEV Shop ][43]
* [ MLH ][44]
* [ Code of Conduct ][45]
* [ Privacy Policy ][46]
* [ Terms of Use ][47]

Built on [Forem][48] — the [open source][49] software that powers [DEV][50] and other inclusive communities.

Made with love and [Ruby on Rails][51]. DEV Community © 2016 - 2026.

[DEV Community]

We're a place where coders share, stay up-to-date and grow their careers.

[ Log in ][52] [ Create account ][53]

[1]: #main-content
[2]: /
[3]: https://www.algolia.com/developers/?utm_source=devto&utm_medium=referral
[4]: https://dev.to/enter?signup_subforem=1
[5]: https://dev.to/enter?signup_subforem=1&state=new-user
[6]: https://twitter.com/intent/tweet?text=%22From%20Wrappers%20to%20Reasoners%3A%20Building%20an%20Iterative%20Research
%20Agent%22%20by%20Kaustav%20Chowdhury%20%23DEVCommunity%20https%3A%2F%2Fdev.to%2Fkaustav_chowdhury_f3cdc47%2Ffrom-wrapp
ers-to-reasoners-building-an-iterative-research-agent-3j7l
[7]: https://www.linkedin.com/shareArticle?mini=true&url=https%3A%2F%2Fdev.to%2Fkaustav_chowdhury_f3cdc47%2Ffrom-wrapper
s-to-reasoners-building-an-iterative-research-agent-3j7l&title=From%20Wrappers%20to%20Reasoners%3A%20Building%20an%20Ite
rative%20Research%20Agent&summary=Submission%20for%20the%20AI%20Agents%20Intensive%20Course%20Writing%20Challenge%20%20I
ntroduction%20%20I%27ll%20be%20honest%E2%80%94before...&source=DEV%20Community
[8]: https://www.facebook.com/sharer.php?u=https%3A%2F%2Fdev.to%2Fkaustav_chowdhury_f3cdc47%2Ffrom-wrappers-to-reasoners
-building-an-iterative-research-agent-3j7l
[9]: https://s2f.kytta.dev/?text=https%3A%2F%2Fdev.to%2Fkaustav_chowdhury_f3cdc47%2Ffrom-wrappers-to-reasoners-building-
an-iterative-research-agent-3j7l
[10]: /report-abuse
[11]: /kaustav_chowdhury_f3cdc47
[12]: /kaustav_chowdhury_f3cdc47
[13]: /t/googleaichallenge
[14]: /t/ai
[15]: /t/agents
[16]: /t/devchallenge
[17]: https://dev.to/kaustav_chowdhury_f3cdc47
[18]: /settings/response-templates
[19]: /404.html
[20]: /code-of-conduct
[21]: /report-abuse
[22]: #
[23]: /report-abuse
[24]: /kaustav_chowdhury_f3cdc47
[25]: /kaustav_chowdhury_f3cdc47
[26]: /kaustav_chowdhury_f3cdc47/beyond-the-hype-why-google-clouds-shift-down-agent-security-changes-everything-n28
[27]: https://aistudio.google.com/?utm_source=partner&utm_medium=partner&utm_campaign=FY25-Global-DEVpartnership-sponsor
ship-AIS&utm_content=-&utm_term=-&bb=146443
[28]: https://neon.tech/?ref=devto&bb=146443
[29]: https://www.algolia.com/developers/?utm_source=devto&utm_medium=referral&bb=146443
[30]: /
[31]: /
[32]: /challenges
[33]: /++
[34]: /videos
[35]: /deved
[36]: /help
[37]: /advertise
[38]: /organizations
[39]: /showcase
[40]: /about
[41]: /contact
[42]: /free-postgres-database-tier
[43]: https://shop.forem.com/
[44]: https://mlh.io/
[45]: /code-of-conduct
[46]: /privacy
[47]: /terms
[48]: https://www.forem.com
[49]: https://dev.to/t/opensource
[50]: https://dev.to
[51]: https://dev.to/t/rails
[52]: https://dev.to/enter?signup_subforem=1
[53]: https://dev.to/enter?signup_subforem=1&state=new-user
```
