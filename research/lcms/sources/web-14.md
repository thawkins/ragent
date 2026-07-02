# Web source

- URL: https://www.linkedin.com/pulse/large-concept-models-language-modeling-sentence-space-vlad-bogolin-wdige
- Title: Agree & Join LinkedIn
- Captured (UTC): 2026-06-29T16:29:48.458973783+00:00

```text
Agree & Join LinkedIn

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][1], [Privacy Policy][2], and [Cookie
Policy][3].

`` `` `` `` `` `` ``

## Sign in to view more content

Create your free account or sign in to continue your search

`` `` `` `` `` `` `` `` `` ``
Email or phone
Password
Show
[Forgot password?][4] Sign in
Sign in with Email

or

New to LinkedIn? [Join now][5]

By clicking Continue to join or sign in, you agree to LinkedIn’s [User Agreement][6], [Privacy Policy][7], and [Cookie
Policy][8].

`` `` `` `` `` `` `` [ Skip to main content ][9] [ LinkedIn ][10]
* [ Top Content ][11]
* [ People ][12]
* [ Learning ][13]
* [ Jobs ][14]
* [ Games ][15]
[ Join now ][16] [ Sign in ][17]
`` `` `` ``
[Large Concept Models: Language Modeling in a Sentence Representation Space]

# Large Concept Models: Language Modeling in a Sentence Representation Space
* [ Report this article ][18]

[ Vlad Bogolin ][19]

### Vlad Bogolin

Published Dec 24, 2024
[ + Follow ][20]

Today's paper introduces a novel approach to language modeling that operates at a higher level of abstraction than
traditional token-based models. Instead of processing text word by word, it works with "concepts" - sentence-level
semantic representations that are language and modality independent. This approach, called Large Concept Model (LCM),
aims to better mimic how humans process and generate information by working with higher-level ideas rather than
individual words.

## Method Overview

The Large Concept Model operates by first converting input text into "concepts" using a system called SONAR, which
transforms sentences into fixed-size embeddings that capture their meaning independently of the specific language used.
These concept embeddings serve as the basic units that the model works with, rather than individual words or tokens.



The model then processes these concepts using one of several architectures, with the main variants being diffusion-based
approaches. These approaches gradually refine noisy concept representations into meaningful ones, guided by the context.
This is similar to how an artist might start with a rough sketch and gradually refine it into a detailed drawing.

A key feature of the system is its modular design - the concept encoders and decoders (SONAR) are separate from the main
model, allowing for easy extension to new languages or modalities. The model supports up to 200 languages for text and
76 languages for speech input, significantly more than other current language models.

The paper explores several architectural variants, including Base-LCM, One-Tower, Two-Tower, and Quantized versions,
each offering different approaches to generating concept embeddings. Through extensive experimentation, the
diffusion-based approaches (One-Tower and Two-Tower) emerged as the most effective.

## Recommended by LinkedIn

[
Rethinking Hallucination Detection in Language Models:…
Sarvex Jatasra 1 year ago
][21]
[
Understanding Large Language Models and Their…
Phaneendra Ganji 1 year ago
][22]
[
What are Large Language Models (LLMs)? How do they…
Asim Hafeez 1 year ago
][23]

## Results

The key results are:

* The model shows strong performance across multiple languages without requiring language-specific training
* It achieves better handling of long-form content due to working with sentence-level concepts rather than individual
  tokens
* The approach shows particular strength in maintaining coherence across longer outputs
* The computational efficiency scales better with context length compared to traditional language models
* The 7B parameter version of the model shows competitive performance with similarly sized traditional language models

## Conclusion

The paper presents a promising new direction in language modeling by operating at a concept level rather than token
level. This approach offers advantages in multilingual capability, scalability, and coherent long-form generation. For
more information please consult the [full paper.][24]

Congrats to the authors for their work!

Barrault, Loïc, et al. "Large Concept Models: Language Modeling in a Sentence Representation Space." Meta AI, 2024.

[ AI Paper of the Day ][25]

### AI Paper of the Day

#### 1,694 follower

[ + Subscribe ][26]
`` `` `` `` ``
``
[
Like
][27]
[ Comment ][28]
`` ``
* Copy
* LinkedIn
* Facebook
* X
Share
`` ``
[ 7 ][29] `` `` `` `` `` `` ``

To view or add a comment, [sign in][30]

## More articles by this author

No more previous content
* [
  
  ### SAIL-VL2 Technical Report
  
  #### Sep 20, 2025
  
  ][31]
* [
  
  ### LongEmotion: Measuring Emotional Intelligence of Large Language Models in Long-Context Interaction
  
  #### Sep 19, 2025
  
  ][32]
* [
  
  ### Stress Testing Deliberative Alignment for Anti-Scheming Training
  
  #### Sep 18, 2025
  
  ][33]
* [
  
  ### CyberSOCEval: Benchmarking LLMs Capabilities for Malware Analysis and Threat Intelligence Reasoning
  
  #### Sep 17, 2025
  
  ][34]
* [
  
  ### UI-S1: Advancing GUI Automation via Semi-online Reinforcement Learning
  
  #### Sep 16, 2025
  
  ][35]
* [
  
  ### Mini-o3: Scaling Up Reasoning Patterns and Interaction Turns for Visual Search
  
  #### Sep 15, 2025
  
  ][36]
* [
  
  ### CDE: Curiosity-Driven Exploration for Efficient Reinforcement Learning in Large Language Models
  
  #### Sep 14, 2025
  
  ][37]
* [
  
  ### HuMo: Human-Centric Video Generation via Collaborative Multi-Modal Conditioning
  
  #### Sep 13, 2025
  
  ][38]
* [
  
  ### VLA-Adapter: An Effective Paradigm for Tiny-Scale Vision-Language-Action Model
  
  #### Sep 12, 2025
  
  ][39]
* [
  
  ### The Majority is not always right: RL training for solution aggregation
  
  #### Sep 11, 2025
  
  ][40]
No more next content
[ See all ][41]
``

## Others also viewed
* [
  
  ### Language Modeling and Word Representation
  
  Mehdi Jafari 3y
  ][42]
* [
  
  ### A Guide to Training Your Own Language Model
  
  CrossML Pvt Ltd 2y
  ][43]
* [
  
  ### Decoding the Language Revolution: A Comprehensive Guide to Large Language Models
  
  Wisecube 3y
  ][44]
* [
  
  ### Evaluating RAG Systems: A Comprehensive Approach to Assessing Retrieval and Generation Performance
  
  Snigdha Kakkar 2y
  ][45]
* [
  
  ### Paper Review: Ferret-v2: An Improved Baseline for Referring and Grounding with Large Language Models
  
  Andrey Lukyanenko 2y
  ][46]
* [
  
  ### BERT-Bidirectional Encoder Representations from Transformers
  
  Shradha Agarwal 2y
  ][47]
* [
  
  ### Exploring LangChain's Expression Language (LCEL)
  
  Rany ElHousieny, PhDᴬᴮᴰ 1y
  ][48]
* [
  
  ### The Road to Agency: How Large Language Models Work
  
  Adam Darmanin 2mo
  ][49]
* [
  
  ### From Cross Entropy to GRPO: A Journey
  
  SARAT BHARGAVA CHINNI 1y
  ][50]
* [
  
  ### The Architecture of Linguistic Discretisation: A Comparative Analysis of Tokenisation Strategies in Large Language
  ### Models
  
  Dr. Partha Majumdar 2mo
  ][51]

Show more Show less

## Similar topics
* [
  
  ### How Large Language Models Create Conceptual Coherence
  
  5 Posts
  2,199
  `` `` `` `` `` `` ``
  ][52]
* [
  
  ### How Large Language Models Represent Concepts and Behaviors
  
  10 Posts
  2,028
  `` `` `` `` `` `` ``
  ][53]
* [
  
  ### How Large Language Models Process Contextual Information
  
  10 Posts
  1,818
  `` `` `` `` `` `` ``
  ][54]
* [
  
  ### How Language Models Transform Information Discovery
  
  7 Posts
  715
  `` `` `` `` `` `` ``
  ][55]
* [
  
  ### Innovations in Language Modeling Techniques
  
  10 Posts
  1,911
  `` `` `` `` `` `` ``
  ][56]
* [
  
  ### Key Findings from Large Language Model Analysis
  
  9 Posts
  2,166
  `` `` `` `` `` `` ``
  ][57]
* [
  
  ### How Large Language Models Process Big Data Sets
  
  6 Posts
  992
  `` `` `` `` `` `` ``
  ][58]
* [
  
  ### How Large Language Models Reshape Data Patterns
  
  5 Posts
  802
  `` `` `` `` `` `` ``
  ][59]
* [
  
  ### How Large Language Models Solve Problems Without Introspection
  
  10 Posts
  1,545
  `` `` `` `` `` `` ``
  ][60]
* [
  
  ### Using Multi-Dimensional Context in Large Language Models
  
  10 Posts
  1,060
  `` `` `` `` `` `` ``
  ][61]

Show more Show less

## Explore content categories
* [Career][62]
* [Productivity][63]
* [Finance][64]
* [Soft Skills & Emotional Intelligence][65]
* [Project Management][66]
* [Education][67]
* [Technology][68]
* [Leadership][69]
* [Ecommerce][70]
* [User Experience][71]
* [Recruitment & HR][72]
* [Customer Experience][73]
* [Real Estate][74]
* [Marketing][75]
* [Sales][76]
* [Retail & Merchandising][77]
* [Science][78]
* [Supply Chain Management][79]
* [Future Of Work][80]
* [Consulting][81]
* [Writing][82]
* [Economics][83]
* [Artificial Intelligence][84]
* [Employee Experience][85]
* [Workplace Trends][86]
* [Fundraising][87]
* [Networking][88]
* [Corporate Social Responsibility][89]
* [Negotiation][90]
* [Communication][91]
* [Engineering][92]
* [Hospitality & Tourism][93]
* [Business Strategy][94]
* [Change Management][95]
* [Organizational Culture][96]
* [Design][97]
* [Innovation][98]
* [Event Planning][99]
* [Training & Development][100]

Show more Show less
* LinkedIn © 2026
* [ About ][101]
* [ Accessibility ][102]
* [ User Agreement ][103]
* [ Privacy Policy ][104]
* [ Cookie Policy ][105]
* [ Copyright Policy ][106]
* [ Brand Policy ][107]
* [ Guest Controls ][108]
* [ Community Guidelines ][109]
* * العربية (Arabic)
  * বাংলা (Bangla)
  * Čeština (Czech)
  * Dansk (Danish)
  * Deutsch (German)
  * Ελληνικά (Greek)
  * **English (English)**
  * Español (Spanish)
  * فارسی (Persian)
  * Suomi (Finnish)
  * Français (French)
  * हिंदी (Hindi)
  * Magyar (Hungarian)
  * Bahasa Indonesia (Indonesian)
  * Italiano (Italian)
  * עברית (Hebrew)
  * 日本語 (Japanese)
  * 한국어 (Korean)
  * मराठी (Marathi)
  * Bahasa Malaysia (Malay)
  * Nederlands (Dutch)
  * Norsk (Norwegian)
  * ਪੰਜਾਬੀ (Punjabi)
  * Polski (Polish)
  * Português (Portuguese)
  * Română (Romanian)
  * Русский (Russian)
  * Svenska (Swedish)
  * తెలుగు (Telugu)
  * ภาษาไทย (Thai)
  * Tagalog (Tagalog)
  * Türkçe (Turkish)
  * Українська (Ukrainian)
  * Tiếng Việt (Vietnamese)
  * 简体中文 (Chinese (Simplified))
  * 正體中文 (Chinese (Traditional))
  Language

[1]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[2]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[3]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
[4]: https://www.linkedin.com/uas/request-password-reset?trk=csm-v2_forgot_password
[5]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-senten
ce-space-vlad-bogolin-wdige&trk=pulse-article_contextual-sign-in-modal_join-link
[6]: /legal/user-agreement?trk=linkedin-tc_auth-button_user-agreement
[7]: /legal/privacy-policy?trk=linkedin-tc_auth-button_privacy-policy
[8]: /legal/cookie-policy?trk=linkedin-tc_auth-button_cookie-policy
[9]: #main-content
[10]: /?trk=article-ssr-frontend-pulse_nav-header-logo
[11]: https://www.linkedin.com/top-content?trk=article-ssr-frontend-pulse_guest_nav_menu_topContent
[12]: https://www.linkedin.com/pub/dir/+/+?trk=article-ssr-frontend-pulse_guest_nav_menu_people
[13]: https://www.linkedin.com/learning/search?trk=article-ssr-frontend-pulse_guest_nav_menu_learning
[14]: https://www.linkedin.com/jobs/search?trk=article-ssr-frontend-pulse_guest_nav_menu_jobs
[15]: https://www.linkedin.com/games?trk=article-ssr-frontend-pulse_guest_nav_menu_games
[16]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_nav-header-join
[17]: https://www.linkedin.com/uas/login?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sentence-spa
ce-vlad-bogolin-wdige&fromSignIn=true&trk=article-ssr-frontend-pulse_nav-header-signin
[18]: /uas/login?session_redirect=https%3A%2F%2Fwww.linkedin.com%2Fpulse%2Flarge-concept-models-language-modeling-senten
ce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_ellipsis-menu-semaphore-sign-in-redirect&guestReportContentTy
pe=PONCHO_ARTICLE&_f=guest-reporting
[19]: https://uk.linkedin.com/in/vladbogo
[20]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_publisher-author-card
[21]: https://www.linkedin.com/pulse/rethinking-hallucination-detection-language-models-we-sarvex-jatasra-5s6jc
[22]: https://www.linkedin.com/pulse/understanding-large-language-models-retrieval-capabilities-g-hmqwe
[23]: https://www.linkedin.com/pulse/what-large-language-models-llms-how-do-work-asim-hafeez-15rlf
[24]: https://www.linkedin.com/redir/redirect?url=https%3A%2F%2Fai%2Emeta%2Ecom%2Fresearch%2Fpublications%2Flarge-concep
t-models-language-modeling-in-a-sentence-representation-space%2F&urlhash=sEod&trk=article-ssr-frontend-pulse_little-text
-block
[25]: https://www.linkedin.com/newsletters/ai-paper-of-the-day-7158959468787978240
[26]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige
[27]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_x-social-details_like-toggle_like-cta
[28]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_comment-cta
[29]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_x-social-details_likes-count_social-actions-reactions
[30]: https://www.linkedin.com/signup/cold-join?session_redirect=%2Fpulse%2Flarge-concept-models-language-modeling-sente
nce-space-vlad-bogolin-wdige&trk=article-ssr-frontend-pulse_x-social-details_feed-cta-banner-cta
[31]: https://www.linkedin.com/pulse/sail-vl2-technical-report-vlad-bogolin-wxawe?trk=article-ssr-frontend-pulse_more-ar
ticles_related-content-card
[32]: https://www.linkedin.com/pulse/longemotion-measuring-emotional-intelligence-large-language-bogolin-pm9ue?trk=artic
le-ssr-frontend-pulse_more-articles_related-content-card
[33]: https://www.linkedin.com/pulse/stress-testing-deliberative-alignment-anti-scheming-training-bogolin-7geqe?trk=arti
cle-ssr-frontend-pulse_more-articles_related-content-card
[34]: https://www.linkedin.com/pulse/cybersoceval-benchmarking-llms-capabilities-malware-analysis-bogolin-vqwme?trk=arti
cle-ssr-frontend-pulse_more-articles_related-content-card
[35]: https://www.linkedin.com/pulse/ui-s1-advancing-gui-automation-via-semi-online-learning-vlad-bogolin-9mlte?trk=arti
cle-ssr-frontend-pulse_more-articles_related-content-card
[36]: https://www.linkedin.com/pulse/mini-o3-scaling-up-reasoning-patterns-interaction-turns-vlad-bogolin-tuuge?trk=arti
cle-ssr-frontend-pulse_more-articles_related-content-card
[37]: https://www.linkedin.com/pulse/cde-curiosity-driven-exploration-efficient-learning-large-bogolin-r0xbe?trk=article
-ssr-frontend-pulse_more-articles_related-content-card
[38]: https://www.linkedin.com/pulse/humo-human-centric-video-generation-via-collaborative-vlad-bogolin-tjibe?trk=articl
e-ssr-frontend-pulse_more-articles_related-content-card
[39]: https://www.linkedin.com/pulse/vla-adapter-effective-paradigm-tiny-scale-model-vlad-bogolin-817ue?trk=article-ssr-
frontend-pulse_more-articles_related-content-card
[40]: https://www.linkedin.com/pulse/majority-always-right-rl-training-solution-vlad-bogolin-nhlpc?trk=article-ssr-front
end-pulse_more-articles_related-content-card
[41]: https://www.linkedin.com/today/author/vladbogo?trk=article-ssr-frontend-pulse_more-articles
[42]: https://www.linkedin.com/pulse/language-modeling-word-representation-mohammad-mehdi-jafari
[43]: https://www.linkedin.com/pulse/guide-training-your-own-language-model-crossml-f1kje
[44]: https://www.linkedin.com/pulse/decoding-language-revolution-comprehensive-guide-large-models
[45]: https://www.linkedin.com/pulse/evaluating-rag-systems-comprehensive-approach-assessing-kakkar-esm9c
[46]: https://www.linkedin.com/pulse/paper-review-ferret-v2-improved-baseline-referring-large-lukyanenko-tuuef
[47]: https://www.linkedin.com/pulse/bert-bidirectional-encoder-representations-from-shradha-agarwal-xnelc
[48]: https://www.linkedin.com/pulse/exploring-langchains-expression-language-lcel-rany-elhousieny-phd%E1%B4%AC%E1%B4%AE
%E1%B4%B0-5evkc
[49]: https://www.linkedin.com/pulse/road-agency-how-large-language-models-work-adam-darmanin-pnqbe
[50]: https://www.linkedin.com/pulse/from-cross-entropy-grpo-journey-sarat-bhargava-chinni-gyigc
[51]: https://www.linkedin.com/pulse/architecture-linguistic-discretisation-comparative-large-majumdar-ylaff
[52]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-create-conceptual
-coherence/
[53]: https://www.linkedin.com/top-content/artificial-intelligence/understanding-ai-systems/how-large-language-models-re
present-concepts-and-behaviors/
[54]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-process-contextua
l-information/
[55]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/how-language-models-tr
ansform-information-discovery/
[56]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/innovations-in-languag
e-modeling-techniques/
[57]: https://www.linkedin.com/top-content/artificial-intelligence/large-language-models-insights/key-findings-from-larg
e-language-model-analysis/
[58]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-process-big-data-
sets/
[59]: https://www.linkedin.com/top-content/technology/ai-language-processing/how-large-language-models-reshape-data-patt
erns/
[60]: https://www.linkedin.com/top-content/artificial-intelligence/understanding-ai-systems/how-large-language-models-so
lve-problems-without-introspection/
[61]: https://www.linkedin.com/top-content/technology/ai-language-processing/using-multi-dimensional-context-in-large-la
nguage-models/
[62]: https://www.linkedin.com/top-content/career/
[63]: https://www.linkedin.com/top-content/productivity/
[64]: https://www.linkedin.com/top-content/finance/
[65]: https://www.linkedin.com/top-content/soft-skills-emotional-intelligence/
[66]: https://www.linkedin.com/top-content/project-management/
[67]: https://www.linkedin.com/top-content/education/
[68]: https://www.linkedin.com/top-content/technology/
[69]: https://www.linkedin.com/top-content/leadership/
[70]: https://www.linkedin.com/top-content/ecommerce/
[71]: https://www.linkedin.com/top-content/user-experience/
[72]: https://www.linkedin.com/top-content/recruitment-hr/
[73]: https://www.linkedin.com/top-content/customer-experience/
[74]: https://www.linkedin.com/top-content/real-estate/
[75]: https://www.linkedin.com/top-content/marketing/
[76]: https://www.linkedin.com/top-content/sales/
[77]: https://www.linkedin.com/top-content/retail-merchandising/
[78]: https://www.linkedin.com/top-content/science/
[79]: https://www.linkedin.com/top-content/supply-chain-management/
[80]: https://www.linkedin.com/top-content/future-of-work/
[81]: https://www.linkedin.com/top-content/consulting/
[82]: https://www.linkedin.com/top-content/writing/
[83]: https://www.linkedin.com/top-content/economics/
[84]: https://www.linkedin.com/top-content/artificial-intelligence/
[85]: https://www.linkedin.com/top-content/employee-experience/
[86]: https://www.linkedin.com/top-content/workplace-trends/
[87]: https://www.linkedin.com/top-content/fundraising/
[88]: https://www.linkedin.com/top-content/networking/
[89]: https://www.linkedin.com/top-content/corporate-social-responsibility/
[90]: https://www.linkedin.com/top-content/negotiation/
[91]: https://www.linkedin.com/top-content/communication/
[92]: https://www.linkedin.com/top-content/engineering/
[93]: https://www.linkedin.com/top-content/hospitality-tourism/
[94]: https://www.linkedin.com/top-content/business-strategy/
[95]: https://www.linkedin.com/top-content/change-management/
[96]: https://www.linkedin.com/top-content/organizational-culture/
[97]: https://www.linkedin.com/top-content/design/
[98]: https://www.linkedin.com/top-content/innovation/
[99]: https://www.linkedin.com/top-content/event-planning/
[100]: https://www.linkedin.com/top-content/training-development/
[101]: https://about.linkedin.com?trk=d_flagship2_pulse_read_footer-about
[102]: https://www.linkedin.com/accessibility?trk=d_flagship2_pulse_read_footer-accessibility
[103]: https://www.linkedin.com/legal/user-agreement?trk=d_flagship2_pulse_read_footer-user-agreement
[104]: https://www.linkedin.com/legal/privacy-policy?trk=d_flagship2_pulse_read_footer-privacy-policy
[105]: https://www.linkedin.com/legal/cookie-policy?trk=d_flagship2_pulse_read_footer-cookie-policy
[106]: https://www.linkedin.com/legal/copyright-policy?trk=d_flagship2_pulse_read_footer-copyright-policy
[107]: https://brand.linkedin.com/policies?trk=d_flagship2_pulse_read_footer-brand-policy
[108]: https://www.linkedin.com/psettings/guest-controls?trk=d_flagship2_pulse_read_footer-guest-controls
[109]: https://www.linkedin.com/legal/professional-community-policies?trk=d_flagship2_pulse_read_footer-community-guide
```
