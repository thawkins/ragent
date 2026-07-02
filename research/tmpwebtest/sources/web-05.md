# Web source

- URL: https://developer.ibm.com/articles/os-developers-know-rust
- Title: [IBM ** Developer**][1]
- Captured (UTC): 2026-06-29T00:55:22.138573056+00:00

```text
[IBM ** Developer**][1]
* [Explore][2]
  * * [Articles][3]
    * [Blogs][4]
    * [Courses][5]
    * [Learning paths][6]
    * [Open projects][7]
    * [Series][8]
    * [Tutorials][9]
    * [
      
      #### Products
      
      ][10]
    * [IBM Bob][11]
    * [Granite models][12]
    * [Open Liberty][13]
    * [watsonx.ai][14]
    * [watsonx.data][15]
    * [Docling][16]
    * [
      
      #### Languages
      
      ][17]
    * [IBM Semeru Runtimes][18]
    * [Java][19]
    * [Python][20]
    * [Node.js][21]
    * [JavaScript][22]
    * [COBOL][23]
    * [
      
      #### Technologies
      
      ][24]
    * [Artificial intelligence][25]
    * [Data Science][26]
    * [Messaging][27]
    * [Machine Learning][28]
    * [Observability][29]
    * [Security][30]
* [Events][31]
  * * [
      
      #### All Events
      
      ][32]
    * [IBM Hackathons][33]
    * [IBM Community Events][34]
    * [TechXchange Conference][35]
* [Resources][36]
  * * #### External Resources
    * [IBM Documentation][37]
    * [IBM Support][38]
    * [IBM Developer Videos][39]
    * [IBM Technology Videos][40]
    * [Open Source @ IBM][41]
    * [TechXchange][42]
* [Home][43]
* * #### Explore
    
    [Articles][44]
    
    [Blogs][45]
    
    [Courses][46]
    
    [Learning paths][47]
    
    [Open projects][48]
    
    [Series][49]
    
    [Tutorials][50]
    
    Products
    * [IBM Bob][51]
    * [Granite models][52]
    * [Open Liberty][53]
    * [watsonx.ai][54]
    * [watsonx.data][55]
    * [Docling][56]
    
    Languages
    * [IBM Semeru Runtimes][57]
    * [Java][58]
    * [Python][59]
    * [Node.js][60]
    * [JavaScript][61]
    * [COBOL][62]
    
    Technologies
    * [Artificial intelligence][63]
    * [Data Science][64]
    * [Messaging][65]
    * [Machine Learning][66]
    * [Observability][67]
    * [Security][68]
    
  * #### Events
    
    All Events
    * [IBM Hackathons][69]
    * [IBM Community Events][70]
    * [TechXchange Conference][71]
    
  * #### Resources
    
    External Resources
    * [IBM Documentation][72]
    * [IBM Support][73]
    * [IBM Developer Videos][74]
    * [IBM Technology Videos][75]
    * [Open Source @ IBM][76]
    * [TechXchange][77]
    
Subscribe
Options
[loading]
Loading page...
[IBM Logo]
* IBM Developer
* [About][78]
* [Third-party notice][79]
* Follow Us
* [X][80]
* [LinkedIn][81]
* [YouTube][82]
* Explore
* [Open Source @ IBM][83]
* [IBM API Hub][84]
* [Contact IBM][85]
* [Privacy][86]
* [Terms of use][87]
* [Accessibility][88]

Article

# Why you should learn the Rust programming language

Discover the history, key concepts, and tools for using Rust

By M. Tim Jones
Save

On this page

[Overview][89]

10 June 2024
Article
Time to read: 13 minutes
Legend
* Technologies
* Development Practices
[
Software development
][90][
Web development
][91]
* Categories
  [
  Software development
  ][92][
  Web development
  ][93]
Interested in generative AI?
[Learn generative AI skills][94]

[1]: https://developer.ibm.com/
[2]: #
[3]: https://developer.ibm.com/articles/?utm_source=developer-site&utm_medium=menu
[4]: https://developer.ibm.com/blogs/?utm_source=developer-site&utm_medium=menu
[5]: https://developer.ibm.com/courses/?utm_source=developer-site&utm_medium=menu
[6]: https://developer.ibm.com/learningpaths/?utm_source=developer-site&utm_medium=menu
[7]: https://developer.ibm.com/openprojects/?utm_source=developer-site&utm_medium=menu
[8]: https://developer.ibm.com/series/?utm_source=developer-site&utm_medium=menu
[9]: https://developer.ibm.com/tutorials/?utm_source=developer-site&utm_medium=menu
[10]: https://developer.ibm.com/components/?utm_source=developer-site&utm_medium=menu-products
[11]: https://developer.ibm.com/components/ibm-bob/?utm_source=developer-site&utm_medium=menu
[12]: https://developer.ibm.com/components/granite-models/?utm_source=developer-site&utm_medium=menu
[13]: https://developer.ibm.com/components/open-liberty/?utm_source=developer-site&utm_medium=menu
[14]: https://developer.ibm.com/components/watsonx-ai/?utm_source=developer-site&utm_medium=menu
[15]: https://developer.ibm.com/components/watsonx-data/?utm_source=developer-site&utm_medium=menu
[16]: https://developer.ibm.com/components/docling/?utm_source=developer-site&utm_medium=menu
[17]: https://developer.ibm.com/languages/?utm_source=developer-site&utm_medium=menu-languages
[18]: https://developer.ibm.com/languages/semeru-runtimes/?utm_source=developer-site&utm_medium=menu
[19]: https://developer.ibm.com/languages/java/?utm_source=developer-site&utm_medium=menu
[20]: https://developer.ibm.com/languages/python/?utm_source=developer-site&utm_medium=menu
[21]: https://developer.ibm.com/languages/node-js/?utm_source=developer-site&utm_medium=menu
[22]: https://developer.ibm.com/languages/javascript/?utm_source=developer-site&utm_medium=menu
[23]: https://developer.ibm.com/languages/cobol/?utm_source=developer-site&utm_medium=menu
[24]: https://developer.ibm.com/technologies/?utm_source=developer-site&utm_medium=menu-technologies
[25]: https://developer.ibm.com/technologies/artificial-intelligence/?utm_source=developer-site&utm_medium=menu
[26]: https://developer.ibm.com/technologies/data-science/?utm_source=developer-site&utm_medium=menu
[27]: https://developer.ibm.com/technologies/messaging/?utm_source=developer-site&utm_medium=menu
[28]: https://developer.ibm.com/technologies/machine-learning/?utm_source=developer-site&utm_medium=menu
[29]: https://developer.ibm.com/devpractices/observability/?utm_source=developer-site&utm_medium=menu
[30]: https://developer.ibm.com/devpractices/security/?utm_source=developer-site&utm_medium=menu
[31]: #
[32]: https://developer.ibm.com/events/?utm_source=developer-site&utm_medium=menu
[33]: https://developer.ibm.com/hackathons/?utm_source=developer-site&utm_medium=menu
[34]: https://www.ibm.com/community/techxchange-events/?utm_source=developer-site&utm_medium=menu
[35]: https://www.ibm.com/community/ibm-techxchange-conference/?utm_source=developer-site&utm_medium=menu
[36]: #
[37]: https://www.ibm.com/docs/en/?utm_source=developer-site&utm_medium=menu
[38]: https://www.ibm.com/support/?utm_source=developer-site&utm_medium=menu
[39]: https://www.youtube.com/channel/UCUm6InQvGI9-6vo1teGWINA?utm_source=developer-site&utm_medium=menu
[40]: https://www.youtube.com/@IBMTechnology?utm_source=developer-site&utm_medium=menu
[41]: https://www.ibm.com/opensource?utm_source=developer-site&utm_medium=menu
[42]: https://www.ibm.com/community/techxchange/?utm_source=developer-site&utm_medium=menu
[43]: https://developer.ibm.com/
[44]: https://developer.ibm.com/articles/?utm_source=developer-site&utm_medium=menu
[45]: https://developer.ibm.com/blogs/?utm_source=developer-site&utm_medium=menu
[46]: https://developer.ibm.com/courses/?utm_source=developer-site&utm_medium=menu
[47]: https://developer.ibm.com/learningpaths/?utm_source=developer-site&utm_medium=menu
[48]: https://developer.ibm.com/openprojects/?utm_source=developer-site&utm_medium=menu
[49]: https://developer.ibm.com/series/?utm_source=developer-site&utm_medium=menu
[50]: https://developer.ibm.com/tutorials/?utm_source=developer-site&utm_medium=menu
[51]: https://developer.ibm.com/components/ibm-bob/?utm_source=developer-site&utm_medium=menu
[52]: https://developer.ibm.com/components/granite-models/?utm_source=developer-site&utm_medium=menu
[53]: https://developer.ibm.com/components/open-liberty/?utm_source=developer-site&utm_medium=menu
[54]: https://developer.ibm.com/components/watsonx-ai/?utm_source=developer-site&utm_medium=menu
[55]: https://developer.ibm.com/components/watsonx-data/?utm_source=developer-site&utm_medium=menu
[56]: https://developer.ibm.com/components/docling/?utm_source=developer-site&utm_medium=menu
[57]: https://developer.ibm.com/languages/semeru-runtimes/?utm_source=developer-site&utm_medium=menu
[58]: https://developer.ibm.com/languages/java/?utm_source=developer-site&utm_medium=menu
[59]: https://developer.ibm.com/languages/python/?utm_source=developer-site&utm_medium=menu
[60]: https://developer.ibm.com/languages/node-js/?utm_source=developer-site&utm_medium=menu
[61]: https://developer.ibm.com/languages/javascript/?utm_source=developer-site&utm_medium=menu
[62]: https://developer.ibm.com/languages/cobol/?utm_source=developer-site&utm_medium=menu
[63]: https://developer.ibm.com/technologies/artificial-intelligence/?utm_source=developer-site&utm_medium=menu
[64]: https://developer.ibm.com/technologies/data-science/?utm_source=developer-site&utm_medium=menu
[65]: https://developer.ibm.com/technologies/messaging/?utm_source=developer-site&utm_medium=menu
[66]: https://developer.ibm.com/technologies/machine-learning/?utm_source=developer-site&utm_medium=menu
[67]: https://developer.ibm.com/devpractices/observability/?utm_source=developer-site&utm_medium=menu
[68]: https://developer.ibm.com/devpractices/security/?utm_source=developer-site&utm_medium=menu
[69]: https://developer.ibm.com/hackathons/?utm_source=developer-site&utm_medium=menu
[70]: https://www.ibm.com/community/techxchange-events/?utm_source=developer-site&utm_medium=menu
[71]: https://www.ibm.com/community/ibm-techxchange-conference/?utm_source=developer-site&utm_medium=menu
[72]: https://www.ibm.com/docs/en/?utm_source=developer-site&utm_medium=menu
[73]: https://www.ibm.com/support/?utm_source=developer-site&utm_medium=menu
[74]: https://www.youtube.com/channel/UCUm6InQvGI9-6vo1teGWINA?utm_source=developer-site&utm_medium=menu
[75]: https://www.youtube.com/@IBMTechnology?utm_source=developer-site&utm_medium=menu
[76]: https://www.ibm.com/opensource?utm_source=developer-site&utm_medium=menu
[77]: https://www.ibm.com/community/techxchange/?utm_source=developer-site&utm_medium=menu
[78]: /about/?lang=en
[79]: /terms/third-party-notice/
[80]: https://twitter.com/IBMDeveloper/
[81]: https://www.linkedin.com/showcase/developerworks/
[82]: https://www.youtube.com/channel/UCUm6InQvGI9-6vo1teGWINA/
[83]: https://www.ibm.com/opensource/
[84]: /apis/
[85]: https://www.ibm.com/contact/global
[86]: https://www.ibm.com/us-en/privacy
[87]: https://www.ibm.com/legal?lnk=flg-able-inen
[88]: https://www.ibm.com/able/?lnk=flg-able-inen
[89]: #overview
[90]: https://developer.ibm.com/devpractices/software-development/?cm_sp=ibmdev-_-developer-_-categorybutton
[91]: https://developer.ibm.com/technologies/web-development/?cm_sp=ibmdev-_-developer-_-categorybutton
[92]: https://developer.ibm.com/devpractices/software-development/?cm_sp=ibmdev-_-developer-_-categorybutton
[93]: https://developer.ibm.com/technologies/web-development/?cm_sp=ibmdev-_-developer-_-categorybutton
[94]: https://developer.ibm.com/technologies/generative-ai/?cm_sp=ibmdev-_-developer-_-getstarted-_-genai
```
