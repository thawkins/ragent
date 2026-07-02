# Web source

- URL: https://medium.com/@mehrdad-/iterative-rag-explained-methods-and-practical-considerations-fbf194fae991
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-30T09:38:58.693259334+00:00

```text
[Sitemap][1]
[Open in app][2]

Sign up

[Sign in][3]

Get app
[
Write
][4]
[
Search
][5]

Sign up

[Sign in][6]

[Unknown user]

Member-only story

# Iterative RAG: Methods and Practical Considerations

[
[Mehrdad]
][7]
[Mehrdad][8]
7 min read
·
Aug 7, 2024
[
][9]

--

[
][10]
[][11]
[

Listen

][12]

Share

Iterative RAG increases the accuracy of retrieving the most relevant content from the knowledge base (and as a result
the accuracy of response generation), especially for applications that require planning, reasoning, etc.

I can categorize the iterative RAG methods into 4 base approaches
* Adding a self-feedback module in RAG
* Adding a planning step to the retrieval process
* Actively generating new queries/ questions using the previous iterations
* Generating multiple drafts first, then verifying them for the final response

Here, we are looking at these approaches with some examples from the literature. However, using iterative RAG can come
with an increase in the latency and the cost. Some considerations for using the iterative RAG in practical use cases
have been mentioned in the last section.

## Iterative RAG Methods

### Method — Adding a self-feedback module in RAG

The main idea is to add a module to evaluate the retrieved documents before generating the final response. Here we are
looking into 2 algorithms in this direction:

[**Iterative Self-Feedback (RA-ISF)**][13]** **The algorithm considers 3 self-knowledge, passage relevance, and question
decomposition modules:
* First, a LLM is used to determine if the response to the query is known or unknown…

[
][14]

--

[
][15]

--

[
][16]
[][17]
[
[Mehrdad]
][18]
[
[Mehrdad]
][19]
[

## Written by Mehrdad

][20]
[11 followers][21]
·[33 following][22]
[

Help

][23]
[

Status

][24]
[

About

][25]
[

Careers

][26]
[

Press

][27]
[

Blog

][28]
[

Store

][29]
[

Privacy

][30]
[

Rules

][31]
[

Terms

][32]
[

Text to speech

][33]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-pra
ctical-considerations-fbf194fae991&source=post_page---top_nav_layout_nav-----------------------global_nav---------------
---
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-pra
ctical-considerations-fbf194fae991&source=post_page---top_nav_layout_nav-----------------------global_nav---------------
---
[7]: /@mehrdad-?source=post_page---byline--fbf194fae991---------------------------------------
[8]: /@mehrdad-?source=post_page---byline--fbf194fae991---------------------------------------
[9]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Ffbf194fae991&operation=register&redirect=https%3A%2F%
2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&user=Mehrdad&user
Id=bb650c75854&source=---header_actions--fbf194fae991---------------------clap_footer------------------
[10]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Ffbf194fae991&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&user=Mehrdad&u
serId=bb650c75854&source=---header_actions--fbf194fae991---------------------repost_header------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Ffbf194fae991&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&source=---he
ader_actions--fbf194fae991---------------------bookmark_footer------------------
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3Dfbf194fae991&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerat
ions-fbf194fae991&source=---header_actions--fbf194fae991---------------------post_audio_button------------------
[13]: https://arxiv.org/pdf/2403.06840
[14]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Ffbf194fae991&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&user=Mehrdad&use
rId=bb650c75854&source=---footer_actions--fbf194fae991---------------------clap_footer------------------
[15]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2Ffbf194fae991&operation=register&redirect=https%3A%2F
%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&user=Mehrdad&use
rId=bb650c75854&source=---footer_actions--fbf194fae991---------------------clap_footer------------------
[16]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Ffbf194fae991&operation=register&redirect=https%3A%
2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&user=Mehrdad&u
serId=bb650c75854&source=---footer_actions--fbf194fae991---------------------repost_footer------------------
[17]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Ffbf194fae991&operation=register&redirect=https%3
A%2F%2Fmedium.com%2F%40mehrdad-%2Fiterative-rag-explained-methods-and-practical-considerations-fbf194fae991&source=---fo
oter_actions--fbf194fae991---------------------bookmark_footer------------------
[18]: /@mehrdad-?source=post_page---post_author_info--fbf194fae991---------------------------------------
[19]: /@mehrdad-?source=post_page---post_author_info--fbf194fae991---------------------------------------
[20]: /@mehrdad-?source=post_page---post_author_info--fbf194fae991---------------------------------------
[21]: /@mehrdad-/followers?source=post_page---post_author_info--fbf194fae991---------------------------------------
[22]: /@mehrdad-/following?source=post_page---post_author_info--fbf194fae991---------------------------------------
[23]: https://help.medium.com/hc/en-us?source=post_page-----fbf194fae991---------------------------------------
[24]: https://status.medium.com/?source=post_page-----fbf194fae991---------------------------------------
[25]: /about?autoplay=1&source=post_page-----fbf194fae991---------------------------------------
[26]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----fbf194fae991-------------------------------------
--
[27]: mailto:pressinquiries@medium.com
[28]: https://blog.medium.com/?source=post_page-----fbf194fae991---------------------------------------
[29]: https://medium.com/store
[30]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----fbf194fae991--------------------
-------------------
[31]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----fbf194fae991-----------------------------
----------
[32]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----fbf194fae991------------------
---------------------
[33]: https://speechify.com/medium?source=post_page-----fbf194fae991---------------------------------------
```
