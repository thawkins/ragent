# Web source

- URL: https://learn.microsoft.com/en-us/answers/questions/5553547/foundry-charges-for-evaluations-vs-standard-api-ca
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T15:41:41.435590214+00:00

```text
[ Skip to main content ][1]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][2] [ More info about Internet Explorer and Microsoft Edge ][3]
Follow question
Add Add to plan

#### Share via

[ Facebook ][4] [ x.com ][5] [ LinkedIn ][6] [ Email ][7]

# Foundry charges for evaluations vs. standard API calls

[ carrie rie ][8] 20 Reputation points
2025-09-12T18:52:07.98+00:00

Is there a difference in how Foundry charges for evaluations vs. standard API calls? Do evaluations run multiple passes
or log extra usage that contributes to the bill? Also, is there a way to estimate or control the cost before running an
evaluation so it doesn’t spike unexpectedly?

Azure Advisor
[ Azure Advisor ][9]

An Azure personalized recommendation engine that helps users follow best practices to optimize Azure deployments.

Sign in to follow Follow
0 comments No comments Report a concern
I have the same question (0)
[ Sign in to comment ][10]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

Answer accepted by question author

[ Azar ][11] • Follow 31,720 Reputation points • MVP • Volunteer Moderator
2025-09-12T19:37:30.34+00:00

Hi there [carrie rie][12]

thnx for using qana

Yes, evaluations in Azure AI Foundry can cost more than direct API calls because each test case may trigger multiple
prompt runs plus extra logging for scoring. That means you’re paying for more tokens than a single request. To keep
costs down, try smaller sample sets first, monitor usage with Azure Cost Analysis/budgets, and if you don’t need
built-in scoring, consider testing directly through the API where you have more control over the calls.

**If this helps kindly accept the response thanks much.**

Was this answer helpful?

Yes No
1 person found this answer helpful.
0 comments No comments Report a concern
[ Sign in to comment ][13]
Add comment
Comment Use comments to ask for clarification, additional information, or improvements to the question.
Discard draft Add comment

## 0 additional answers

Sort by: Most helpful
[Most helpful][14] [Newest][15] [Oldest][16]
[ Sign in to answer ][17]

## Your answer

Answer Answers can be marked as 'Accepted' by the question author and 'Recommended' by moderators, which helps users
know the answer solved the author's problem.
Post answer Discard draft
[en-us][18]
[ Your Privacy Choices][19]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][20]
* [Previous Versions][21]
* [Blog][22]
* [Contribute][23]
* [Privacy][24]
* [Consumer Health Privacy][25]
* [Terms of Use][26]
* [Code of Conduct][27]
* [Trademarks][28]
* © Microsoft 2026

[1]: #main
[2]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[3]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[4]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5553
547%2Ffoundry-charges-for-evaluations-vs-standard-api-ca 											
[5]: https://twitter.com/intent/tweet?original_referer=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2
F5553547%2Ffoundry-charges-for-evaluations-vs-standard-api-ca 											&text=Foundry%20charges%20for%20evaluations%20vs.%20standa
rd%20API%20calls 											&tw_p=tweetbutton&url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5553547%2Ffound
ry-charges-for-evaluations-vs-standard-api-ca
[6]: https://www.linkedin.com/cws/share?url=https%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%2F5553547%2F
foundry-charges-for-evaluations-vs-standard-api-ca
[7]: mailto:?subject= 												%5BShared%20Question%5D%20Foundry%20charges%20for%20evaluations%20vs.%20standard%20API%20calls&bod
y=Foundry charges for evaluations vs. standard API callshttps%3A%2F%2Flearn.microsoft.com%2Fen-us%2Fanswers%2Fquestions%
2F5553547%2Ffoundry-charges-for-evaluations-vs-standard-api-ca
[8]: /en-us/users/na/?userid=b686f4ff-ff6e-4ebb-90e1-376e1e2075cd
[9]: /en-us/answers/tags/398/azure-advisor/
[10]: #
[11]: /en-us/users/na/?userid=852a7cae-9e0a-4faf-a879-f96214e21230
[12]: https://learn.microsoft.com/en-us/users/na/?userid=b686f4ff-ff6e-4ebb-90e1-376e1e2075cd
[13]: #
[14]: ?orderby=helpful&page=1#answers
[15]: ?orderby=newest&page=1#answers
[16]: ?orderby=oldest&page=1#answers
[17]: #
[18]: #
[19]: https://aka.ms/yourcaliforniaprivacychoices
[20]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[21]: https://learn.microsoft.com/en-us/previous-versions/
[22]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[23]: https://learn.microsoft.com/en-us/contribute
[24]: https://go.microsoft.com/fwlink/?LinkId=521839
[25]: https://go.microsoft.com/fwlink/?linkid=2259814
[26]: https://learn.microsoft.com/en-us/legal/termsofuse
[27]: https://aka.ms/msftqacodeconduct
[28]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
