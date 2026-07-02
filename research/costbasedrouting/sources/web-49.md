# Web source

- URL: https://learn.microsoft.com/en-ca/answers/questions/5552043/azure-ai-foundry-model-router-can-we-add-external
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:43:10.687604557+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Azure AI Foundry Model Router – Can we add external models or restrict routing to specific models?

[ Dharun Balaji ][8] 60 Reputation points
2025-09-11T12:51:05.19+00:00

I’m working with the **Azure AI Foundry Model Router** (deployment version 2025-05-19), which automatically chooses
between models like **GPT-4.1-nano, GPT-4.1-mini, GPT-4.1, o4-mini, GPT-5-nano, GPT-5-mini, GPT-5-chat, GPT-5** based on
query complexity.

This auto-routing works well for cost/performance optimization, but I have some specific requirements:

I would like some clarification on routing controls:
1. **Can we add external Azure models** (custom or fine-tuned deployments) into the router’s pool so they can be
   selected?
2. **Can we restrict or block certain models** from being used by the router (e.g., prevent gpt-5-mini)?
3. **Is there an API parameter or configuration** (custom headers) to control which models the router uses?
4. **Can prompt engineering or request settings influence routing** (e.g., force reasoning-capable models)?
5. **Is there a roadmap** for exposing more direct control over routing in future versions?

Any guidance on these points, or insight into future plans for routing control, would be greatly appreciated. Thank you!

Foundry Tools
[ Foundry Tools ][9]

Formerly known as Azure AI Services or Azure Cognitive Services is a unified collection of prebuilt AI capabilities
within the Microsoft Foundry platform

Sign in to follow Follow
0 comments No comments Report a concern
I have the same question (0)
[ Sign in to comment ][10]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

Answer accepted by question author

[ Gowtham CP ][11] • Follow 7,960 Reputation points • Volunteer Moderator
2025-09-12T04:43:13.6066667+00:00

Hello [Dharun Balaji][12] ,

**Thank you for reaching out on Microsoft Q&A.**

For the Azure AI Foundry **Model Router** (deployment version 2025-05-19):

You cannot add external or fine-tuned Azure models into the router’s pool. Only the models defined by Microsoft are
used.

There is no option today to restrict or block specific models from routing.

There are no API parameters or custom headers that allow you to control or limit model selection.

Prompt engineering and request settings (e.g., temperature, max tokens) affect generation, but they do not force the
router to select a reasoning-capable model.

At this time, there is no published roadmap for exposing direct routing controls. If you need strict control, you would
deploy and call a specific model directly instead of relying on the router.

You can read more here:

[1] [Model Router concepts][13]

[2] [How to use Model Router][14]

I hope this helps!

**If the information is useful, please accept the answer and upvote it to assist other community members.**

Was this answer helpful?

Yes No
1 person found this answer helpful.
0 comments No comments Report a concern
[ Sign in to comment ][15]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 0 additional answers

Sort by: Most helpful
[Most helpful][16] [Newest][17] [Oldest][18]
[ Sign in to answer ][19]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-ca][20]
[ Your Privacy Choices][21]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][22]
* [Previous Versions][23]
* [Blog][24]
* [Contribute][25]
* [Privacy][26]
* [Consumer Health Privacy][27]
* [Terms of Use][28]
* [Code of Conduct][29]
* [Trademarks][30]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5552
043%2Fazure-ai-foundry-model-router-can-we-add-external 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2
F5552043%2Fazure-ai-foundry-model-router-can-we-add-external 											&text=Azure%20AI%20Foundry%20Model%20Router%20%E2%80%93%20C
an%20we%20add%20external%20models%20or%20restrict%20routing%20to%20specific%20models%3F 											&tw_p=tweetbutton&url=https%3A%2
F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5552043%2Fazure-ai-foundry-model-router-can-we-add-external
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F5552043%2F
azure-ai-foundry-model-router-can-we-add-external
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Azure%20AI%20Foundry%20Model%20Router%20%E2%80%93%20Can%20we%20add%20ext
ernal%20models%20or%20restrict%20routing%20to%20specific%20models%3F&body=Azure AI Foundry Model Router – Can we add ext
ernal models or restrict routing to specific models?https%3A%2F%2Flearn.microsoft.com%2Fen-ca%2Fanswers%2Fquestions%2F55
52043%2Fazure-ai-foundry-model-router-can-we-add-external
[8]: /en-ca/users/na/?userid=190acfe7-69c9-49d7-b32e-4cc97456d74d
[9]: /en-ca/answers/tags/1580/foundry-tools/
[10]: #
[11]: /en-ca/users/na/?userid=271f3ae4-c343-450a-8de2-6a9085573909
[12]: https://learn.microsoft.com/en-us/users/na/?userid=190acfe7-69c9-49d7-b32e-4cc97456d74d
[13]: https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/model-router?utm_source=chatgpt.com
[14]: https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/model-router?utm_source=chatgpt.com
[15]: #
[16]: ?orderby=helpful&page=1#answers
[17]: ?orderby=newest&page=1#answers
[18]: ?orderby=oldest&page=1#answers
[19]: #
[20]: #
[21]: https://aka.ms/yourcaliforniaprivacychoices
[22]: https://learn.microsoft.com/en-ca/principles-for-ai-generated-content
[23]: https://learn.microsoft.com/en-ca/previous-versions/
[24]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[25]: https://learn.microsoft.com/en-ca/contribute
[26]: https://go.microsoft.com/fwlink/?LinkId=521839
[27]: https://go.microsoft.com/fwlink/?linkid=2259814
[28]: https://learn.microsoft.com/en-ca/legal/termsofuse
[29]: https://aka.ms/msftqacodeconduct
[30]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
