# Web source

- URL: https://blog.lnsresearch.com/autonomous-operations-ai-with-guardrails
- Title: [[LNSLogo for Web]][1]
- Captured (UTC): 2026-06-30T09:40:28.513756272+00:00

```text
[[LNSLogo for Web]][1]
* [HOW WE WORK ][2]
  * [
    **Our Process**
    
    Discover how we empower COOs & industrial leaders
    
    ][3]
  * [
    **Pricing**
    
    Simple menu-based pricing
    
    ][4]
  * [
    **Our Leaders**
    
    A highly collaborative team passionate about your growth
    
    ][5]
* [RESOURCES ][6]
  * [
    **Blog**
    
    Latest insights from our Analysts
    
    ][7]
  * [
    **Research**
    
    Data-driven IX research reports
    
    ][8]
  * [
    **Events**
    
    Connect and share with IX experts
    
    ][9]
* [The COO Council ][10]
* [PRODUCTIVITY ][11]
  * [
    **IPI**
    
    Industrial Productivity Index: 600 Companies Benchmarked
    
    ][12]
  * [
    **World's Most Productive Companies**
    
    Top 100 in 2026
    
    ][13]
  * [
    **Productivity Pathfinders**
    
    30 Market-Shaping Enterprises
    
    ][14]
  * [
    **Industries + Peer Groups**
    
    Productivity by Industry with Peer Group Detail
    
    ][15]
* [EVENTS ][16]
  * [
    **Executive Roundtables**
    
    Collaborate on today's industrial challenges
    
    ][17]
  * [
    **The Productivity Event**
    
    Benchmark the World's Most Productive Companies
    
    ][18]
  * [
    **The Transformation Event**
    
    Convene, engage, and drive the future of industry
    
    ][19]
[[Log in]][20] [[Join The COO Council]][21]

# Autonomous Operations: AI with Guardrails

[ Niels Andersen ][22]
Jun 25, 2025

## The Paradox of Autonomy

Can you trust an AI agent to make the right decision in your supply network?

AI agents are being actively investigated in industrial operations, from scheduling and forecasting to predictive
maintenance and autonomous quality checks. These systems promise unprecedented efficiency and responsiveness. But with
great autonomy comes great risk.

In a recent LNS Research Survey about Intelligent Supply Networks, 33% of the respondents said that they want AI agents
responsible for taking actions without human intervention, and 17% said they want agents who are responsible and
accountable for those actions. Only 4% stated that they desire to have no agents.

We see this pattern is even stronger when splitting between Intelligent Supply Network (ISN) Leaders and Followers.
Figure 1 illustrates how Leaders in this field are significantly more likely to prefer AI agents that are both
responsible and accurate —  almost three times more likely than Followers at 32% versus 13%.

[Which of the Following Best Describes theDesired Software Autonomous Operations Architecture]

##### Figure 1 - The Desire for AI Agents

The power of autonomous AI is also its Achilles’ heel: it acts independently. When designed without principles or
operated without boundaries, AI can lead to poor decisions, system failures, or even safety incidents. Autonomy without
accountability is a recipe for failure.

To unlock AI's benefits in manufacturing and supply chains, we must design agents with clear principles and guardrails.
Only then can we ensure that AI serves business goals safely and effectively.

## Understanding the Four Domains of Agentic AI Behavior

The scope of an AI agent’s behavior can be visualized across two dimensions: *safety* and *training*. This yields four
behavioral domains:
* * * Safe & Trained: The sweet spot. We are within the safe operating envelope, and the agent can confidently operate
      as we are within the scope of the training data. Closed-loop probabilistic agents can be used.
    * Safe & Untrained: A zone of untapped potential. We are within the safe operating envelope, so the risk is low, but
      the agent is not familiar with this domain. Closed-loop probabilistic agents are not recommended.
    * Unsafe & Trained: The most deceptive zone. We are outside the safe operating envelope, so we need to apply
      deterministic systems. Even though the agent has been trained in this domain, it is too risky to use a
      probabilistic system, as we cannot guarantee the outcome. We can record the agent’s recommendations, but not close
      the loop.
    * Unsafe & Untrained: The danger zone. Here, the AI is out of its depth in a high-stakes environment. Mistakes are
      not only likely but potentially catastrophic. Closed-loop probabilistic systems shall never be used.

[The Safe Autonomy Framework]

##### Figure 2 - The Safe Autonomy Framework

The takeaway: training data doesn’t define safety. AI systems need guardrails to understand when they are outside their
capabilities.

## Defining the Safe Operating Envelope

In industrial systems, the safe operating envelope is a “defined range of operating conditions within which a system or
process can be safely operated without causing damage or failure”. These are the conditions under which a probabilistic
system can function with acceptable risk. Guardrails must be implemented at the boundary of the safe operating envelope
so that deterministic systems can take over for the agent and ensure safe operations.

Generative AI and AI systems based on random sampling or inputs are inherently probabilistic, not deterministic. A
deterministic system is characterized by predictable behavior, where a specific input always leads to the same output.
In contrast, a probabilistic system incorporates randomness and provides a range of possible outcomes with associated
probabilities.

Understanding where probabilistic systems can be used is especially important in a distributed, collaborative
architecture with many interdependencies that are difficult to test and where autonomous systems must safely
interoperate.

## Embedding Principles and Rules into Autonomous Agents

AI agents, like intelligent business processes, must operate on principles rather than just rigid rules. These
principles are based on strategies and business objectives that consider the desired outcome for both the task and the
higher-level business.

For example, an AI agent optimizing operations must consider not just speed and cost but compliance and product
integrity.

However, when the boundary of the safe operating envelope is approached or breached, deterministic rules-based systems
must take over to ensure safe operations.

## From Uncertainty to Control: A Stepwise Approach to AI Agents

Understanding AI agent safety and performance begins with acknowledging the unknown. Here’s a stepwise model for
managing and classifying AI agent behavior:

────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
[Pic│**Start with All Possible States: **                                                                               
ture│                                                                                                                   
3-3]│Assume these states are unsafe until proven otherwise. This is a conservative approach that many forget.           
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
[Pic│**Define the Safe Operating Envelope: **                                                                           
ture│                                                                                                                   
4-1]│This is where the world is safe, even if everything does not go according to plan. It is OK to take risks inside   
    │this envelope.                                                                                                     
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
    │**Train the Model and Define the Scope of Training Data: **                                                        
    │                                                                                                                   
[Pic│AI models are trained, not programmed. They learn from the data that you provide. Remember, the model cannot       
ture│extrapolate; it is not intelligent, so don’t expect it to do something you have not trained it to do.              
5-2]│                                                                                                                   
────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────
    │**Classify the Regions of Operation:**                                                                             
    │                                                                                                                   
[Pic│You can now give the regions names and choose the operating strategy:                                              
ture│* * * Safe & Trained: Autonomous operation is allowed.                                                             
6]  │    * Safe & Untrained: Use deterministic control, supervised learning, or human-in-the-loop.                      
    │    * Unsafe & Trained: Apply guardrails and restrict autonomy.                                                    
    │    * Unsafe & Untrained: Prohibit operation entirely.                                                             
────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────

This framework reframes AI deployment as a risk-informed decision, not a leap of faith.

## The Intelligent Supply Network: An Architecture that Enables AI Agents

LNS has created the Intelligent Supply Network architecture as a recommendation for building intelligent systems inside
the plant and across the supply chain. In this architecture, AI agents sit in the Industrial Applications & Autonomous
Agents layer, which provides an abstraction between the physical assets and the standardized business system.

AI agents must interconnect with business systems (ERP, SCM, PLM), plant assets (equipment, sensors, actuators), human
operators (Connected Frontline Workforce), and subject matter experts in the Virtual Operations Center. The connection
is done through data platforms and infrastructure, enabling applications and AI agents to communicate with other
systems.

[Intelligent Supply Network Software]

##### Figure 3 - LNS Intelligent Supply Network

The Platforms & Infrastructure within the Intelligent Supply Network is enabling AI Agents through several capabilities:
* * * Standard interfaces such as MCP (Model Context Protocol), A2A (Agent to Agent), JSON REST, and HTML
    * Contextualization and metadata that give the data meaning
    * Guardrails: Role-based permissions, authentication, and authorization that control who can do what within each
      domain of safe operations

These capabilities ensure AI agents act not in isolation but as responsible members of a coordinated system.

## Rethinking Organizational Structure in the Age of AI Agents

As AI agents become embedded across operations, the nature of decision-making and management will change. Latency (the
time to make a good decision) is a productivity killer.

AI agents could become the new middle managers, trained to execute decisions, simulate scenarios, and recommend actions.
This shift has deep implications:
* * * Strategic leadership increases in importance as it defines the guiding principles.
    * Middle management may shrink or evolve, while the importance of the frontline workforce will increase.

New organization structures that focus more on the value streams and communication paths than the hierarchy must be
formed.

[From hierarchical to value-stream communication-driven organizations]

##### Figure 4 - From Hierarchical to Value-Stream Communication-Driven Organizations

AI agents provide the opportunity to move decisions closer to where the action is happening, which can shorten the
latency and allow for experimentation and learning.

## Why People Still Matter

Despite the promise of automation, people remain essential. Humans bring critical thinking, ethics, and adaptability.
The frontline workforce provides contextual intelligence. They can hear, feel, see, and smell things and identify
patterns that many AI systems miss.

Autonomy is not a substitute for human wisdom. It is a tool for augmenting it.

Safety First: Just as machines have emergency stop buttons, AI agents must be designed with the equivalent: a way for
humans to intervene immediately when outcomes veer off course. These agents must have an E-stop capability, not just
automation, but interruptible automation.

## Recommendations for COOs

The future of intelligent supply networks isn’t just smarter machines; it’s smarter systems and structures.

AI is showing tremendous promise, but it comes with significant challenges. Organizations that master the use of AI will
significantly outperform those that do not. This is not the time to put your head in the sand; we must lean forward and
identify the opportunities and pitfalls:
* * * Understand your business objectives and measure everything you do against them.
    * Take a risk-based approach: Understand and document your safe operating envelope.
    * Educate yourself about the opportunities and limitations of AI agents. Be realistic; don’t get fooled by one-off
      promising results.
    * Be clear on your operating principles and guardrails. Gather good training data. Create a decision hierarchy that
      ensures guardrails always have the highest priority.
    * Build an architecture that supports AI agents. If you have a spaghetti structure today, it will just become more
      chaotic once AI systems start interacting with it.
    * Build an operating model and organization that leverages AI. AI alone will not produce the results you want.
    * Remember, even with AI agents, you are still accountable for all actions and outcomes. Make sure that you have the
      right safeguards in place.

This is not just a technological shift - it’s an organizational one. Leadership, structure, and oversight must evolve
alongside autonomy.

## What to read next:
* * * [Why COOs Need an Intelligent Supply Network][23]
    * [Operational Excellence Driving Market Changes][24]
    * [Five Ways Industrial AI is Shaking Up Manufacturing (and Who’s Doing It)][25]
    * [Demystifying AI & ML in Industrial Analytics][26]
    * [What is an Industrial Operations Strategy, & How Can You Use it to Win?][27]

## Glossary:

────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────
**Term**    │**Meaning**                                                                                                
────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────
**AI agent**│An artificial intelligence application that can act on behalf of someone or another agent. AI agents can be
            │connected and have orchestrated behaviors.                                                                 
────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────
**Agent AI**│The capability of artificial intelligence to act on behalf of someone else.                                
────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────
**Determinis│A program or algorithm that, given the same inputs, will always produce the same output, with the same     
tic**       │sequence of intermediate states.                                                                           
────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────
**Probabilis│Based on or adapted to a theory of probability; subject to or involving chance variation.                  
tic**       │                                                                                                           
────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────
[[Knowledge Management]][28]

 

All entries in this Industrial Transformation blog represent the opinions of the authors based on their industry
experience and their view of the information collected using the methods described in our [Research Integrity][29]. All
product and company names are trademarks™ or registered® trademarks of their respective holders. Use of them does not
imply any affiliation with or endorsement by them.

[Autonomous Operations][30] [Virtual Operations Center][31] [Industrial AI][32] [Intelligent Supply Network][33]

#### **Subscribe Now**


#### **Become an LNS Research Member!**

As a member-level partner of LNS Research, you will receive our expert and proven Advisory Services. These exclusive
benefits give your team:
* Regular advisory sessions with our highly experienced LNS Research Analysts
* Access to the complete LNS Research Library
* Participation in members-only executive Roundtable events
* Important, continuous knowledge of Industrial Transformation (IX)

Let us help you with key decisions based on our solid research methodology and vast industrial experience. 

[ BOOK A STRATEGY CALL ][34]

#### Trending Now

[Siemens Acquires Camstar: Better Realizing Innovation for 3 Vertical Industries][35]

[What Is Industrial DataOps & Why Does Every Manufacturer Need It?][36]

[The Definitive Guide to Manufacturing Acronyms][37]

[Software-Defined Automation: Surprise... It's Not About Cost Savings][38]

[Manufacturing Execution Systems: Still Core, Now Smarter][39]

#### What Our Analysts Are Saying
* [Allison Kuhn (41)][40]
* [Andrew Hughes (77)][41]
* [Bob Francis (9)][42]
* [Chris Follis (4)][43]
* [Dan Jacob (86)][44]
* [Dan Miklovic (170)][45]
* [Diane Murray (30)][46]
* [Greg Goodwin (89)][47]
* [James Wells (43)][48]
* [Jason Kasper (31)][49]
* [Joe Perino (50)][50]
* [LNS Research (1)][51]
* [Mark Davidson (63)][52]
* [Matthew Littlefield (215)][53]
* [Mehul Shah (24)][54]
* [Mike Carroll (2)][55]
* [Niels Andersen (17)][56]
* [Patrick Fetterman (13)][57]
* [Peter S Bussey (71)][58]
* [| Research Team | (344)][59]
* [Tom Comstock (50)][60]
* [Vivek Murugesan (42)][61]

## Similar posts

Industrial Transformation / Digital Transformation

### [A Message to Leadership: Say Yes to Open, Interoperable Automation Systems][62]

In his latest blog, Principal Analyst Joe Perino, discusses why leadership should be saying yes to open, interoperable
automation systems.

Joe Perino Jun 9, 2021
Manufacturing Operations Management (MOM)

### [The Road to the Center of the Enterprise: Smart Manufacturing [Infographic]][63]

LNS Research provides this rapid-fire infographic for the road to Smart Manufacturing.

Andrew Hughes Dec 8, 2016
Supply Chain Management (SCM)

### [AWS: Boldly Going Where No Cloud Hyperscaler Has Gone Before][64]

Cut through the hype with LNS Research Analyst Bob Francis as he covers the key takeaways from AWS re:Invent 2022 and
'the voyages of the Starship...

Bob Francis Jan 30, 2023

#### 
#### SUBSCRIBE TO THE LNS RESEARCH BLOG

## Stay on top of the latest industrial transformation insights from our expert analysts

The Industrial Transformation and Operational Excellence Blog is an informal environment for our analysts to share
thoughts and insights on a range of technology and business topics.

[ [LNSLogo for Web] ][65]

1 Broadway
Cambridge, MA 02142

[LNS AI Logo_LNS Blue White]

 
* [ ][66]
* [ ][67]
* [ ][68]
* [ ][69]

### Members
* [Login][70]
* [Research Library][71]

### Quick Links
* [How We Work][72]
* [Schedule A Call][73]
* [Pricing][74]
* [The IX Event][75]

### Resources
* [Blog][76]
* [Research][77]
* [Events][78]

### Company
* [About][79]
* [Contact][80]
* [Careers][81]
* [News & Press][82]
* [Integrity & Privacy Policy][83]

© 2026 LNS Research

[1]: https://www.lnsresearch.com/
[2]: 
[3]: https://www.lnsresearch.com/how-we-work-0
[4]: https://www.lnsresearch.com/pricing
[5]: https://www.lnsresearch.com/our-leaders
[6]: 
[7]: https://blog.lnsresearch.com/
[8]: https://www.lnsresearch.com/industrial-transformation-research
[9]: https://www.lnsresearch.com/events
[10]: https://www.lnsresearch.com/-the-coo-council
[11]: https://www.lnsresearch.com/industrial-productivity-index
[12]: https://www.lnsresearch.com/industrial-productivity-index
[13]: https://www.lnsresearch.com/worlds-most-productive-companies
[14]: https://www.lnsresearch.com/pathfinders
[15]: https://www.lnsresearch.com/wmpc-industries
[16]: https://www.lnsresearch.com/events
[17]: https://www.lnsresearch.com/events
[18]: https://www.theproductivityevent.com
[19]: https://www.thetransformationevent.com
[20]: https://cta-redirect.hubspot.com/cta/redirect/136847/4a3e443f-13d3-4c38-95db-543425ca1025
[21]: https://cta-redirect.hubspot.com/cta/redirect/136847/167535e4-8146-4c2a-b972-1f84d51f6192
[22]: https://blog.lnsresearch.com/author/niels-andersen
[23]: https://blog.lnsresearch.com/why-coos-need-an-intelligent-supply-network
[24]: https://blog.lnsresearch.com/operational-excellence-driving-market-changes
[25]: https://blog.lnsresearch.com/five-ways-industrial-ai-is-shaking-up-manufacturing-and-whos-doing-it
[26]: https://blog.lnsresearch.com/demystifying-ai-ml-in-industrial-analytics
[27]: https://blog.lnsresearch.com/what-is-an-industrial-operations-strategy-how-do-you-use-it-to-win
[28]: https://cta-redirect.hubspot.com/cta/redirect/136847/1a9ee704-2fa3-44d5-a65a-5bfb166ee8b7
[29]: https://www.lnsresearch.com/privacy-and-cookie-policy
[30]: https://blog.lnsresearch.com/topic/autonomous-operations
[31]: https://blog.lnsresearch.com/topic/virtual-operations-center
[32]: https://blog.lnsresearch.com/topic/industrial-ai
[33]: https://blog.lnsresearch.com/topic/intelligent-supply-network
[34]: https://136847-hs-sites-com.sandbox.hs-sites.com/strategy-call
[35]: https://blog.lnsresearch.com/blog/bid/202779/siemens-acquires-camstar-better-realizing-innovation-for-3-vertical-i
ndustries
[36]: https://blog.lnsresearch.com/what-is-industrial-dataops-why-does-every-manufacturer-need-it
[37]: https://blog.lnsresearch.com/acronym-quick-reference
[38]: https://blog.lnsresearch.com/software-defined-automation-surprise-it-is-not-about-cost-savings
[39]: https://blog.lnsresearch.com/manufacuring-execution-systems-still-core-now-smarter
[40]: https://blog.lnsresearch.com/author/allison-kuhn
[41]: https://blog.lnsresearch.com/author/andrew-hughes
[42]: https://blog.lnsresearch.com/author/bob-francis
[43]: https://blog.lnsresearch.com/author/chris-follis
[44]: https://blog.lnsresearch.com/author/dan-jacob
[45]: https://blog.lnsresearch.com/author/dan-miklovic
[46]: https://blog.lnsresearch.com/author/diane-murray
[47]: https://blog.lnsresearch.com/author/greg-goodwin
[48]: https://blog.lnsresearch.com/author/james-wells
[49]: https://blog.lnsresearch.com/author/jason-kasper
[50]: https://blog.lnsresearch.com/author/joe-perino
[51]: https://blog.lnsresearch.com/author/lns-research
[52]: https://blog.lnsresearch.com/author/mark-davidson
[53]: https://blog.lnsresearch.com/author/matthew-littlefield
[54]: https://blog.lnsresearch.com/author/mehul-shah
[55]: https://blog.lnsresearch.com/author/mike-carroll
[56]: https://blog.lnsresearch.com/author/niels-andersen
[57]: https://blog.lnsresearch.com/author/patrick-fetterman
[58]: https://blog.lnsresearch.com/author/peter-s-bussey
[59]: https://blog.lnsresearch.com/author/research-team
[60]: https://blog.lnsresearch.com/author/tom-comstock
[61]: https://blog.lnsresearch.com/author/vivek-murugesan
[62]: https://blog.lnsresearch.com/a-message-to-leadership-say-yes-to-open-interoperable-automation-systems
[63]: https://blog.lnsresearch.com/the-road-to-smart-manufacturing-infographic
[64]: https://blog.lnsresearch.com/aws-boldly-going-where-no-cloud-hyperscaler-has-gone-before
[65]: https://www.lnsresearch.com/
[66]: https://www.linkedin.com/company/lns-research/
[67]: https://twitter.com/lnsresearch
[68]: https://www.youtube.com/@lnsresearch
[69]: https://www.facebook.com/profile.php?id=100057364603399
[70]: https://www.lnsresearch.com/privacy-and-cookie-policy/login
[71]: https://members.lnsresearch.com/research-library
[72]: https://www.lnsresearch.com/how-we-work
[73]: https://www.lnsresearch.com/strategy-call
[74]: https://www.lnsresearch.com/pricing
[75]: https://www.theixevent.com/
[76]: https://blog.lnsresearch.com/?__hstc=233546881.e3f4c654137a198e55f66c8e80241c66.1655912946072.1656016516679.165609
3985205.6&__hssc=233546881.114.1656093985205&__hsfp=2908620477
[77]: https://136847-hs-sites-com.sandbox.hs-sites.com/industrial-transformation-research
[78]: https://www.lnsresearch.com/events
[79]: https://www.lnsresearch.com/about
[80]: https://www.lnsresearch.com/contact
[81]: https://lnsresearch.bamboohr.com/careers
[82]: https://www.lnsresearch.com/press
[83]: https://www.lnsresearch.com/privacy-and-cookie-policy
```
