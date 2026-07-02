# Web source

- URL: https://www.salesforce.com/blog/leveraging-language-models-for-commonsense
- Title: [
- Captured (UTC): 2026-06-29T16:30:16.951422448+00:00

```text
[
Skip to Content
][1]
0%
[ AI][2]

# Leveraging Language Models for Commonsense Reasoning in Neural Networks

[Featured image for Leveraging Language Models for Commonsense Reasoning in Neural Networks]

Commonsense reasoning that draws upon world knowledge derived from spatial and temporal relations, laws of physics,
causes and effects, and social conventions is a feature of human intelligence.

[
Bryan McCann
][3] [
Nazneen Rajani
][4]
June 27, 2019 6 min read

## Share article

Commonsense reasoning that draws upon world knowledge derived from spatial and temporal relations, laws of physics,
causes and effects, and social conventions is a feature of human intelligence.  However, it is difficult to instill such
commonsense reasoning abilities into artificial intelligence implemented by deep neural networks. While neural networks
effectively learn from a large number of examples, commonsense reasoning for humans precisely hits upon the kind of
reasoning that is in less need of exemplification. Rather, humans pick up the kind of knowledge required to do
commonsense reasoning simply by living in the world and doing everyday things.

AI models have limited access to the kind of world knowledge that is necessary for commonsense reasoning. Neural
networks designed for tasks that use natural language often only see text — no visual data, sounds, tactile sensations,
or scents are known to these networks. Since these natural language processing networks are limited to text alone, as a
poor substitute for living in the world, we have them read a human-mind-boggling amount of text, including all of
Wikipedia and thousands of books.

We then probe the commonsense reasoning capacity of the neural network by using a multiple-choice test called
[Commonsense Question Answering (CQA)][5] [Talmor et al., 2019]. The neural network trains on a few examples from CQA
that require commonsense reasoning to answer. Then, we administer the real test with questions the network has never
seen. Compared to humans, these well-read neural networks have known to perform quite poorly on this task [Talmor et
al., 2019].

We instill commonsense reasoning in neural networks by way of explanations and show that it improves performance on the
CQA as shown below.

## Commonsense Explanations (CoS-E) Dataset

The first part of our work proposes to show the network examples not only of questions with answer choices but also of
the thought process that humans use to answer such commonsense questions. In order to get this information for the
network, we ask humans to annotate these multiple-choice-question answer pairs with explanations that mimic their
internal commonsense reasoning. Specifically, the human annotators are prompted with the following question: “Why is the
predicted output the most appropriate answer?” Annotators were instructed to highlight relevant words in the question
that justifies the ground-truth answer choice and to provide a brief open-ended explanation based on the highlighted
justification could serve as the commonsense reasoning behind the question. The figure below shows examples from the CQA
dataset along with the collected human commonsense explanations (CoS-E) dataset.

The CoS-E dataset can then be shown to the network while it is being trained to do the multiple-choice tests alongside
the original input question and answer choices. Surprisingly, even though it does not have the CoS-E dataset during the
real test, the network performs much better on the real test after seeing examples of human reasoning only during
training. Though we do not yet fully understand how using the CoS-E dataset during training would benefit the networks
even when CoS-E is not present at test time, we speculate that the explanations capture valuable information about the
way the world works and the network learns to reason based on that information at test time. This well-read model is a
pre-trained transformer neural network called BERT [Devlin et al., 2019].

## Commonsense Auto-Generated Explanation (CAGE) Model

The second part of our work proposes to provide a form of reasoning for the network even during the real test. CoS-E
cannot be used during the testing phase because it would assume that we had already told a human the correct answer and
they wrote down an explanation for that answer. But, if the network could learn to generate its own kind of reasoning,
it could do so during the real test as well.

In order to do this, we train a second neural network that only learns how to generate commonsense reasoning and is not
burdened by the task of multiple-choice question answering. We assume that this network starts out having read a lot of
text, just as the test-taking network had. We then show it the commonsense questions and the answer choices. We do not
tell the network the correct answer, but we train it to generate and mimic the human explanations from the CoS-E
dataset. In this way, the network is trained to look at a question with answer choices and write down what it is
thinking. Because this process does not depend on knowing the correct answer, we can use the commonsense auto-generate
explanation (CAGE) model on the real test too.

This neural network that automatically generates explanations is a transformer language model (LM) called OpenAI GPT
[Radford et al., 2018]. Figure below shows the process of how the LM generates explanation tokens given the input
question and answer choices.

Now if we allow the CAGE model to generate commonsense reasoning for the test-taking network, i.e. the BERT, on the real
test, we see marked improvements in the test score as shown in the figure below.

The CAGE model beats our own prior performance of using the CoS-E dataset only during training. The table below shows
the results obtained by both our models compared with other state-of-the-art deep learning models. As shown, the
state-of-the-art deep neural networks are still lagging far behind human performance on this task.

Here are some examples from the actual CQA data with human CoS-E data and our autogenerated reasoning from the CAGE
model. We observed that our model’s reasoning typically employs a much simpler construction.  Nonetheless, this simple
declarative mode can sometimes be more informative than explanations from the CoS-E dataset. The CAGE model achieves
this by either providing more explicit guidance (as in the final example) or by adding meaningful context (as in the
second example by introducing the word ‘friends’).

We also extend our explanation generation work to other out-of-domain tasks such as story completion on the [Story
Cloze][6] data [Mostafazadeh et al., 2016] and next scene prediction in the [SWAG][7] data [Zellers et al., 2018]. The
LM neural network is asked to generate explanations on these tasks without actually training on these tasks, just by
transferring its learned commonsense reasoning abilities from the CQA task. Here are some examples of the generated
explanations on both these datasets.

In the SWAG dataset, each question is a video caption from activity recognition videos with choices about what might
happen next and the correct answer is the video caption of the next scene. Generated explanations for SWAG appear to be
grounded in the given images even though the language model was not at all trained on SWAG. Similarly, we found that for
the Story Cloze dataset, the explanations had information pointing to the correct ending.

## Conclusion

In summary, we introduced the Common Sense Explanations (CoS-E) dataset built on top of the existing CQA dataset. We
also proposed the novel Commonsense Auto-Generated Explanations (CAGE) model that trains a language model to generate
useful explanations when fine-tuned on the problem input and human explanations. These explanations can then be used by
a classifier model to make predictions. We empirically show that such an approach not only results in state-of-the-art
performance on a difficult commonsense reasoning task but also opens further avenues for studying explanation as it
relates to interpretable commonsense reasoning. We also extended explanation transfer to out-of-domain datasets.Citation
Credit

> Nazneen Fatema Rajani, Bryan McCann, Caiming Xiong and Richard Socher. [Explain Yourself! Leveraging Language Models
> for Commonsense Reasoning. ][8]In Proceedings of the 2019 Conference of the Association for Computational Linguistics
> (ACL2019).

### References:
1. Jacob Devlin, Ming-Wei Chang, Kenton Lee, and Kristina Toutanova.BERT: Pre-training of deep bidirectional
   transformers for language understanding. In Proceedings of the 2019 Conference of the North American Chapter of the
   Association for Computational Linguistics: Human Language Technologies (NAACL2019).
2. Nasrin Mostafazadeh, Nathanael Chambers, Xiaodong He, Devi Parikh, Dhruv Batra, Lucy Vanderwende, Pushmeet Kohli, and
   James Allen. A corpus and cloze evaluation for deeper understanding of commonsense stories. In Proceedings of the
   2016 Conference of the North American Chapter of the Association for Computational Linguistics: Human Language
   Technologies (NAACL2016).
3. Alec Radford, Karthik Narasimhan, Tim Salimans, and Ilya Sutskever. 2018. Improving language understanding by
   generative pre-training.
4. Alon Talmor, Jonathan Herzig, Nicholas Lourie, and Jonathan Berant. CommonsenseQA: A question answering challenge
   targeting commonsense knowledge. In Proceedings of the 2019 Conference of the North American Chapter of the
   Association for Computational Linguistics: Human Language Technologies (NAACL2019).
5. Rowan Zellers, Yonatan Bisk, Roy Schwartz, and Yejin Choi. Swag: A large-scale adversarial dataset for grounded
   commonsense inference. In Proceedings of the 2018 Conference on Empirical Methods in Natural Language Processing
   (EMNLP2018).

## Share article

## Just For You

[ ][9]

### [ 5 Ways Brands Are Turning Loyalty and Referral Marketing into Big-Time Growth ][10]

5 min read
[ ][11]

### [ Top Takeaways from Connections 2026 ][12]

5 min read

## Explore related content by topic
* [ AI ][13]
* [ AI Research ][14]

Bryan McCann


[ More by Bryan ][15]
Nazneen Rajani


[ More by Nazneen ][16]

## Get the latest articles in your inbox.

Sign up now

## Just For You

[ [Digital Origination for Loans and Deposit Accounts] ][17]

### [ Why Does Originating a Loan or Opening an Account in 2026 Still Feel Like 2006? ][18]

9 min read
[ [A diagram shows a user interface within a continuous cycle, surrounded by security, integration, validation, and
privacy icons.] ][19]

### [ Summer ‘26 Release: Top Development & Security Features ][20]

4 min read
[ [A series of arrow blows around a circuit with AI in the middle on a purple background.] ][21]

### [ AI Adoption For Startups: Ready Your Team for the Fast-Track ][22]

5 min read
[ [African american women shaking hands with man celebrating success, with a crowd of customers around, signifying
customer trust in an office setting.] ][23]

### [ Customer Trust in An AI World: If You Build It, They Will Come ][24]

7 min read
[ ][25]

## [ Industry Insider: 5 Martech Trends to Watch This Summer ][26]

7 min read
[ [an AI Agent Orchestrating Business Workflows in icons on black background] ][27]

## [ AI Agent Culture: What Does it Mean For Your Lean Team? ][28]

6 min read
[ ][29]

## [ What is the MCP Server and What Does it Mean for Your Marketing? ][30]

9 min read

## [ Model Cards for AI Model Transparency ][31]

2 min read

Get the latest articles in your inbox.

Sign up now
Close

## Get the latest articles in your inbox.

### 360 Highlights

Selected

### IT

Selected

### Commerce

Selected

### Marketing

Selected

### Service

Selected

### Sales

Selected
Please select at least one newsletter.
Email Address Enter a valid e-mail address
Select your country United States Afghanistan Albania Algeria American Samoa Andorra Anguilla Antarctica Antigua &
Barbuda Argentina Armenia Aruba Australia Austria Azerbaijan Bahamas Bahrain Bangladesh Barbados Belarus Belgium Belize
Benin Bermuda Bhutan Bolivia Bosnia & Herzegovina Botswana Bouvet Island Brazil British Indian Ocean Territory British
Virgin Islands Brunei Bulgaria Burkina Faso Burundi Cambodia Cameroon Canada Cape Verde Caribbean Netherlands Cayman
Islands Central African Republic Chad Chile China Christmas Island Cocos (Keeling) Islands Colombia Comoros Congo -
Brazzaville Congo - Kinshasa Cook Islands Costa Rica Croatia Curaçao Cyprus Czechia Côte d’Ivoire Denmark Djibouti
Dominica Dominican Republic Ecuador Egypt El Salvador Equatorial Guinea Eritrea Estonia Eswatini Ethiopia Falkland
Islands Faroe Islands Fiji Finland France French Guiana French Polynesia French Southern Territories Gabon Gambia
Georgia Germany Ghana Gibraltar Greece Greenland Grenada Guadeloupe Guam Guatemala Guinea Guinea-Bissau Guyana Haiti
Heard & McDonald Islands Honduras Hong Kong SAR China Hungary Iceland India Indonesia Ireland Israel Italy Jamaica Japan
Jordan Kazakhstan Kenya Kiribati Kuwait Kyrgyzstan Laos Latvia Lebanon Lesotho Liberia Liechtenstein Lithuania
Luxembourg Macao SAR China Madagascar Malawi Malaysia Maldives Mali Malta Marshall Islands Martinique Mauritania
Mauritius Mayotte Mexico Micronesia Moldova Monaco Mongolia Montserrat Morocco Mozambique Myanmar (Burma) Namibia Nauru
Nepal Netherlands New Caledonia New Zealand Nicaragua Niger Nigeria Niue Norfolk Island North Macedonia Northern Mariana
Islands Norway Oman Pakistan Palau Panama Papua New Guinea Paraguay Peru Philippines Pitcairn Islands Poland Portugal
Puerto Rico Qatar Romania Russia Rwanda Réunion Samoa San Marino Saudi Arabia Senegal Serbia Seychelles Sierra Leone
Singapore Sint Maarten Slovakia Slovenia Solomon Islands Somalia South Africa South Georgia & South Sandwich Islands
South Korea Spain Sri Lanka St. Helena St. Kitts & Nevis St. Lucia St. Pierre & Miquelon St. Vincent & Grenadines
Suriname Svalbard & Jan Mayen Sweden Switzerland São Tomé & Príncipe Taiwan Tajikistan Tanzania Thailand Timor-Leste
Togo Tokelau Tonga Trinidad & Tobago Tunisia Turkey Turkmenistan Turks & Caicos Islands Tuvalu U.S. Outlying Islands
U.S. Virgin Islands Uganda Ukraine United Arab Emirates United Kingdom Uruguay Uzbekistan Vanuatu Vatican City Venezuela
Vietnam Wallis & Futuna Western Sahara Yemen Zambia Zimbabwe Select your Country Select your Country
State/province 北海道 - Hokkaido 青森県 - Aomori 岩手県 - Iwate 宮城県 - Miyagi 秋田県 - Akita 山形県 - Yamagata 福島県
- Fukushima 茨城県 - Ibaraki 栃木県 - Tochigi 群馬県 - Gunma 埼玉県 - Saitama 千葉県 - Chiba 東京都 - Tokyo 神奈川県 -
Kanagawa 新潟県 - Niigata 富山県 - Toyama 石川県 - Ishikawa 福井県 - Fukui 山梨県 - Yamanashi 長野県 - Nagano 岐阜県 -
Gifu 静岡県 - Shizuoka 愛知県 - Aichi 三重県 - Mie 滋賀県 - Shiga 京都府 - Kyoto 大阪府 - Osaka 兵庫県 - Hyogo 奈良県 -
Nara 和歌山県 - Wakayama 鳥取県 - Tottori 島根県 - Shimane 岡山県 - Okayama 広島県 - Hiroshima 山口県 - Yamaguchi 徳島県
- Tokushima 香川県 - Kagawa 愛媛県 - Ehime 高知県 - Kochi 福岡県 - Fukuoka 佐賀県 - Saga 長崎県 - Nagasaki 熊本県 -
Kumamoto 大分県 - Oita 宮崎県 - Miyazaki 鹿児島県 - Kagoshima 沖縄県 - Okinawa Select a state/province Select a
state/province
State/province Alberta British Columbia Manitoba New Brunswick Newfoundland Northwest Territories Nova Scotia Nunavut
Ontario Prince Edward Island Quebec Saskatchewan Yukon Select a state/province Select a state/province
State/province Alabama Alaska Arizona Arkansas California Colorado Connecticut Delaware District of Columbia Florida
Georgia Hawaii Idaho Illinois Indiana Iowa Kansas Kentucky Louisiana Maine Maryland Massachusetts Michigan Minnesota
Mississippi Missouri Montana Nebraska Nevada New Hampshire New Jersey New Mexico New York North Carolina North Dakota
Ohio Oklahoma Oregon Pennsylvania Rhode Island South Carolina South Dakota Tennessee Texas Utah Vermont Virginia
Washington West Virginia Wisconsin Wyoming Select a state/province Select a state/province

**Yes,** I would like to receive the Salesforce 360 Highlights newsletter as well as marketing emails regarding
Salesforce products, services, and events. I can unsubscribe at any time.

I agree to the [Privacy Statement][32] and to the [handling of my personal information][33]. In particular, I consent to
the transfer of my personal information to other countries, including the United States, for the purpose of hosting and
processing the information as set forth in the Privacy Statement. [Learn More][34]
I understand that these countries may not have the same data protection laws as the country from which I provide my
personal information. For more information, click [here.][35]
Please read and agree to the Master Subscription Agreement

By registering, you confirm that you agree to the processing of your personal data by Salesforce as described in the
[Privacy Statement][36].

Sign up now

## Thanks, you're subscribed!

[Salesforce logo]
[ [Facebook] ][37] [ [X] ][38] [ [LinkedIn] ][39] [ [Instagram] ][40] [ [YouTube] ][41]
CALL US AT [1-800-664-9073][42]

New to Salesforce?
* [What is Salesforce?][43]
* [Best CRM software][44]
* [Explore all products][45]
* [What is cloud computing][46]
* [Customer success][47]
* [Product pricing][48]

About Salesforce
* [Our story][49]
* [Press][50]
* [Blog][51]
* [Careers][52]
* [Trust][53]
* [Salesforce.org][54]
* [Sustainability][55]
* [Investors][56]
* [Legal][57]

Popular Links
* [Salesforce Mobile][58]
* [AppExchange][59]
* [Dreamforce][60]
* [CRM software][61]
* [Salesforce LIVE][62]
* [Salesforce for startups][63]

Worldwide

Americas
* [América Latina (Español)][64]
* [Brasil (Português)][65]
* [Canada (English)][66]
* [Canada (Français)][67]
* [United States (English)][68]

Europe, Middle East, and Africa
* [España (Español)][69]
* [Deutschland (Deutsch)][70]
* [France (Français)][71]
* [Italia (Italiano)][72]
* [Nederland (Nederlands)][73]
* [Sverige (Svenska)][74]
* [United Kingdom (English)][75]
* [All other countries (English)][76]

Asia Pacific
* [Australia (English)][77]
* [India (English)][78]
* [日本 (日本語)][79]
* [中国 (简体中文)][80]
* [香港 (繁體中文)][81]
* [台灣 (繁體中文)][82]
* [한국 (한국어)][83]
* [Malaysia (English)][84]
* [ประเทศไทย (ไทย)][85]
* [All other countries (English)][86]

© Copyright 2026 Salesforce, Inc. [All rights reserved. ][87]Various trademarks held by their respective owners.
Salesforce, Inc. Salesforce Tower, 415 Mission Street, 3rd Floor, San Francisco, CA 94105, United States
* [Legal][88]
* [Terms of Service][89]
* [Privacy Information][90]
* [Responsible Disclosure][91]
* [Trust][92]
* [Contact][93]
* [Cookie Preferences][94]
* [Your Privacy Choices][95]

Copied

[1]: #main-content
[2]: https://www.salesforce.com/blog/category/ai/
[3]: https://www.salesforce.com/blog/author/bryan-mccann/
[4]: https://www.salesforce.com/blog/author/nazneen-rajani/
[5]: https://www.tau-nlp.org/commonsenseqa
[6]: http://cs.rochester.edu/nlp/rocstories/
[7]: https://rowanzellers.com/swag/
[8]: https://arxiv.org/abs/1906.02361
[9]: https://www.salesforce.com/blog/loyalty-and-referral-marketing/
[10]: https://www.salesforce.com/blog/loyalty-and-referral-marketing/
[11]: https://www.salesforce.com/blog/connections-conference-takeaways/
[12]: https://www.salesforce.com/blog/connections-conference-takeaways/
[13]: https://www.salesforce.com/blog/category/ai/
[14]: https://www.salesforce.com/blog/category/ai-research/
[15]: https://www.salesforce.com/blog/author/bryan-mccann/
[16]: https://www.salesforce.com/blog/author/nazneen-rajani/
[17]: https://www.salesforce.com/blog/digital-loan-origination-software/
[18]: https://www.salesforce.com/blog/digital-loan-origination-software/
[19]: https://www.salesforce.com/blog/platform-summer-26-release/
[20]: https://www.salesforce.com/blog/platform-summer-26-release/
[21]: https://www.salesforce.com/blog/small-business/ai-adoption-for-startups/
[22]: https://www.salesforce.com/blog/small-business/ai-adoption-for-startups/
[23]: https://www.salesforce.com/blog/small-business/customer-trust-with-ai/
[24]: https://www.salesforce.com/blog/small-business/customer-trust-with-ai/
[25]: https://www.salesforce.com/blog/martech-trends/
[26]: https://www.salesforce.com/blog/martech-trends/
[27]: https://www.salesforce.com/blog/small-business/ai-agent-culture/
[28]: https://www.salesforce.com/blog/small-business/ai-agent-culture/
[29]: https://www.salesforce.com/blog/marketing-mcp-server/
[30]: https://www.salesforce.com/blog/marketing-mcp-server/
[31]: https://www.salesforce.com/blog/model-cards-for-ai-model-transparency/
[32]: https://www.salesforce.com/jp/company/privacy/full_privacy/
[33]: https://www.salesforce.com/jp/company/privacy-notification-a3/
[34]: #
[35]: https://www.salesforce.com/jp/company/privacy/full_privacy/
[36]: https://www.salesforce.com/company/privacy/full_privacy/
[37]: https://www.facebook.com/salesforce
[38]: https://x.com/salesforce
[39]: https://www.linkedin.com/company/salesforce
[40]: https://www.instagram.com/salesforce/?hl=en
[41]: https://www.youtube.com/Salesforce
[42]: tel:1-800-664-9073
[43]: https://www.salesforce.com/products/what-is-salesforce/
[44]: https://www.salesforce.com/company/recognition/
[45]: https://www.salesforce.com/products/
[46]: https://www.salesforce.com/products/platform/best-practices/cloud-computing/?d=70130000000i88b
[47]: https://www.salesforce.com/customer-success-stories/
[48]: https://www.salesforce.com/editions-pricing/overview/
[49]: https://www.salesforce.com/company/about-us/
[50]: https://www.salesforce.com/news/press-releases/
[51]: https://www.salesforce.com/blog/
[52]: https://www.salesforce.com/company/careers/
[53]: https://trust.salesforce.com/en/
[54]: https://www.salesforce.org/
[55]: https://www.salesforce.com/company/sustainability/
[56]: https://investor.salesforce.com/overview/default.aspx
[57]: https://www.salesforce.com/company/legal/
[58]: https://www.salesforce.com/solutions/mobile/overview/?d=70130000000i7zy
[59]: https://appexchange.salesforce.com/
[60]: https://www.salesforce.com/dreamforce/
[61]: https://www.salesforce.com/crm/?d=7010M000001wyI9
[62]: https://www.salesforce.com/video/
[63]: https://www.salesforce.com/solutions/salesforce-for-startups/overview/
[64]: https://www.salesforce.com/mx/
[65]: https://www.salesforce.com/br/
[66]: https://www.salesforce.com/ca/
[67]: https://www.salesforce.com/fr-ca/
[68]: https://www.salesforce.com/
[69]: https://www.salesforce.com/es
[70]: https://www.salesforce.com/de/
[71]: https://www.salesforce.com/fr/
[72]: https://www.salesforce.com/it/
[73]: https://www.salesforce.com/nl/
[74]: https://www.salesforce.com/se/
[75]: https://www.salesforce.com/uk/
[76]: https://www.salesforce.com/eu/
[77]: https://www.salesforce.com/au/
[78]: https://www.salesforce.com/in/
[79]: https://www.salesforce.com/jp/
[80]: https://www.salesforce.com/cn/
[81]: https://www.salesforce.com/hk/
[82]: https://www.salesforce.com/tw/
[83]: https://www.salesforce.com/kr/
[84]: https://www.salesforce.com/my/
[85]: https://www.salesforce.com/th/
[86]: https://www.salesforce.com/ap/
[87]: https://www.salesforce.com/company/legal/intellectual/
[88]: https://www.salesforce.com/company/legal/
[89]: https://www.salesforce.com/company/legal/sfdc-website-terms-of-service/
[90]: https://www.salesforce.com/company/privacy.jsp
[91]: https://www.salesforce.com/company/disclosure/
[92]: https://trust.salesforce.com/
[93]: https://www.salesforce.com/company/contact-us/
[94]: javascript:void(0)
[95]: /form/other/privacy-request/?d=cta-footer-1
```
