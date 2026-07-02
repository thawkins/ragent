# Web source

- URL: https://github.com/commonsense/conceptnet5/wiki/FAQ
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:30:11.940010226+00:00

```text
[Skip to content][1]

## Navigation Menu

Toggle navigation
[ Sign in ][2]
Appearance settings
* Platform
  * AI CODE CREATION
    * [
      GitHub CopilotWrite better code with AI
      ][3]
    * [
      GitHub Copilot appDirect agents from issue to merge
      ][4]
    * [
      MCP Registry^{New}Integrate external tools
      ][5]
  * DEVELOPER WORKFLOWS
    * [
      ActionsAutomate any workflow
      ][6]
    * [
      CodespacesInstant dev environments
      ][7]
    * [
      IssuesPlan and track work
      ][8]
    * [
      Code ReviewManage code changes
      ][9]
  * APPLICATION SECURITY
    * [
      GitHub Advanced SecurityFind and fix vulnerabilities
      ][10]
    * [
      Code securitySecure your code as you build
      ][11]
    * [
      Secret protectionStop leaks before they start
      ][12]
  * EXPLORE
    * [Why GitHub][13]
    * [Documentation][14]
    * [Blog][15]
    * [Changelog][16]
    * [Marketplace][17]
  [View all features][18]
* Solutions
  * BY COMPANY SIZE
    * [Enterprises][19]
    * [Small and medium teams][20]
    * [Startups][21]
    * [Nonprofits][22]
  * BY USE CASE
    * [App Modernization][23]
    * [DevSecOps][24]
    * [DevOps][25]
    * [CI/CD][26]
    * [View all use cases][27]
  * BY INDUSTRY
    * [Healthcare][28]
    * [Financial services][29]
    * [Manufacturing][30]
    * [Government][31]
    * [View all industries][32]
  [View all solutions][33]
* Resources
  * EXPLORE BY TOPIC
    * [AI][34]
    * [Software Development][35]
    * [DevOps][36]
    * [Security][37]
    * [View all topics][38]
  * EXPLORE BY TYPE
    * [Customer stories][39]
    * [Events & webinars][40]
    * [Ebooks & reports][41]
    * [Business insights][42]
    * [GitHub Skills][43]
  * SUPPORT & SERVICES
    * [Documentation][44]
    * [Customer support][45]
    * [Community forum][46]
    * [Trust center][47]
    * [Partners][48]
  [View all resources][49]
* Open Source
  * COMMUNITY
    * [
      GitHub SponsorsFund open source developers
      ][50]
  * PROGRAMS
    * [Security Lab][51]
    * [Maintainer Community][52]
    * [Accelerator][53]
    * [GitHub Stars][54]
    * [Archive Program][55]
  * REPOSITORIES
    * [Topics][56]
    * [Trending][57]
    * [Collections][58]
* Enterprise
  * ENTERPRISE SOLUTIONS
    * [
      Enterprise platformAI-powered developer platform
      ][59]
  * AVAILABLE ADD-ONS
    * [
      GitHub Advanced SecurityEnterprise-grade security features
      ][60]
    * [
      Copilot for BusinessEnterprise-grade AI features
      ][61]
    * [
      Premium SupportEnterprise-grade 24/7 support
      ][62]
* [Pricing][63]
Search or jump to...

# Search code, repositories, users, issues, pull requests...

Search
Clear
[Search syntax tips][64]

# Provide feedback

We read every piece of feedback, and take your input very seriously.

Include my email address so I can be contacted
Cancel Submit feedback

# Saved searches

## Use saved searches to filter your results more quickly

Name
Query

To see all available qualifiers, see our [documentation][65].

Cancel Create saved search
[ Sign in ][66]
[ Sign up ][67]
Appearance settings
Resetting focus
You signed in with another tab or window. [Reload][68] to refresh your session. You signed out in another tab or window.
[Reload][69] to refresh your session. You switched accounts on another tab or window. [Reload][70] to refresh your
session. Dismiss alert

### Uh oh!


There was an error while loading. [Please reload this page][71].

[ commonsense ][72] / ** [conceptnet5][73] ** Public
* [ Notifications ][74] You must be signed in to change notification settings
* [ Fork 357 ][75]
* [ Star 2.9k ][76]
* [ Code ][77]
* [ Issues 41 ][78]
* [ Pull requests 4 ][79]
* [ Discussions ][80]
* [ Actions ][81]
* [ Projects ][82]
* [ Wiki ][83]
* [ Security and quality 0 ][84]
* [ Insights ][85]
Additional navigation options
* [ Code ][86]
* [ Issues ][87]
* [ Pull requests ][88]
* [ Discussions ][89]
* [ Actions ][90]
* [ Projects ][91]
* [ Wiki ][92]
* [ Security and quality ][93]
* [ Insights ][94]

# FAQ

[Jump to bottom][95]
Robyn Speer edited this page May 21, 2019 · [24 revisions][96]

Here are answers to some frequently-asked questions, updated for ConceptNet 5.7.

## The basics

### What is ConceptNet?

ConceptNet is a knowledge graph of things people know and computers should know, expressed in various natural languages.
See the [main page][97] for more details.

### Is ConceptNet an AI? Can I talk to it?

ConceptNet is a resource. You can use it as part of making an AI that understands the meanings of words people use.

ConceptNet is not a chatbot. Some chatbot systems have used ConceptNet as a resource, but this is not a primary use case
that ConceptNet is designed for.

### How can I see what ConceptNet knows?

You can browse the knowledge graph at [http://www.conceptnet.io/][98].

### How do I use ConceptNet in my own code?

We recommend starting with the Web [API][99]. If you need a greater flow of information than the Web API provides, then
consider [downloading][100] the data.

One way to take advantage of all the information in ConceptNet, as well as information that can be learned from large
corpora of text, is to use the [ConceptNet Numberbatch][101] word embeddings. These can be used as a more accurate
replacement for word2vec or GloVe vectors.

When used together with some extra code in `conceptnet5.vectors`, ConceptNet Numberbatch provides the best word
embeddings in the world in multiple languages, as tested at SemEval 2017.

## Citing ConceptNet

### Which paper should I cite?

The paper we recommend citing when you're using recent versions of ConceptNet is:

> Robyn Speer, Joshua Chin, and Catherine Havasi. 2017. "[ConceptNet 5.5: An Open Multilingual Graph of General
> Knowledge][102]." In proceedings of AAAI 31.

It's okay to cite this paper for versions later than 5.5. We don't get to publish a new paper for every version.

The BibTeX information is:

`@paper{speer2017conceptnet,
    author = {Robyn Speer and Joshua Chin and Catherine Havasi},
    title = {ConceptNet 5.5: An Open Multilingual Graph of General Knowledge},
    conference = {AAAI Conference on Artificial Intelligence},
    year = {2017},
    pages = {4444--4451},
    keywords = {ConceptNet; knowledge graph; word embeddings},
    url = {http://aaai.org/ocs/index.php/AAAI/AAAI17/paper/view/14972}
}
`

If you want to cite a more general, older overview:

> Robyn Speer and Catherine Havasi, 2012. [Representing General Relational Knowledge in ConceptNet 5][103]. In *LREC*
> (pp. 3679-3686).

ConceptNet has changed a lot over its existence. If you cite a paper by Hugo Liu (one of the original creators of
ConceptNet), realize that the citation only applies to the design of ConceptNet from 2005 and earlier.

### I'm seeing citations of ConceptNet that cite a different R. Speer -- why should I not use those?

I'm Robyn Speer. The other name is my "deadname", a former name that I don't use for any purposes and don't want to
propagate, because it doesn't fit my gender identity.

For me to continue in research as a trans woman, I need to be able to choose my name and keep my publication history.
I've amended many of my recent papers to have my new name on them. No matter whether you're seeing the amended version,
please always cite me as Robyn Speer.

See [Citation complications][104] for further details on this, including what to do if you've accidentally created a new
citation of my deadname.

## Building on ConceptNet

### Can I release a project that uses ConceptNet as part of it?

Yes! This is allowed by the Creative Commons Attribution-ShareAlike license, which has two conditions. Here's what they
approximately mean for ConceptNet:
* **Attribution**: Visibly give credit to ConceptNet and its creators
* **ShareAlike**: If you add data to ConceptNet, modify its data, or combine its data into a larger database, the
  resulting dataset must have the same license terms as ConceptNet.

To give proper attribution to ConceptNet's data, we suggest this text:

> This work includes data from ConceptNet 5, which was compiled by the Commonsense Computing Initiative. ConceptNet 5 is
> freely available under the Creative Commons Attribution-ShareAlike license (CC BY SA 4.0) from
> [http://conceptnet.io][105]. The included data was created by contributors to Commonsense Computing projects,
> contributors to Wikimedia projects, Games with a Purpose, Princeton University's WordNet, DBPedia, OpenCyc, and Umbel.

In particular, you **may not** add restrictions on how data built on ConceptNet is used, such as "research purposes
only" or "non-commercial".

### I need to use ConceptNet together with a "research purposes only" resource. I really am just using it for research
### purposes. What do I do?

You can't change ConceptNet's license, not even for the sake of research. I can't change it either, even if I wanted to,
because I've agreed to the same license from Wikimedia. But I wouldn't want to change it. The Attribution-ShareAlike
license makes sure that ConceptNet remains open data.

Some options you have are:
* Try to get a more permissive license from the creators of the other resource
* Find a different resource
* Put either ConceptNet or the other resource in a separate component, whose data is distributed separately

### But I want to make something for ordinary people, not corporations! Why do I have to allow commercial use?

ConceptNet would not exist without commercial use.

Large corporations will get all the data they want anyway. When you put restrictions on data, you don't do anything to
large corporations, you only harm people without connections. "Research use only" or "academic use only" is a
particularly insidious form of elitism.

## Using the API

### How do I get started using the ConceptNet API?

We went to some effort to make the API responses look nice in a Web browser. The JSON gets formatted and highlighted,
and values that are references to other URLs you can look up become links, so you can just explore by following these
links.

Try clicking the link below and you'll be using the ConceptNet API:

[http://api.conceptnet.io/c/en/example][106]

Of course you don't have to be a Web browser. If you have `curl` (a small command-line HTTP utility) on your computer,
try running this at the command line:

`curl http://api.conceptnet.io/c/en/example
`

Or in Python, using the `requests` library:

`import requests
requests.get('http://api.conceptnet.io/c/en/example').json()
`

There are more things you can do that won't be quite so obvious just from looking at the responses, so once you've
explored a little, go read the [API][107] documentation.

### The API returns fewer results than I saw on the Web interface. Where are the rest of the results?

There are more pages of results. The default page size is set to 20 -- this speeds up the responses, and makes sure you
*notice* that there aren't many results.

When the API results are paginated, the response will end with a section that looks like this:

`  "view": {
    "@id": "/c/en/example?offset=0&limit=20",
    "@type": "PartialCollectionView",
    "comment": "There are more results. Follow the 'nextPage' link for more.",
    "firstPage": "/c/en/example?offset=0&limit=20",
    "nextPage": "/c/en/example?offset=20&limit=20",
    "paginatedProperty": "edges"
  }
`

As the comment states, "nextPage" contains a link to the next page of results. If you're viewing the API response in a
Web browser, you can click the link to see more results.

### I queried the API and got a bunch of HTML formatting. How do I just get the JSON?

We were trying to only send you the formatted HTML if it looked like you were using a Web browser, but maybe we're
wrong, and maybe you just want the plain JSON anyway. Add `?format=json` to the URL that you query. For example:

[http://api.conceptnet.io/c/en/example?format=json][108]

Try going to that URL in Firefox, which has its own built-in JSON formatter. It won't give you a way to follow the
links, but other than that, it's pretty nice.

### What format are these API responses in?

[JSON-LD][109], a linked data format that on the surface is just reasonable-looking JSON, and under the hood, preserves
some of the good parts of RDF and the Semantic Web.

## Comparisons to other projects

### How does ConceptNet compare to WordNet?

This is an interesting comparison to make, as the projects have similar goals, and by now they both make use of
multilingual linked data.

ConceptNet contains more kinds of relationships than WordNet. ConceptNet's vocabulary is larger and interconnected in
many more ways. In exchange, it's somewhat messier than WordNet.

ConceptNet does only the bare minimum to distinguish word senses so far -- in the built graph of ConceptNet 5.5, word
senses are only distinguished by their part of speech (similar to sense2vec). WordNet has a large number of senses for
every word, though some of them are difficult to distinguish in practice.

WordNet is too sparse for some applications. You can't build word vectors from WordNet alone. You can't compare nouns to
verbs in WordNet, because they are mostly unconnected vocabularies.

ConceptNet does not assume that words fall into "synsets", sets of synonyms that are completely interchangeable.
Synonymy in ConceptNet is a relation like any other. If you've worked with WordNet, you may have been frustrated by the
implications of the synset assumption on real text, where words are not marked with specific senses, and where the word
"He" cannot usually be replaced synonymously with "atomic number 2".

In ConceptNet, we incorporate as much of WordNet as we can while undoing the synset assumption, and we give it a high
weight, because the information in WordNet is valuable and usually quite accurate.

### How does ConceptNet compare to the Google Knowledge Graph?

ConceptNet is linked open data, and that makes it fundamentally a different thing than a proprietary knowledge base.

Google's Knowledge Graph is a brand name on top of the structured knowledge that it takes to run the Google search
engine, Google Assistant, and probably other applications. It provides those sidebars of facts you get when you search
for things on Google, and it provides answers to questions that you ask the Google Assistant. It seems to focus largely
on things you can buy and things you can look up on Wikipedia. (In ConceptNet, we focus more on the general meanings of
all words, whether they be nouns, verbs, adjectives, or adverbs, and less on named entities.)

I assume it's a very well-designed knowledge representation for a search engine. And there is only one search engine
that it can power. Fundamentally, the Google Knowledge Graph supports the ability to interact with Google products on
Google's terms.

Unlike the typical corporate knowledge base, ConceptNet has remained true to its crowdsourcing roots. While it's a
project developed at Luminoso, it is open for anyone to use under a Creative Commons license. This is the fair thing to
do, given how much of it depends on public contributions and linked data, but it's also part of Luminoso's ideals. When
we let you see and use our state-of-the-art knowledge representation first-hand, it promotes understanding of why
Luminoso's products are a better approach to NLP.

### How does ConceptNet compare to BabelNet?

BabelNet is very similar in structure to ConceptNet, but very different in openness.

BabelNet uses many of the same knowledge sources as ConceptNet. It lacks the Open Mind Common Sense and Games with a
Purpose data, which provide ConceptNet with a wide range of noisy but effective relational knowledge. It does, on the
other hand, have a representation of WordNet-style word senses that ConceptNet doesn't have.

As of 2018, BabelNet is proprietary and not available to the public. You may find this surprising given how they've
touted their openness in the past, and given that it's built on Creative Commons Share-Alike resources, but check their
site. You won't find a download link.

They allow you to submit an application to use it for research purposes only, if you meet the requirements of having
academic credentials and a current academic affiliation.

### How does ConceptNet compare to DBPedia?

DBPedia is very much focused on named entities. It's messier than ConceptNet. Its vocabulary consists only of titles of
Wikipedia articles.

DBPedia contains information that can be used for answering specific questions, such as "Where is the birthplace of John
Adams?" or "What countries have a population of over 10 million?". It particularly knows a lot about locations, movies,
and music albums. You could use DBPedia to solve Six Degrees of Kevin Bacon.

ConceptNet imports a small amount of DBPedia, and also contains external links to DBPedia and Wikidata.

### How does ConceptNet compare to DBnary?

DBnary is a counterpart to DBPedia that's actually quite compatible with ConceptNet. Like ConceptNet, it focuses on word
definitions rather than named entities, and it gets them from parsing Wiktionary.

Right now we use our own Wiktionary parser, which covers fewer Wiktionary sites than DBnary does but extracts more
detail from each entry. We would gladly use DBnary instead, if DBnary starts extracting information such as links from
definitions.

### How does ConceptNet compare to (Open)Cyc?

Cyc was an ontology built on a predicate logic representation called CycL. CycL enabled very precise reasoning in a way
that machine learning over ConceptNet doesn't. However, Cyc was intolerant of errors, and adding information to Cyc was
a difficult task that kept Cycorp occupied for over 30 years.

OpenCyc provides a hierarchy of types of things, with English names, some of which are automatically generated. It seems
to be intended as a preview of the full Cyc system, a proprietary system that was shut down in 2017.

ConceptNet includes a subset of OpenCyc, consisting of the *IsA* statements that can be reasonably represented in
natural language.

### How does ConceptNet compare to the Microsoft Concept Graph?

The Microsoft Concept Graph is a proprietary taxonomy of English nouns, connected with the "IsA" relation, with some
automatic word sense disambiguation. Its data comes from machine reading of a Web search index. It resembles an
automatically-generated version of OpenCyc, and is derived from an earlier project named Probase.

The Microsoft Concept Graph was shut down in 2018.

## Knowledge representation

### How many statements (edges) are there in ConceptNet?

Approximately 34 million.

### Does ConceptNet use logical predicates?

No. Its representation is words and phrases of natural language, and relations between them. Natural language can be
vague, illogical, and incredibly useful.

### How many languages is ConceptNet in?

The data that ConceptNet is built from spans a lot of different languages, with a long tail of marginally-represented
languages. 10 languages have core support, 77 languages have moderate support, and 304 languages are supported in total.
See [Languages][110] for a complete list.

### ConceptNet is missing facts.

This will always be true. We use machine-learning techniques, including word embeddings, to learn generalizable things
from ConceptNet despite the incompleteness of the knowledge it contains.

### ConceptNet contains false information.

There will probably always be isolated mistakes or falsehoods in ConceptNet. Our data sources and our processes are not
perfect. Machine learning can be relatively robust against errors, as long as the errors are not systematic.

If you've identified a *systematic* source of errors in ConceptNet, that is more important. It would probably improve
ConceptNet to get rid of it. In that case, please go to the 'Issues' tab and describe it in an issue report.

### What are the relations represented in ConceptNet? What do they mean?

See the table on the [Relations][111] page of this wiki.

### Where do the edge weights in ConceptNet come from?

Made-up numbers that are programmed into the [reader][112] modules that import various sources of knowledge. These
weights represent a rough heuristic of which statements you should trust more than other statements.

### Can I add new information to ConceptNet?

During the golden age of crowdsourcing (the decade of the 2000s), ConceptNet accepted direct contributions of knowledge.
This was a great start, but now the opportunities for improving ConceptNet have changed, and we are content to leave
crowdsourcing to the organizations that are really good at it, like the Wikimedia Foundation.

If you contribute to [Wiktionary][113] and follow their guidelines, the information you contribute will eventually be
represented in ConceptNet.

### What I mean is, can I make my own version of ConceptNet that includes information that I need in my domain?

Well, you can reproduce ConceptNet's [build process][114] and change the code to import a new source of data. This may
or may not accomplish what you want.

What ConceptNet is designed for is representing general knowledge. Making a useful domain-specific semantic model is a
rather different process, in our experience. The software we built on top of ConceptNet to make this possible eventually
became our company, [Luminoso][115]. Luminoso provides software as a service that creates domain-specific semantic
models, which make use of ConceptNet so they can start out knowing what words mean and just have to learn what's
different in your domain.

## Technologies

### What kind of database does ConceptNet use?

We've tried a lot of them. Currently PostgreSQL.

### Why not a graph database? Why not [insert new database name here]?

Probably one of the following reasons:
* It isn't as efficient as PostgreSQL
* It doesn't actually work as advertised
* It is no longer maintained
* It doesn't provide a good workflow for importing a medium-sized graph such as ConceptNet
* It takes more than a day to import a medium-sized graph such as ConceptNet
* It inflates the size of the data it stores by a factor of more than 10
* It assumes every user has access to and wants to use a distributed computing cluster
* It would be hard for people who want their own copy of ConceptNet to install it
* It's not free software
* It has a restriction on it that would prevent people from reusing ConceptNet, such as the GPL or "academic use only"

If you think you know of a database that doesn't fail one of these criteria, I'd still be interested to hear about it.

### Is ConceptNet "big data"?

It fits on a hard disk, so no. It's *enough* data for many purposes. But text is small.

If you have textual knowledge that actually requires distributed computation, you work at a company that does Web
search.

### Is there a graph visualization of ConceptNet?

You're asking about a visualization [like this][116], right?

Notice that that graph is a few thousand times smaller than ConceptNet and it's already an incomprehensible
rainbow-colored hairball. I am not convinced there's a technology that exists that can put all of ConceptNet in one
meaningful image, although there may be an approach that involves spreading it out into local clusters using t-SNE.

It will almost certainly involve custom code -- ConceptNet makes off-the-shelf graph visualizers collapse under the
insoluble problem of laying out its edges. I'm interested in making such a visualization, but the result has to be
informative, not just a hairball.

### Can ConceptNet be queried using SPARQL?

No. SPARQL is computationally infeasible. Similar projects that use SPARQL have unacceptable latency and go down
whenever anyone starts using them in earnest.

The way to query ConceptNet is using a rather straightforward REST API, described on the [API][117] page. If you need to
make a form of query that this API doesn't support, open an issue and we'll look into supporting it.

## AI hype

### I heard that ConceptNet has the intelligence of a 4-year-old, is this true?

Blame science reporting for doing what it usually does. There's a nugget of truth in there surrounded by a big wad of
meaningless AI hype. It's true that ConceptNet 4 could compete with 4-year-olds on a particular question-answering task
-- and ConceptNet 5 performs much better on a similar task. This is cool. It doesn't mean that anyone's about to make
[robot children][118].

Here's the background: A much older version of ConceptNet, ConceptNet 4, was [evaluated on some intelligence tests][119]
involving question-answering and sentence comprehension. The researchers who performed these tests compared ConceptNet's
performance to a 4-year-old child.

We found the comparison odd but flattering. 4-year-old children are incredible beings. They have desires, goals, and
imagination, and they can communicate them in their spoken language with a level of competence that second-language
learners have to put tremendous effort into achieving. No real AI system can come close to emulating the range of things
a child can do.

When it comes to the narrower task of answering questions, though, it's believable that ConceptNet 4 compared to a
4-year-old. We're always interested in measurably improving the general intelligence contained in ConceptNet.
Excitingly, we now have a question-answering task in which ConceptNet 5 compares to a 17-year-old: that of answering
SAT-style analogy questions.

The [Story Cloze Test][120] is a test of story understanding that any human can score close to 100% on in their native
language. ConceptNet is used in state-of-the-art systems that solve this task. See this paper by [Jiaao Chen et
al][121].

## Wiki pages Pages 17
* Loading [
  Home
  ][122]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][123].
* Loading [
  API
  ][124]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][125].
* Loading [
  Build process
  ][126]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][127].
* Loading [
  Changelog
  ][128]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][129].
* Loading [
  Citation complications
  ][130]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][131].
* Loading [
  Copying and sharing ConceptNet
  ][132]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][133].
* Loading [
  Docker
  ][134]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][135].
* Loading [
  Downloads
  ][136]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][137].
* Loading [
  Edges
  ][138]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][139].
* Loading [
  FAQ
  ][140]
  * [The basics][141]
  * [What is ConceptNet?][142]
  * [Is ConceptNet an AI? Can I talk to it?][143]
  * [How can I see what ConceptNet knows?][144]
  * [How do I use ConceptNet in my own code?][145]
  * [Citing ConceptNet][146]
  * [Which paper should I cite?][147]
  * [I'm seeing citations of ConceptNet that cite a different R. Speer -- why should I not use those?][148]
  * [Building on ConceptNet][149]
  * [Can I release a project that uses ConceptNet as part of it?][150]
  * [I need to use ConceptNet together with a "research purposes only" resource. I really am just using it for research
    purposes. What do I do?][151]
  * [But I want to make something for ordinary people, not corporations! Why do I have to allow commercial use?][152]
  * [Using the API][153]
  * [How do I get started using the ConceptNet API?][154]
  * [The API returns fewer results than I saw on the Web interface. Where are the rest of the results?][155]
  * [I queried the API and got a bunch of HTML formatting. How do I just get the JSON?][156]
  * [What format are these API responses in?][157]
  * [Comparisons to other projects][158]
  * [How does ConceptNet compare to WordNet?][159]
  * [How does ConceptNet compare to the Google Knowledge Graph?][160]
  * [How does ConceptNet compare to BabelNet?][161]
  * [How does ConceptNet compare to DBPedia?][162]
  * [How does ConceptNet compare to DBnary?][163]
  * [How does ConceptNet compare to (Open)Cyc?][164]
  * [How does ConceptNet compare to the Microsoft Concept Graph?][165]
  * [Knowledge representation][166]
  * [How many statements (edges) are there in ConceptNet?][167]
  * [Does ConceptNet use logical predicates?][168]
  * [How many languages is ConceptNet in?][169]
  * [ConceptNet is missing facts.][170]
  * [ConceptNet contains false information.][171]
  * [What are the relations represented in ConceptNet? What do they mean?][172]
  * [Where do the edge weights in ConceptNet come from?][173]
  * [Can I add new information to ConceptNet?][174]
  * [What I mean is, can I make my own version of ConceptNet that includes information that I need in my domain?][175]
  * [Technologies][176]
  * [What kind of database does ConceptNet use?][177]
  * [Why not a graph database? Why not [insert new database name here]?][178]
  * [Is ConceptNet "big data"?][179]
  * [Is there a graph visualization of ConceptNet?][180]
  * [Can ConceptNet be queried using SPARQL?][181]
  * [AI hype][182]
  * [I heard that ConceptNet has the intelligence of a 4-year-old, is this true?][183]
* Loading [
  Graph structure
  ][184]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][185].
* Loading [
  JSON streams
  ][186]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][187].
* Loading [
  Knowledge sources
  ][188]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][189].
* Loading [
  Languages
  ][190]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][191].
* Loading [
  Relations
  ][192]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][193].
* Loading [
  Running your own copy
  ][194]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][195].
* Loading [
  URI hierarchy
  ][196]
  
  ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][197].
* Show 2 more pages…

**Starting points**
* **[FAQ][198]**
* Web [API][199]
* [Downloads][200]

**Reproducibility**
* [Copying and sharing][201]
* [Build process][202]
* [Running your own copy][203]

**Details**
* [Edges][204]
* [Relations][205]
* [Languages][206]
* [URI hierarchy][207]

### Clone this wiki locally

## Footer

© 2026 GitHub, Inc.

### Footer navigation
* [Terms][208]
* [Privacy][209]
* [Security][210]
* [Status][211]
* [Community][212]
* [Docs][213]
* [Contact][214]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Fcommonsense%2Fconceptnet5%2Fwiki%2FFAQ
[3]: https://github.com/features/copilot
[4]: https://github.com/features/ai/github-app
[5]: https://github.com/mcp
[6]: https://github.com/features/actions
[7]: https://github.com/features/codespaces
[8]: https://github.com/features/issues
[9]: https://github.com/features/code-review
[10]: https://github.com/security/advanced-security
[11]: https://github.com/security/advanced-security/code-security
[12]: https://github.com/security/advanced-security/secret-protection
[13]: https://github.com/why-github
[14]: https://docs.github.com
[15]: https://github.blog
[16]: https://github.blog/changelog
[17]: https://github.com/marketplace
[18]: https://github.com/features
[19]: https://github.com/enterprise
[20]: https://github.com/team
[21]: https://github.com/enterprise/startups
[22]: https://github.com/solutions/industry/nonprofits
[23]: https://github.com/solutions/use-case/app-modernization
[24]: https://github.com/solutions/use-case/devsecops
[25]: https://github.com/solutions/use-case/devops
[26]: https://github.com/solutions/use-case/ci-cd
[27]: https://github.com/solutions/use-case
[28]: https://github.com/solutions/industry/healthcare
[29]: https://github.com/solutions/industry/financial-services
[30]: https://github.com/solutions/industry/manufacturing
[31]: https://github.com/solutions/industry/government
[32]: https://github.com/solutions/industry
[33]: https://github.com/solutions
[34]: https://github.com/resources/articles?topic=ai
[35]: https://github.com/resources/articles?topic=software-development
[36]: https://github.com/resources/articles?topic=devops
[37]: https://github.com/resources/articles?topic=security
[38]: https://github.com/resources/articles
[39]: https://github.com/customer-stories
[40]: https://github.com/resources/events
[41]: https://github.com/resources/whitepapers
[42]: https://github.com/solutions/executive-insights
[43]: https://skills.github.com
[44]: https://docs.github.com
[45]: https://support.github.com
[46]: https://github.com/orgs/community/discussions
[47]: https://github.com/trust-center
[48]: https://github.com/partners
[49]: https://github.com/resources
[50]: https://github.com/sponsors
[51]: https://securitylab.github.com
[52]: https://maintainers.github.com
[53]: https://github.com/accelerator
[54]: https://stars.github.com
[55]: https://archiveprogram.github.com
[56]: https://github.com/topics
[57]: https://github.com/trending
[58]: https://github.com/collections
[59]: https://github.com/enterprise
[60]: https://github.com/security/advanced-security
[61]: https://github.com/features/copilot/copilot-business
[62]: https://github.com/premium-support
[63]: https://github.com/pricing
[64]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[65]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Fcommonsense%2Fconceptnet5%2Fwiki%2FFAQ
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fwiki%2Fshow&sour
ce=header-repo&source_repo=commonsense%2Fconceptnet5
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /commonsense
[73]: /commonsense/conceptnet5
[74]: /login?return_to=%2Fcommonsense%2Fconceptnet5
[75]: /login?return_to=%2Fcommonsense%2Fconceptnet5
[76]: /login?return_to=%2Fcommonsense%2Fconceptnet5
[77]: /commonsense/conceptnet5
[78]: /commonsense/conceptnet5/issues
[79]: /commonsense/conceptnet5/pulls
[80]: /commonsense/conceptnet5/discussions
[81]: /commonsense/conceptnet5/actions
[82]: /commonsense/conceptnet5/projects
[83]: /commonsense/conceptnet5/wiki
[84]: /commonsense/conceptnet5/security
[85]: /commonsense/conceptnet5/pulse
[86]: /commonsense/conceptnet5
[87]: /commonsense/conceptnet5/issues
[88]: /commonsense/conceptnet5/pulls
[89]: /commonsense/conceptnet5/discussions
[90]: /commonsense/conceptnet5/actions
[91]: /commonsense/conceptnet5/projects
[92]: /commonsense/conceptnet5/wiki
[93]: /commonsense/conceptnet5/security
[94]: /commonsense/conceptnet5/pulse
[95]: #wiki-pages-box
[96]: /commonsense/conceptnet5/wiki/FAQ/_history
[97]: http://www.conceptnet.io/
[98]: http://www.conceptnet.io/
[99]: /commonsense/conceptnet5/wiki/API
[100]: /commonsense/conceptnet5/wiki/Downloads
[101]: https://github.com/commonsense/conceptnet-numberbatch
[102]: https://arxiv.org/abs/1612.03975
[103]: https://www.aclweb.org/anthology/papers/L/L12/L12-1639/
[104]: /commonsense/conceptnet5/wiki/Citation-complications
[105]: http://conceptnet.io
[106]: http://api.conceptnet.io/c/en/example
[107]: /commonsense/conceptnet5/wiki/API
[108]: http://api.conceptnet.io/c/en/example?format=json
[109]: https://json-ld.org/
[110]: /commonsense/conceptnet5/wiki/Languages
[111]: /commonsense/conceptnet5/wiki/Relations
[112]: https://github.com/commonsense/conceptnet5/tree/master/conceptnet5/readers
[113]: https://en.wiktionary.org/
[114]: /commonsense/conceptnet5/wiki/Build-process
[115]: http://www.luminoso.com
[116]: https://commons.wikimedia.org/wiki/File:SocialNetworkAnalysis.png
[117]: /commonsense/conceptnet5/wiki/API
[118]: http://www.imdb.com/title/tt0212720/
[119]: http://www.techtimes.com/articles/93030/20151009/mit-artificial-intelligence-program-has-iq-of-four-year-old-chil
d.htm
[120]: http://cs.rochester.edu/nlp/rocstories/
[121]: https://arxiv.org/pdf/1811.00625.pdf
[122]: /commonsense/conceptnet5/wiki
[123]: 
[124]: /commonsense/conceptnet5/wiki/API
[125]: 
[126]: /commonsense/conceptnet5/wiki/Build-process
[127]: 
[128]: /commonsense/conceptnet5/wiki/Changelog
[129]: 
[130]: /commonsense/conceptnet5/wiki/Citation-complications
[131]: 
[132]: /commonsense/conceptnet5/wiki/Copying-and-sharing-ConceptNet
[133]: 
[134]: /commonsense/conceptnet5/wiki/Docker
[135]: 
[136]: /commonsense/conceptnet5/wiki/Downloads
[137]: 
[138]: /commonsense/conceptnet5/wiki/Edges
[139]: 
[140]: /commonsense/conceptnet5/wiki/FAQ
[141]: /commonsense/conceptnet5/wiki/FAQ#the-basics
[142]: /commonsense/conceptnet5/wiki/FAQ#what-is-conceptnet
[143]: /commonsense/conceptnet5/wiki/FAQ#is-conceptnet-an-ai-can-i-talk-to-it
[144]: /commonsense/conceptnet5/wiki/FAQ#how-can-i-see-what-conceptnet-knows
[145]: /commonsense/conceptnet5/wiki/FAQ#how-do-i-use-conceptnet-in-my-own-code
[146]: /commonsense/conceptnet5/wiki/FAQ#citing-conceptnet
[147]: /commonsense/conceptnet5/wiki/FAQ#which-paper-should-i-cite
[148]: /commonsense/conceptnet5/wiki/FAQ#im-seeing-citations-of-conceptnet-that-cite-a-different-r-speer----why-should-i
-not-use-those
[149]: /commonsense/conceptnet5/wiki/FAQ#building-on-conceptnet
[150]: /commonsense/conceptnet5/wiki/FAQ#can-i-release-a-project-that-uses-conceptnet-as-part-of-it
[151]: /commonsense/conceptnet5/wiki/FAQ#i-need-to-use-conceptnet-together-with-a-research-purposes-only-resource-i-real
ly-am-just-using-it-for-research-purposes-what-do-i-do
[152]: /commonsense/conceptnet5/wiki/FAQ#but-i-want-to-make-something-for-ordinary-people-not-corporations-why-do-i-have
-to-allow-commercial-use
[153]: /commonsense/conceptnet5/wiki/FAQ#using-the-api
[154]: /commonsense/conceptnet5/wiki/FAQ#how-do-i-get-started-using-the-conceptnet-api
[155]: /commonsense/conceptnet5/wiki/FAQ#the-api-returns-fewer-results-than-i-saw-on-the-web-interface-where-are-the-res
t-of-the-results
[156]: /commonsense/conceptnet5/wiki/FAQ#i-queried-the-api-and-got-a-bunch-of-html-formatting-how-do-i-just-get-the-json
[157]: /commonsense/conceptnet5/wiki/FAQ#what-format-are-these-api-responses-in
[158]: /commonsense/conceptnet5/wiki/FAQ#comparisons-to-other-projects
[159]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-wordnet
[160]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-the-google-knowledge-graph
[161]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-babelnet
[162]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-dbpedia
[163]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-dbnary
[164]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-opencyc
[165]: /commonsense/conceptnet5/wiki/FAQ#how-does-conceptnet-compare-to-the-microsoft-concept-graph
[166]: /commonsense/conceptnet5/wiki/FAQ#knowledge-representation
[167]: /commonsense/conceptnet5/wiki/FAQ#how-many-statements-edges-are-there-in-conceptnet
[168]: /commonsense/conceptnet5/wiki/FAQ#does-conceptnet-use-logical-predicates
[169]: /commonsense/conceptnet5/wiki/FAQ#how-many-languages-is-conceptnet-in
[170]: /commonsense/conceptnet5/wiki/FAQ#conceptnet-is-missing-facts
[171]: /commonsense/conceptnet5/wiki/FAQ#conceptnet-contains-false-information
[172]: /commonsense/conceptnet5/wiki/FAQ#what-are-the-relations-represented-in-conceptnet-what-do-they-mean
[173]: /commonsense/conceptnet5/wiki/FAQ#where-do-the-edge-weights-in-conceptnet-come-from
[174]: /commonsense/conceptnet5/wiki/FAQ#can-i-add-new-information-to-conceptnet
[175]: /commonsense/conceptnet5/wiki/FAQ#what-i-mean-is-can-i-make-my-own-version-of-conceptnet-that-includes-informatio
n-that-i-need-in-my-domain
[176]: /commonsense/conceptnet5/wiki/FAQ#technologies
[177]: /commonsense/conceptnet5/wiki/FAQ#what-kind-of-database-does-conceptnet-use
[178]: /commonsense/conceptnet5/wiki/FAQ#why-not-a-graph-database-why-not-insert-new-database-name-here
[179]: /commonsense/conceptnet5/wiki/FAQ#is-conceptnet-big-data
[180]: /commonsense/conceptnet5/wiki/FAQ#is-there-a-graph-visualization-of-conceptnet
[181]: /commonsense/conceptnet5/wiki/FAQ#can-conceptnet-be-queried-using-sparql
[182]: /commonsense/conceptnet5/wiki/FAQ#ai-hype
[183]: /commonsense/conceptnet5/wiki/FAQ#i-heard-that-conceptnet-has-the-intelligence-of-a-4-year-old-is-this-true
[184]: /commonsense/conceptnet5/wiki/Graph-structure
[185]: 
[186]: /commonsense/conceptnet5/wiki/JSON-streams
[187]: 
[188]: /commonsense/conceptnet5/wiki/Knowledge-sources
[189]: 
[190]: /commonsense/conceptnet5/wiki/Languages
[191]: 
[192]: /commonsense/conceptnet5/wiki/Relations
[193]: 
[194]: /commonsense/conceptnet5/wiki/Running-your-own-copy
[195]: 
[196]: /commonsense/conceptnet5/wiki/URI-hierarchy
[197]: 
[198]: /commonsense/conceptnet5/wiki/FAQ
[199]: /commonsense/conceptnet5/wiki/API
[200]: /commonsense/conceptnet5/wiki/Downloads
[201]: /commonsense/conceptnet5/wiki/Copying-and-sharing-ConceptNet
[202]: /commonsense/conceptnet5/wiki/Build-process
[203]: /commonsense/conceptnet5/wiki/Running-your-own-copy
[204]: /commonsense/conceptnet5/wiki/Edges
[205]: /commonsense/conceptnet5/wiki/Relations
[206]: /commonsense/conceptnet5/wiki/Languages
[207]: /commonsense/conceptnet5/wiki/URI-hierarchy
[208]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[209]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[210]: https://github.com/security
[211]: https://www.githubstatus.com/
[212]: https://github.community/
[213]: https://docs.github.com/
[214]: https://support.github.com?tags=dotcom-footer
```
