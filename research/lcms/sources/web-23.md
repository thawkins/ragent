# Web source

- URL: https://link.springer.com/article/10.1007/s12559-024-10345-6
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-29T16:30:10.900279026+00:00

```text
[Skip to main content][1]

Advertisement

[ [Advertisement] ][2]
[ [Springer Nature Link] ][3]
[Log in][4]
[ Menu ][5]
[ Find a journal ][6] [ Publish with us ][7] [ Track your research ][8]
[ Search ][9]
[ Saved research ][10]
[ Cart ][11]
1. [Home][12]
2. [Cognitive Computation][13]
3. Article

# PrimeNet: A Framework for Commonsense Knowledge Representation and Reasoning Based on Conceptual Primitives
* Research
* Published: 30 August 2024
* Volume 16, pages 3429–3456 (2024)
* [Cite this article][14]

[ Save article ][15]
[ View saved research ][16]
[ Cognitive Computation ][17] [ Aims and scope ][18] [ Submit manuscript ][19]
* [Qian Liu][20]^{[1][21]},
* [Sooji Han][22]^{[2][23]},
* [Erik Cambria][24]^{[3][25]},
* [Yang Li][26]^{[4][27]} &
* …
* [Kenneth Kwok][28]^{[5][29]} 
Show authors
* 656 Accesses
* 20 Citations
* [Explore all metrics ][30]

## Abstract

Commonsense knowledge acquisition and representation is a core topic in artificial intelligence (AI), which is crucial
for building more sophisticated and human-like AI systems. However, existing commonsense knowledge bases organize facts
in an isolated manner like bag of facts, lacking the cognitive-level connections that humans commonly possess. People
have the ability to efficiently organize vast amounts of knowledge by linking or generalizing concepts using a limited
set of conceptual primitives that serve as the fundamental building blocks of reasoning. These conceptual primitives are
basic, foundational elements of thought that humans use to make sense of the world. By combining and recombining these
primitives, people can construct complex ideas, solve problems, and understand new concepts. To emulate this cognitive
mechanism, we design a new commonsense knowledge base, termed PrimeNet, organized in a three-layer structure: a small
core of conceptual primitives (e.g., FOOD), a bigger set of concepts that connect to such primitives (e.g., fruit), and
an even larger layer of entities connecting to the concepts (e.g., banana). First, we collect commonsense knowledge and
employ a gradual expansion strategy for knowledge integration. After refinement, PrimeNet contains 6 million edges
between 2 million nodes, with 34 different types of relations. Then, we design a new conceptualization method by
leveraging a probabilistic taxonomy, to build the concept layer of PrimeNet. Finally, we conduct primitive detection to
build the primitive layer, where a lexical substitution task is used to identify related concepts, and large language
models are employed to generate a rational primitive to label each concept cluster as well as verify the primitive
detection process.

This is a preview of subscription content, [log in via an institution][31] to check access.

## Access this article

[ Log in via an institution ][32]

## Subscribe and save

Springer+
from €37.37 /Month
* Starting from 10 chapters or articles per month
* Access and download chapters and articles from more than 300k books and 2,500 journals
* Cancel anytime
[View plans ][33]

## Buy Now

Buy article PDF 39,95 €

Price includes VAT (Thailand)

Instant access to the full article PDF.

[Institutional subscriptions ][34]

**Fig. 1**
**Fig. 2**
**Fig. 3**
**Fig. 4**
**Fig. 5**
**Fig. 6**
**Fig. 7**
**Fig. 8**
**Fig. 9**
**Fig. 10**
**Fig. 11**

### Similar content being viewed by others

### [Knowledge Representation & Reasoning ][35]

Chapter © 2025

### [Accelerating primer design for amplicon sequencing using large language model-powered agents ][36]

Article 30 July 2025

### [DeepPrimitive: Image decomposition by layered primitive detection ][37]

Article Open access 23 December 2018

### Explore related subjects

Discover the latest articles, books and news in related subjects, suggested using machine learning.
* [Computational Intelligence][38]
* [ELISPOT][39]
* [Epistemology][40]
* [Knowledge Based Systems][41]
* [Metacognition][42]
* [Artificial Intelligence][43]
* [Knowledge Graphs and Semantic Data Integration][44]

## Data Availability

No datasets were generated or analyzed during the current study.

## Notes
1.  Please find more details from [https://wordnet.princeton.edu/][45]. Core WordNet is available in
    [https://wordnetcode.princeton.edu/glosstag.shtml][46].
2.  We use the ConceptNet version 5.7.0, which is available at
    [https://github.com/commonsense/conceptnet5/wiki/Downloads][47].
3.  We use the DBpedia version 2022.09.01, which is available at [https://www.dbpedia.org/resources/][48].
4.  The project description and mappings are available on [https://github.com/usc-isi-i2/cskg][49]. Please refer to o
    Ilievski et al. [[7][50]] for more details on processing individual sources, performing node resolution, and
    constructing mappings.
5.  Used version: [https://huggingface.co/sentence-transformers/all-mpnet-base-v2][51].
6.  [https://www.nltk.org/][52]
7.  [https://wordnetcode.princeton.edu/standoff-files/core-wordnet.txt][53]
8.  In our experiment, the used pretrained model is all-mpnet-base-v2 Having undergone pretraining on over 1 billion
    sentence pairs, this model is capable of mapping input text to a 768-dimensional vector space, ideal for tasks such
    as clustering or semantic search. Further details can be found at:
    [https://huggingface.co/sentence-transformers/all-mpnet-base-v2][54].
9.  More details are available at [https://sbert.net/examples/applications/clustering/README.html][55].
10. Performances of compared knowledge bases are reported by [[36][56]], which are evaluated through crowdsourcing on
    the Amazon Mechanical Turk platform.
11. [https://github.com/mfaruqui/retrofitting][57]
12. We use the Text8Corpus which is available in Gensim: [https://github.com/RaRe-Technologies/gensim-data][58], and the
    CBOW model for training: [https://code.google.com/archive/p/word2vec/][59]
13. [https://nlp.stanford.edu/projects/glove/][60]
14. [https://github.com/facebookresearch/LAMA][61]

## References
1.   Cambria E, Hussain A, Havasi C, Eckl C. Common sense computing: from the society of mind to digital intuition and
     beyond. In: Biometric ID management and multimodal communication. Lecture Notes in Computer Science; 2009. vol.
     5707, pp. 252–9.
2.   Lenat DB. CYC: a large-scale investment in knowledge infrastructure. Commun ACM. 1995;38(11):32–8.
     
     [Article][62]  [ Google Scholar][63] 
3.   Baker CF, Fillmore CJ, Lowe JB. The Berkeley FrameNet project. In: Proceedings of annual meeting of the Association
     for Computational Linguistics, ACL. 1998. pp. 86–90.
4.   Speer R, Chin J, Havasi C. Conceptnet 5.5: an open multilingual graph of general knowledge. In: Proceedings of AAAI
     conference on artificial intelligence (AAAI). 2017. pp. 4444–51.
5.   Zhang H, Khashabi D, Song Y, Roth D. Transomcs: from linguistic graphs to commonsense knowledge. In: Proceedings of
     the International Joint Conference on Artificial Intelligence, IJCAI. 2020. pp. 4004–10.
6.   Sap M, Le Bras R, Allaway E, Bhagavatula C, Lourie N, Rashkin H, Roof B, Smith NA, Choi Y. Atomic: an atlas of
     machine commonsense for if-then reasoning. In: Proceedings of the AAAI conference on artificial intelligence. 2019.
     vol. 33, pp. 3027–35.
7.   Ilievski F, Szekely PA, Zhang B. CSKG: the commonsense knowledge graph. In: Proceedings of the semantic web - 18th
     international conference, ESWC. Lecture Notes in Computer Science; 2021. vol. 12731, pp. 680–96.
8.   Liu J, Chen T, Wang C, Liang J, Chen L, Xiao Y, Chen Y, Jin K. Vocsk: verb-oriented commonsense knowledge mining
     with taxonomy-guided induction. Artif Intell. 2022;310: 103744.
     
     [Article][64]  [MathSciNet][65]  [ Google Scholar][66] 
9.   Cambria E, Mao R, Chen M, Wang Z, Ho S-B. Seven pillars for the future of artificial intelligence. IEEE Intell
     Syst. 2023;38(6):62–9.
     
     [Article][67]  [ Google Scholar][68] 
10.  Zechmeister EB, Chronis AM, Cull WL, D’Anna CA, Healy NA. Growth of a functionally important lexicon. J Read Behav.
     1995;27(2):201–12.
     
     [Article][69]  [ Google Scholar][70] 
11.  Jackendoff R. Toward an explanatory semantic representation. Linguist Inq. 1976;7(1):89–150.
     
     [ Google Scholar][71] 
12.  Minsky M. A framework for representing knowledge. Cambridge: MIT; 1974.
     
     [ Google Scholar][72] 
13.  Rumelhart DE, Ortony A. The representation of knowledge in memory. Schooling and the acquisition of knowledge.
     1977;99:135.
     
     [ Google Scholar][73] 
14.  Schank RC. Conceptual dependency: a theory of natural language understanding. Cogn Psychol. 1972;3(4):552–631.
     
     [Article][74]  [ Google Scholar][75] 
15.  Wierzbicka A. Semantics: primes and universals: primes and universals. UK: Oxford University Press; 1996.
     
     [Book][76]  [ Google Scholar][77] 
16.  Ge M, Mao R, Cambria E. Explainable metaphor identification inspired by conceptual metaphor theory. Proc AAAI Conf
     Artif Intell. 2022;36(10):10681–9.
     
     [ Google Scholar][78] 
17.  Mao R, Li X, He K, Ge M, Cambria E. MetaPro Online: a computational metaphor processing online system. In:
     Proceedings of the annual meeting of the association for computational linguistics (Volume 3: System
     Demonstrations). 2023. pp. 127–35.
18.  Mao R, Du K, Ma Y, Zhu L, Cambria E. Discovering the cognition behind language: financial metaphor analysis with
     MetaPro. In: 2023 IEEE International Conference on Data Mining (ICDM). IEEE; 2023. pp. 1211–16.
19.  Cambria E, Zhang X, Mao R, Chen M, Kwok K. SenticNet 8: fusing emotion AI and commonsense AI for interpretable,
     trustworthy, and explainable affective computing. In: International conference on Human-Computer Interaction
     (HCII). 2024.
20.  Zhang H, Liu X, Pan H, Song Y, Leung CW. ASER: a large-scale eventuality knowledge graph. In: Proceedings of The
     Web Conference 2020, WWW. 2020. pp. 201–11.
21.  Wu W, Li H, Wang H, Zhu KQ. Probase: a probabilistic taxonomy for text understanding. In: Proceedings of the ACM
     SIGMOD international conference on management of data, SIGMOD. 2012. pp. 481–92.
22.  Wang Z, Wang H, Wen J, Xiao Y. An inference approach to basic level of categorization. In: Proceedings of the ACM
     international Conference on Information and Knowledge Management, CIKM. 2015. pp. 653–62.
23.  Chomsky N. Syntactic structures. Berlin, Boston: De Gruyter Mouton; 1957.
     
     [Book][79]  [ Google Scholar][80] 
24.  Jackendoff RS, et al. Semantics and cognition. Cambridge, Massachusetts: The MIT Press; 1983.
     
     [ Google Scholar][81] 
25.  Pesina S, Solonchak T. Semantic primitives and conceptual focus. Procedia Soc Behav Sci. 2015;192:339–45.
     
     [Article][82]  [ Google Scholar][83] 
26.  Piaget J, Cook M, et al. The origins of intelligence in children, vol. 8. New York: International Universities
     Press; 1952.
     
     [Book][84]  [ Google Scholar][85] 
27.  Winograd T. Towards a procedural understanding of semantics. Revue internationale de philosophie. 1976;260–303.
28.  Bobrow DG, Norman DA. Some principles of memory schemata. In: Representation and understanding. Morgan Kaufmann,
     San Diego; 1975. pp. 131–49.
29.  Johnson M. The body in the mind: the bodily basis of meaning, imagination, and reason. J Aesthetics and Art
     Criticism. 1989;47(4).
30.  Spelke ES, Kinzler KD. Core knowledge. Dev Sci. 2007;10(1):89–96.
     
     [Article][86]  [ Google Scholar][87] 
31.  West M. Developing high quality data models. Morgan Kaufmann Publishers Inc., 340 Pine Street, Sixth FloorSan
     FranciscoCAUnited States; 2011.
32.  Wachowiak L, Gromann D. Systematic analysis of image schemas in natural language through explainable multilingual
     neural language processing. In: Proceedings of the international conference on computational linguistics, COLING.
     2022. pp. 5571–81.
33.  Miller GA. Wordnet: a lexical database for english. Commun ACM. 1995;38:39–41.
     
     [Article][88]  [ Google Scholar][89] 
34.  Kipfer BA. Roget’s 21st century thesaurus in dictionary form. 3rd ed. New York, NY: Bantam Dell; 2006.
     
     [ Google Scholar][90] 
35.  Auer S, Bizer C, Kobilarov G, Lehmann J, Cyganiak R, Ives ZG. DBpedia: a nucleus for a web of open data. In:
     Proceedings of the semantic web, 6th international semantic web conference, 2nd Asian Semantic Web Conference.
     2007. vol. 4825, pp. 722–35.
36.  Hwang JD, Bhagavatula C, Bras RL, Da J, Sakaguchi K, Bosselut A, Choi Y. Comet-atomic 2020: on symbolic and neural
     commonsense knowledge graphs. In: Proceedings of the AAAI conference on artificial intelligence. 2020.
37.  Krishna R, Zhu Y, Groth O, Johnson J, Hata K, Kravitz J, Chen S, Kalantidis Y, Li L, Shamma DA, Bernstein MS,
     Fei-Fei L. Visual genome: connecting language and vision using crowdsourced dense image annotations. Int J Comput
     Vision. 2017;123(1):32–73.
     
     [Article][91]  [MathSciNet][92]  [ Google Scholar][93] 
38.  Reimers N, Gurevych I. Sentence-bert: sentence embeddings using siamese bert-networks. In: Proceedings of the
     conference on empirical methods in natural language processing and the 9th International Joint Conference on
     Natural Language Processing, EMNLP-IJCNLP. 2019. pp. 3980–90.
39.  Cambria E, Mao R, Han S, Liu Q. Sentic parser: a graph-based approach to concept extraction for sentiment analysis.
     In: Proceedings of ICDM workshops. 2022. pp. 413–20.
40.  Guarino N. Formal ontology, conceptual analysis and knowledge representation. Int J Hum Comput Stud.
     1995;43(5–6):625–40.
     
     [Article][94]  [ Google Scholar][95] 
41.  Von Ahn L. Games with a purpose. Computer. 2006;39(6):92–4.
     
     [Article][96]  [ Google Scholar][97] 
42.  Faruqui M, Dodge J, Jauhar SK, Dyer C, Hovy EH, Smith NA. Retrofitting word vectors to semantic lexicons. In:
     Proceedings of the conference of the North American chapter of the association for computational linguistics: human
     language technologies. 2015. pp. 1606–15.
43.  Liu Q, Huang H, Zhang G, Gao Y, Xuan J, Lu J. Semantic structure-based word embedding by incorporating concept
     convergence and word divergence. In: Proceedings of the AAAI conference on artificial intelligence. 2018. pp.
     5261–8.
44.  Myers JL, Well AD. Research design & statistical analysis. New York: Routledge; 1995.
     
     [ Google Scholar][98] 
45.  Yang D, Powers DM. Measuring semantic similarity in the taxonomy of WordNet. Australia: Australian Computer
     Society; 2005.
     
     [ Google Scholar][99] 
46.  Bruni E, Boleda G, Baroni M, Tran N. Distributional semantics in technicolor. In: Proceedings of the annual meeting
     of the Association for Computational Linguistics, ACL. 2012. pp. 136–45.
47.  Rubenstein H, Goodenough JB. Contextual correlates of synonymy. Commun ACM. 1965;8(10):627–33.
     
     [Article][100]  [ Google Scholar][101] 
48.  Halawi G, Dror G, Gabrilovich E, Koren Y. Large-scale learning of word relatedness with constraints. In:
     Proceedings of ACM SIGKDD international conference on knowledge discovery and data mining. 2012. pp. 1406–14.
49.  Hill F, Reichart R, Korhonen A. Simlex-999: evaluating semantic models with (genuine) similarity estimation. Comput
     Linguist. 2015;41(4):665–95.
     
     [Article][102]  [MathSciNet][103]  [ Google Scholar][104] 
50.  Gerz D, Vulic I, Hill F, Reichart R, Korhonen A. Simverb-3500: a large-scale evaluation set of verb similarity. In:
     Proceedings of the conference on empirical methods in natural language processing, EMNLP. 2016. pp. 2173–82.
51.  Baker S, Reichart R, Korhonen A. An unsupervised model for instance level subcategorization acquisition. In:
     Proceedings of the conference on empirical methods in natural language processing, EMNLP. 2014. pp. 278–89.
52.  Finkelstein L, Gabrilovich E, Matias Y, Rivlin E, Solan Z, Wolfman G, Ruppin E. Placing search in context: the
     concept revisited. In: Proceedings of the international World Wide Web Conference, WWW. 2001. pp. 406–14.
53.  Mikolov T, Sutskever I, Chen K, Corrado GS, Dean J. Distributed representations of words and phrases and their
     compositionality. In: Proceedings of advances in neural information processing systems. 2013. pp. 3111–9.
54.  Pennington J, Socher R, Manning CD. Glove: global vectors for word representation. In: Proceedings of the
     conference on Empirical Methods in Natural Language Processing, EMNLP. 2014. pp. 1532–43.
55.  Liu Q, Geng X, Wang Y, Cambria E, Jiang D. Disentangled retrieval and reasoning for implicit question answering.
     IEEE Trans Neural Netw Learn Syst. 2024;35(6):7804–15.
     
     [Article][105]  [ Google Scholar][106] 
56.  Ilievski F, Oltramari A, Ma K, Zhang B, McGuinness DL, Szekely PA. Dimensions of commonsense knowledge. Knowl-Based
     Syst. 2021;229:107347.
     
     [Article][107]  [ Google Scholar][108] 
57.  Ma K, Ilievski F, Francis J, Bisk Y, Nyberg E, Oltramari A. Knowledge-driven data construction for zero-shot
     evaluation in commonsense question answering. In: Proceedings of thirty-fifth AAAI conference on artificial
     intelligence, AAAI. 2021. pp. 13507–15.
58.  Shwartz V, West P, Bras RL, Bhagavatula C, Choi Y. Unsupervised commonsense question answering with self-talk. In:
     Proceedings of the 2020 conference on Empirical Methods in Natural Language Processing, EMNLP. 2020. pp. 4615–29.
59.  Banerjee P, Baral C. Self-supervised knowledge triplet learning for zero-shot question answering. In: Proceedings
     of the conference on Empirical Methods in Natural Language Processing, EMNLP. 2020. pp. 151–62.
60.  Levesque HJ. The winograd schema challenge. In: Logical formalizations of commonsense reasoning, Papers from the
     2011 AAAI Spring Symposium, Technical Report SS-11-06. 2011. pp. 1–1.
61.  Bhagavatula C, Bras RL, Malaviya C, Sakaguchi K, Holtzman A, Rashkin H, Downey D, Yih W, Choi Y. Abductive
     commonsense reasoning. In: Proceedings of International Conference on Learning Representations, ICLR. 2020.
62.  Talmor A, Herzig J, Lourie N, Berant J. Commonsenseqa: a question answering challenge targeting commonsense
     knowledge. In: Proceedings of the conference of the North American Chapter of the Association for Computational
     Linguistics: Human Language Technologies, NAACL-HLT. 2019. pp. 4149–58.
63.  Bisk Y, Zellers R, Bras RL, Gao J, Choi Y. PIQA: reasoning about physical commonsense in natural language. In:
     Proceedings of the thirty-fourth AAAI conference on Artificial Intelligence, AAAI. 2020. pp. 7432–9.
64.  Sap M, Rashkin H, Chen D, Bras RL, Choi Y. Social iqa: commonsense reasoning about social interactions. In:
     Proceedings of the conference on Empirical Methods in Natural Language Processing and the 9th International Joint
     Conference on Natural Language Processing, EMNLP-IJCNLP. 2019. pp. 4462–72.
65.  Sakaguchi K, Bras RL, Bhagavatula C, Choi Y. Winogrande: an adversarial winograd schema challenge at scale. In:
     Proceedings of the thirty-fourth AAAI conference on Artificial Intelligence, AAAI. 2020. pp. 8732–40.
66.  Singh P, Lin T, Mueller ET, Lim G, Perkins T, Zhu WL. Open mind common sense: knowledge acquisition from the
     general public. In: On the move to meaningful internet systems. Lecture Notes in Computer Science; 2002. vol. 2519,
     pp. 1223–37.
67.  Chklovski T. Learner: a system for acquiring commonsense knowledge by analogy. In: Gennari JH, Porter BW, Gil Y,
     editors. Proceedings of the 2nd international conference on knowledge capture (K-CAP 2003). 2003. pp. 4–12.
68.  Ahn L, Kedia M, Blum M. Verbosity: a game for collecting common-sense facts. In: Proceedings of the 2006 conference
     on human factors in computing systems, CHI. 2006. pp. 75–8.
69.  Kuo Y, Lee J, Chiang K, Wang R, Shen E, Chan C, Hsu JY. Community-based game design: experiments on social games
     for commonsense data collection. In: Proceedings of the ACM SIGKDD workshop on human computation. 2009. pp. 15–22.
70.  Gangemi A, Guarino N, Masolo C, Oltramari A, Schneider L. Sweetening ontologies with DOLCE. In: Knowledge
     engineering and knowledge management. Ontologies and the Semantic Web, 13th International Conference, EKAW. Lecture
     Notes in Computer Science; 2002. vol. 2473, pp. 166–81.
71.  Bollacker KD, Evans C, Paritosh PK, Sturge T, Taylor J. Freebase: a collaboratively created graph database for
     structuring human knowledge. In: Proceedings of the ACM SIGMOD international conference on management of data,
     SIGMOD. 2008. pp. 1247–50.
72.  Singhal A. Official google blog: introducing the knowledge graph: things, not strings. 2012.
73.  Dodge E, Hong J, Stickles E. MetaNet: deep semantic automatic metaphor analysis. In: Proceedings of the third
     workshop on metaphor in NLP. 2015. pp. 40–9.
74.  Schuler KK. VerbNet: a broad-coverage, comprehensive verb lexicon. University of Pennsylvania, Philadelphia, PA,
     United States; 2005.
75.  Palmer M, Kingsbury PR, Gildea D. The proposition bank: an annotated corpus of semantic roles. Comput Linguist.
     2005;31(1):71–106.
     
     [Article][109]  [ Google Scholar][110] 
76.  Vrandecic D, Krötzsch M. Wikidata: a free collaborative knowledgebase. Commun ACM. 2014;57(10):78–85.
     
     [Article][111]  [ Google Scholar][112] 
77.  Lehmann J, Isele R, Jakob M, Jentzsch A, Kontokostas D, Mendes PN, Hellmann S, Morsey M, Kleef P, Auer S, Bizer C.
     Dbpedia - a large-scale, multilingual knowledge base extracted from wikipedia. Semantic Web. 2015;6(2):167–95.
     
     [Article][113]  [ Google Scholar][114] 
78.  Young T, Cambria E, Chaturvedi I, Zhou H, Biswas S, Huang M. Augmenting end-to-end dialogue systems with
     commonsense knowledge. In: Proceedings of the Thirty-Second AAAI conference on artificial intelligence. 2018. pp.
     4970–7.
79.  Mitchell T, Fredkin E. Never-ending language learning. In: 2014 IEEE International conference on big data (Big
     Data). 2014. pp. 1–1.
80.  Tandon N, Melo G, Suchanek FM, Weikum G. Webchild: harvesting and organizing commonsense knowledge from the web.
     In: Proceedings of the 7th ACM international conference on web search and data mining, WSDM. 2014. pp. 523–32.
81.  Ji L, Wang Y, Shi B, Zhang D, Wang Z, Yan J. Microsoft concept graph: mining semantic concepts for short text
     understanding. Data Intelligence. 2019;1(3):238–70.
     
     [Article][115]  [ Google Scholar][116] 
82.  Navigli R, Ponzetto SP. Babelnet: the automatic construction, evaluation and application of a wide-coverage
     multilingual semantic network. Artif Intell. 2012;193:217–50.
     
     [Article][117]  [MathSciNet][118]  [ Google Scholar][119] 
83.  Shen X, Wu S, Xia R. Dense-atomic: towards densely-connected ATOMIC with high knowledge coverage and massive
     multi-hop paths. In: Proceedings of the annual meeting of the Association for Computational Linguistics, ACL. 2023.
     pp. 13292–305.
84.  Suchanek FM, Kasneci G, Weikum G. Yago: a core of semantic knowledge. In: Proceedings of the international
     conference on the World Wide Web, WWW. 2007. pp. 697–706.
85.  Bouraoui Z, Konieczny S, Ma T, Schwind N, Varzinczak I. Region-based merging of open-domain terminological
     knowledge. In: Proceedings of the international conference on principles of knowledge representation and reasoning,
     KR. 2022. pp. 81–90.
86.  AlKhamissi B, Li M, Celikyilmaz A, Diab MT, Ghazvininejad M. A review on language models as knowledge bases. CoRR
     abs/2204.06031. 2022.
87.  Bhargava P, Ng V. Commonsense knowledge reasoning and generation with pre-trained language models: a survey. In:
     Proceedings of thirty-sixth AAAI conference on Artificial Intelligence, AAAI. 2022. pp. 12317–25.
88.  Radford A, Narasimhan K, Salimans T, Sutskever I, et al. Improving language understanding by generative
     pre-training. Open AI Preprint. 2018.
89.  Radford A, Wu J, Child R, Luan D, Amodei D, Sutskever I, et al. Language models are unsupervised multitask
     learners. OpenAI blog. 2019;1(8):9.
     
     [ Google Scholar][120] 
90.  Brown TB, Mann B, Ryder N, Subbiah M, Kaplan J, Dhariwal P, Neelakantan A, Shyam P, Sastry G, Askell A, Agarwal S,
     Herbert-Voss A, Krueger G, Henighan T, Child R, Ramesh A, Ziegler DM, Wu J, Winter C, Hesse C, Chen M, Sigler E,
     Litwin M, Gray S, Chess B, Clark J, Berner C, McCandlish S, Radford A, Sutskever I, Amodei D. Language models are
     few-shot learners. In: Proceedings of advances in Neural Information Processing Systems, NeurIPS. 2020.
91.  Yao L, Mao C, Luo Y. KG-BERT: BERT for knowledge graph completion. CoRR abs/1909.03193. 2019.
92.  Fang T, Wang W, Choi S, Hao S, Zhang H, Song Y, He B. Benchmarking commonsense knowledge base population with an
     effective evaluation dataset. In: Proceedings of the conference on Empirical Methods in Natural Language
     Processing, EMNLP. 2021. pp. 8949–64.
93.  Fang T, Do QV, Zhang H, Song Y, Wong GY, See S. Pseudoreasoner: leveraging pseudo labels for commonsense knowledge
     base population. In: Findings of the association for computational linguistics, EMNLP. 2022. pp. 3379–94.
94.  Bosselut A, Rashkin H, Sap M, Malaviya C, Celikyilmaz A, Choi Y. COMET: commonsense transformers for automatic
     knowledge graph construction. In: Proceedings of the 57th conference of the Association for Computational
     Linguistics, ACL. 2019. pp. 4762–79.
95.  Petroni F, Rocktäschel T, Riedel S, Lewis PSH, Bakhtin A, Wu Y, Miller AH. Language models as knowledge bases? In:
     Proceedings of the conference on Empirical Methods in Natural Language Processing and the 9th International Joint
     Conference on Natural Language Processing, EMNLP-IJCNLP. 2019. pp. 2463–73.
96.  Petroni F, Lewis PSH, Piktus A, Rocktäschel T, Wu Y, Miller AH, Riedel S. How context affects language models’
     factual predictions. In: Proceedings of conference on automated knowledge base construction, AKBC0. 2020.
97.  West P, Bhagavatula C, Hessel J, Hwang JD, Jiang L, Bras RL, Lu X, Welleck S, Choi Y. Symbolic knowledge
     distillation: from general language models to commonsense models. In: Proceedings of the conference of the North
     American Chapter of the Association for Computational Linguistics: Human Language Technologies, NAACL. 2022. pp.
     4602–25.
98.  Goel V, Navarrete G, Noveck IA, Prado J. The reasoning brain: the interplay between cognitive neuroscience and
     theories of reasoning. Frontiers Media SA. 2017.
99.  Salmon MH. Introduction to logic and critical thinking. 1989.
100. Clark P, Tafjord O, Richardson K. Transformers as soft reasoners over language. In: Proceedings of IJCAI. 2020. pp.
     3882–90.
101. Yang Z, Dong L, Du X, Cheng H, Cambria E, Liu X, Gao J, Wei F. Language models as inductive reasoners. In:
     Proceedings of EACL. 2024. pp. 209–225
102. Geva M, Khashabi D, Segal E, Khot T, Roth D, Berant J. Did Aristotle use a laptop? A question answering benchmark
     with implicit reasoning strategies. Trans Assoc Comput Linguist. 2021;9:346–61.
     
     [Article][121]  [ Google Scholar][122] 
103. Rajpurkar P, Zhang J, Lopyrev K, Liang P. SQuAD: 100,000+ questions for machine comprehension of text. In:
     Proceedings of the 2016 conference on empirical methods in natural language processing. 2016. pp. 2383–92.
104. Joshi M, Choi E, Weld D, Zettlemoyer L. TriviaQA: a large scale distantly supervised challenge dataset for reading
     comprehension. In: Proceedings of the annual meeting of the association for computational linguistics, ACL. 2017.
     pp. 1601–11.
105. Lake BM, Ullman TD, Tenenbaum JB, Gershman SJ. Building machines that learn and think like people. Behav Brain Sci.
     2017;40:253.
106. Musen MA, Lei J. Of brittleness and bottlenecks: challenges in the creation of pattern-recognition and
     expert-system models. In: Machine intelligence and pattern recognition. 1988. vol. 7, pp. 335–52.
107. Li W, Zhu L, Mao R, Cambria E. SKIER: a symbolic knowledge integrated model for conversational emotion recognition.
     In: Proceedings of the AAAI conference on artificial intelligence. 2023. pp. 13121–9.
108. Smolensky P, McCoy R, Fernandez R, Goldrick M, Gao J. Neurocompositional computing: from the central paradox of
     cognition to a new generation of ai systems. AI Mag. 2022;43(3):308–22.
     
     [ Google Scholar][123] 

[Download references][124]

## Funding

This research is supported by the Agency for Science, Technology and Research (A*STAR) under its AME Programmatic
Funding Scheme (Project #A18A2b0046). The project is also supported by the Ministry of Education, Singapore under its
MOE Academic Research Fund Tier 2 (STEM RIE2025 Award MOE-T2EP20123-0005) and by the RIE2025 Industry Alignment Fund -
Industry Collaboration Projects (IAF-ICP) (Award I2301E0026), administered by A*STAR, as well as supported by Alibaba
Group and NTU Singapore.

## Author information

### Authors and Affiliations
1. School of Computer Science, The University of Auckland, Auckland, New Zealand
   
   Qian Liu
2. Intapp, Berlin, Germany
   
   Sooji Han
3. College of Computing and Data Science, Nanyang Technological University, Singapore, Singapore
   
   Erik Cambria
4. School of Automation, Northwestern Polytechnical University, Xi’an, China
   
   Yang Li
5. Institute of High Performance Computing, A*STAR, Singapore, Singapore
   
   Kenneth Kwok

Authors
1. Qian Liu
   [View author publications][125]
   
   Search author on:[PubMed][126] [Google Scholar][127]
2. Sooji Han
   [View author publications][128]
   
   Search author on:[PubMed][129] [Google Scholar][130]
3. Erik Cambria
   [View author publications][131]
   
   Search author on:[PubMed][132] [Google Scholar][133]
4. Yang Li
   [View author publications][134]
   
   Search author on:[PubMed][135] [Google Scholar][136]
5. Kenneth Kwok
   [View author publications][137]
   
   Search author on:[PubMed][138] [Google Scholar][139]

### Contributions

Qian Liu: conceptualization, methodology, software, validation, writing—original draft, visualization. Sooji Han: formal
analysis, investigation, writing—original draft. Erik Cambria: conceptualization, methodology, writing—review and
editing, supervision, project administration, funding acquisition. Yang Li: software, resources, data curation,
writing—review and editing. Kenneth Kwok: investigation, writing—reviewing and editing, funding acquisition.

### Corresponding author

Correspondence to [Erik Cambria][140].

## Ethics declarations

### Conflict of Interest

The authors declare no competing interests.

## Additional information

### Publisher's Note

Springer Nature remains neutral with regard to jurisdictional claims in published maps and institutional affiliations.

## Rights and permissions

Springer Nature or its licensor (e.g. a society or other partner) holds exclusive rights to this article under a
publishing agreement with the author(s) or other rightsholder(s); author self-archiving of the accepted manuscript
version of this article is solely governed by the terms of such publishing agreement and applicable law.

[Reprints and permissions][141]

## About this article

[[Check for updates. Verify currency and authenticity via CrossMark]][142]

### Cite this article

Liu, Q., Han, S., Cambria, E. et al. PrimeNet: A Framework for Commonsense Knowledge Representation and Reasoning Based
on Conceptual Primitives. Cogn Comput **16**, 3429–3456 (2024). https://doi.org/10.1007/s12559-024-10345-6

[Download citation][143]
* Received: 07 April 2024
* Accepted: 12 August 2024
* Published: 30 August 2024
* Version of record: 30 August 2024
* Issue date: November 2024
* DOI: https://doi.org/10.1007/s12559-024-10345-6

### Share this article

Anyone you share the following link with will be able to read this content:

Get shareable link

Sorry, a shareable link is not currently available for this article.

Copy shareable link to clipboard

Provided by the Springer Nature SharedIt content-sharing initiative

### Keywords
* [Commonsense acquisition][144]
* [Knowledge representation and reasoning][145]
* [Conceptual primitives][146]

### Profiles
1. Erik Cambria [ View author profile ][147]
2. Yang Li [ View author profile ][148]

## Associated Content

Part of a collection:

### [Cognitive Analysis for Humans and AI][149]

## Access this article

[ Log in via an institution ][150]

## Subscribe and save

Springer+
from €37.37 /Month
* Starting from 10 chapters or articles per month
* Access and download chapters and articles from more than 300k books and 2,500 journals
* Cancel anytime
[View plans ][151]

## Buy Now

Buy article PDF 39,95 €

Price includes VAT (Thailand)

Instant access to the full article PDF.

[Institutional subscriptions ][152]

Advertisement

## Search

Search by keyword or author
Search

## Navigation
* [ Find a journal ][153]
* [ Publish with us ][154]
* [ Track your research ][155]

## Footer Navigation

### Discover content
* [Journals A-Z][156]
* [Books A-Z][157]
* [Subjects A-Z][158]

### Publish with us
* [Journal finder][159]
* [Publish your research][160]
* [Language editing][161]
* [Open access publishing][162]

### Products and services
* [Our products][163]
* [Librarians][164]
* [Societies][165]
* [Partners and advertisers][166]

### Our brands
* [Springer][167]
* [Nature Portfolio][168]
* [BMC][169]
* [Palgrave Macmillan][170]
* [Apress][171]
* [Discover][172]

### Corporate Navigation
* Your privacy choices/Manage cookies
* [Your US state privacy rights][173]
* [Accessibility statement][174]
* [Terms and conditions][175]
* [Privacy policy][176]
* [Help and support][177]
* [Legal notice][178]
* [Cancel contracts here][179]

184.22.9.30

Not affiliated

[ [Springer Nature] ][180]

© 2026 Springer Nature

[1]: #main
[2]: //pubads.g.doubleclick.net/gampad/jump?iu=/270604982/springerlink/12559/article&sz=728x90&pos=top&articleid=s12559-
024-10345-6
[3]: https://link.springer.com
[4]: https://idp.springer.com/auth/personal/springernature?redirect_uri=https://link.springer.com/article/10.1007/s12559
-024-10345-6?
[5]: #eds-c-header-nav
[6]: https://link.springer.com/journals/
[7]: https://www.springernature.com/gp/authors
[8]: https://link.springernature.com/home/
[9]: #eds-c-header-popup-search
[10]: /saved-research
[11]: https://order.springer.com/public/cart
[12]: /
[13]: /journal/12559
[14]: #citeas
[15]: /article/10.1007/s12559-024-10345-6/save-research?_csrf=xloNwLHnXVpnTtu2MMZTVj32NpzewX9D
[16]: /saved-research
[17]: /journal/12559
[18]: /journal/12559/aims-and-scope
[19]: https://submission.springernature.com/new-submission/12559/3
[20]: #auth-Qian-Liu-Aff1
[21]: #Aff1
[22]: #auth-Sooji-Han-Aff2
[23]: #Aff2
[24]: #auth-Erik-Cambria-Aff3
[25]: #Aff3
[26]: #auth-Yang-Li-Aff4
[27]: #Aff4
[28]: #auth-Kenneth-Kwok-Aff5
[29]: #Aff5
[30]: /article/10.1007/s12559-024-10345-6/metrics
[31]: //wayf.springernature.com?redirect_uri=https%3A%2F%2Flink.springer.com%2Farticle%2F10.1007%2Fs12559-024-10345-6%3F
error%3Dcookies_not_supported%26code%3D1fe3cd89-e3f8-493a-a702-87add1f87b49
[32]: //wayf.springernature.com?redirect_uri=https%3A%2F%2Flink.springer.com%2Farticle%2F10.1007%2Fs12559-024-10345-6%3F
error%3Dcookies_not_supported%26code%3D1fe3cd89-e3f8-493a-a702-87add1f87b49
[33]: https://link.springer.com/product/springer-plus
[34]: https://www.springernature.com/gp/librarians/licensing/agc/journals
[35]: https://link.springer.com/10.1007/978-3-031-73974-3_5?fromPaywallRec=true
[36]: https://link.springer.com/10.1038/s41551-025-01455-z?fromPaywallRec=true
[37]: https://link.springer.com/10.1007/s41095-018-0128-6?fromPaywallRec=true
[38]: /subjects/computational-intelligence
[39]: /subjects/elispot
[40]: /subjects/epistemology
[41]: /subjects/knowledge-based-systems
[42]: /subjects/metacognition
[43]: /subjects/artificial-intelligence
[44]: /subjects/knowledge-graphs-and-semantic-data-integration
[45]: https://wordnet.princeton.edu/
[46]: https://wordnetcode.princeton.edu/glosstag.shtml
[47]: https://github.com/commonsense/conceptnet5/wiki/Downloads
[48]: https://www.dbpedia.org/resources/
[49]: https://github.com/usc-isi-i2/cskg
[50]: /article/10.1007/s12559-024-10345-6#ref-CR7
[51]: https://huggingface.co/sentence-transformers/all-mpnet-base-v2
[52]: https://www.nltk.org/
[53]: https://wordnetcode.princeton.edu/standoff-files/core-wordnet.txt
[54]: https://huggingface.co/sentence-transformers/all-mpnet-base-v2
[55]: https://sbert.net/examples/applications/clustering/README.html
[56]: /article/10.1007/s12559-024-10345-6#ref-CR36
[57]: https://github.com/mfaruqui/retrofitting
[58]: https://github.com/RaRe-Technologies/gensim-data
[59]: https://code.google.com/archive/p/word2vec/
[60]: https://nlp.stanford.edu/projects/glove/
[61]: https://github.com/facebookresearch/LAMA
[62]: https://doi.org/10.1145%2F219717.219745
[63]: http://scholar.google.com/scholar_lookup?&title=CYC%3A%20a%20large-scale%20investment%20in%20knowledge%20infrastru
cture&journal=Commun.%20ACM&doi=10.1145%2F219717.219745&volume=38&issue=11&pages=32-38&publication_year=1995&author=Lena
t%2CDB
[64]: https://doi.org/10.1016%2Fj.artint.2022.103744
[65]: http://www.ams.org/mathscinet-getitem?mr=4442911
[66]: http://scholar.google.com/scholar_lookup?&title=Vocsk%3A%20verb-oriented%20commonsense%20knowledge%20mining%20with
%20taxonomy-guided%20induction&journal=Artif.%20Intell.&doi=10.1016%2Fj.artint.2022.103744&volume=310&publication_year=2
022&author=Liu%2CJ&author=Chen%2CT&author=Wang%2CC&author=Liang%2CJ&author=Chen%2CL&author=Xiao%2CY&author=Chen%2CY&auth
or=Jin%2CK
[67]: https://doi.org/10.1109%2FMIS.2023.3329745
[68]: http://scholar.google.com/scholar_lookup?&title=Seven%20pillars%20for%20the%20future%20of%20artificial%20intellige
nce&journal=IEEE%20Intell.%20Syst.&doi=10.1109%2FMIS.2023.3329745&volume=38&issue=6&pages=62-69&publication_year=2023&au
thor=Cambria%2CE&author=Mao%2CR&author=Chen%2CM&author=Wang%2CZ&author=Ho%2CS-B
[69]: https://doi.org/10.1080%2F10862969509547878
[70]: http://scholar.google.com/scholar_lookup?&title=Growth%20of%20a%20functionally%20important%20lexicon&journal=J%20R
ead%20Behav.&doi=10.1080%2F10862969509547878&volume=27&issue=2&pages=201-212&publication_year=1995&author=Zechmeister%2C
EB&author=Chronis%2CAM&author=Cull%2CWL&author=D%E2%80%99Anna%2CCA&author=Healy%2CNA
[71]: http://scholar.google.com/scholar_lookup?&title=Toward%20an%20explanatory%20semantic%20representation&journal=Ling
uist%20Inq.&volume=7&issue=1&pages=89-150&publication_year=1976&author=Jackendoff%2CR
[72]: http://scholar.google.com/scholar_lookup?&title=A%20framework%20for%20representing%20knowledge&publication_year=19
74&author=Minsky%2CM
[73]: http://scholar.google.com/scholar_lookup?&title=The%20representation%20of%20knowledge%20in%20memory&journal=School
ing%20and%20the%20acquisition%20of%20knowledge&volume=99&publication_year=1977&author=Rumelhart%2CDE&author=Ortony%2CA
[74]: https://doi.org/10.1016%2F0010-0285%2872%2990022-9
[75]: http://scholar.google.com/scholar_lookup?&title=Conceptual%20dependency%3A%20a%20theory%20of%20natural%20language%
20understanding&journal=Cogn%20Psychol.&doi=10.1016%2F0010-0285%2872%2990022-9&volume=3&issue=4&pages=552-631&publicatio
n_year=1972&author=Schank%2CRC
[76]: https://doi.org/10.1093%2Foso%2F9780198700029.001.0001
[77]: http://scholar.google.com/scholar_lookup?&title=Semantics%3A%20primes%20and%20universals%3A%20primes%20and%20unive
rsals&doi=10.1093%2Foso%2F9780198700029.001.0001&publication_year=1996&author=Wierzbicka%2CA
[78]: http://scholar.google.com/scholar_lookup?&title=Explainable%20metaphor%20identification%20inspired%20by%20conceptu
al%20metaphor%20theory&journal=Proc%20AAAI%20Conf%20Artif%20Intell&volume=36&issue=10&pages=10681-10689&publication_year
=2022&author=Ge%2CM&author=Mao%2CR&author=Cambria%2CE
[79]: https://doi.org/10.1515%2F9783112316009
[80]: http://scholar.google.com/scholar_lookup?&title=Syntactic%20structures&doi=10.1515%2F9783112316009&publication_yea
r=1957&author=Chomsky%2CN
[81]: http://scholar.google.com/scholar_lookup?&title=Semantics%20and%20cognition&publication_year=1983&author=Jackendof
f%2CRS
[82]: https://doi.org/10.1016%2Fj.sbspro.2015.06.048
[83]: http://scholar.google.com/scholar_lookup?&title=Semantic%20primitives%20and%20conceptual%20focus&journal=Procedia%
20Soc%20Behav%20Sci.&doi=10.1016%2Fj.sbspro.2015.06.048&volume=192&pages=339-345&publication_year=2015&author=Pesina%2CS
&author=Solonchak%2CT
[84]: https://doi.org/10.1037%2F11494-000
[85]: http://scholar.google.com/scholar_lookup?&title=The%20origins%20of%20intelligence%20in%20children&doi=10.1037%2F11
494-000&publication_year=1952&author=Piaget%2CJ&author=Cook%2CM
[86]: https://doi.org/10.1111%2Fj.1467-7687.2007.00569.x
[87]: http://scholar.google.com/scholar_lookup?&title=Core%20knowledge&journal=Dev%20Sci.&doi=10.1111%2Fj.1467-7687.2007
.00569.x&volume=10&issue=1&pages=89-96&publication_year=2007&author=Spelke%2CES&author=Kinzler%2CKD
[88]: https://doi.org/10.1145%2F219717.219748
[89]: http://scholar.google.com/scholar_lookup?&title=Wordnet%3A%20a%20lexical%20database%20for%20english&journal=Commun
%20ACM&doi=10.1145%2F219717.219748&volume=38&pages=39-41&publication_year=1995&author=Miller%2CGA
[90]: http://scholar.google.com/scholar_lookup?&title=Roget%E2%80%99s%2021st%20century%20thesaurus%20in%20dictionary%20f
orm&publication_year=2006&author=Kipfer%2CBA
[91]: https://link.springer.com/doi/10.1007/s11263-016-0981-7
[92]: http://www.ams.org/mathscinet-getitem?mr=3640738
[93]: http://scholar.google.com/scholar_lookup?&title=Visual%20genome%3A%20connecting%20language%20and%20vision%20using%
20crowdsourced%20dense%20image%20annotations&journal=Int%20J%20Comput%20Vision.&doi=10.1007%2Fs11263-016-0981-7&volume=1
23&issue=1&pages=32-73&publication_year=2017&author=Krishna%2CR&author=Zhu%2CY&author=Groth%2CO&author=Johnson%2CJ&autho
r=Hata%2CK&author=Kravitz%2CJ&author=Chen%2CS&author=Kalantidis%2CY&author=Li%2CL&author=Shamma%2CDA&author=Bernstein%2C
MS&author=Fei-Fei%2CL
[94]: https://doi.org/10.1006%2Fijhc.1995.1066
[95]: http://scholar.google.com/scholar_lookup?&title=Formal%20ontology%2C%20conceptual%20analysis%20and%20knowledge%20r
epresentation&journal=Int%20J%20Hum%20Comput%20Stud.&doi=10.1006%2Fijhc.1995.1066&volume=43&issue=5%E2%80%936&pages=625-
640&publication_year=1995&author=Guarino%2CN
[96]: https://doi.org/10.1109%2FMC.2006.196
[97]: http://scholar.google.com/scholar_lookup?&title=Games%20with%20a%20purpose&journal=Computer&doi=10.1109%2FMC.2006.
196&volume=39&issue=6&pages=92-94&publication_year=2006&author=Ahn%2CL
[98]: http://scholar.google.com/scholar_lookup?&title=Research%20design%20%26%20statistical%20analysis&publication_year=
1995&author=Myers%2CJL&author=Well%2CAD
[99]: http://scholar.google.com/scholar_lookup?&title=Measuring%20semantic%20similarity%20in%20the%20taxonomy%20of%20Wor
dNet&publication_year=2005&author=Yang%2CD&author=Powers%2CDM
[100]: https://doi.org/10.1145%2F365628.365657
[101]: http://scholar.google.com/scholar_lookup?&title=Contextual%20correlates%20of%20synonymy&journal=Commun%20ACM&doi=
10.1145%2F365628.365657&volume=8&issue=10&pages=627-633&publication_year=1965&author=Rubenstein%2CH&author=Goodenough%2C
JB
[102]: https://doi.org/10.1162%2FCOLI_a_00237
[103]: http://www.ams.org/mathscinet-getitem?mr=3449117
[104]: http://scholar.google.com/scholar_lookup?&title=Simlex-999%3A%20evaluating%20semantic%20models%20with%20%28genuin
e%29%20similarity%20estimation&journal=Comput%20Linguist.&doi=10.1162%2FCOLI_a_00237&volume=41&issue=4&pages=665-695&pub
lication_year=2015&author=Hill%2CF&author=Reichart%2CR&author=Korhonen%2CA
[105]: https://doi.org/10.1109%2FTNNLS.2022.3220933
[106]: http://scholar.google.com/scholar_lookup?&title=Disentangled%20retrieval%20and%20reasoning%20for%20implicit%20que
stion%20answering&journal=IEEE%20Trans%20Neural%20Netw%20Learn%20Syst.&doi=10.1109%2FTNNLS.2022.3220933&volume=35&issue=
6&pages=7804-7815&publication_year=2024&author=Liu%2CQ&author=Geng%2CX&author=Wang%2CY&author=Cambria%2CE&author=Jiang%2
CD
[107]: https://doi.org/10.1016%2Fj.knosys.2021.107347
[108]: http://scholar.google.com/scholar_lookup?&title=Dimensions%20of%20commonsense%20knowledge&journal=Knowl-Based%20S
yst.&doi=10.1016%2Fj.knosys.2021.107347&volume=229&publication_year=2021&author=Ilievski%2CF&author=Oltramari%2CA&author
=Ma%2CK&author=Zhang%2CB&author=McGuinness%2CDL&author=Szekely%2CPA
[109]: https://doi.org/10.1162%2F0891201053630264
[110]: http://scholar.google.com/scholar_lookup?&title=The%20proposition%20bank%3A%20an%20annotated%20corpus%20of%20sema
ntic%20roles&journal=Comput%20Linguist.&doi=10.1162%2F0891201053630264&volume=31&issue=1&pages=71-106&publication_year=2
005&author=Palmer%2CM&author=Kingsbury%2CPR&author=Gildea%2CD
[111]: https://doi.org/10.1145%2F2629489
[112]: http://scholar.google.com/scholar_lookup?&title=Wikidata%3A%20a%20free%20collaborative%20knowledgebase&journal=Co
mmun%20ACM&doi=10.1145%2F2629489&volume=57&issue=10&pages=78-85&publication_year=2014&author=Vrandecic%2CD&author=Kr%C3%
B6tzsch%2CM
[113]: https://doi.org/10.3233%2FSW-140134
[114]: http://scholar.google.com/scholar_lookup?&title=Dbpedia%20-%20a%20large-scale%2C%20multilingual%20knowledge%20bas
e%20extracted%20from%20wikipedia&journal=Semantic%20Web&doi=10.3233%2FSW-140134&volume=6&issue=2&pages=167-195&publicati
on_year=2015&author=Lehmann%2CJ&author=Isele%2CR&author=Jakob%2CM&author=Jentzsch%2CA&author=Kontokostas%2CD&author=Mend
es%2CPN&author=Hellmann%2CS&author=Morsey%2CM&author=Kleef%2CP&author=Auer%2CS&author=Bizer%2CC
[115]: https://doi.org/10.1162%2Fdint_a_00013
[116]: http://scholar.google.com/scholar_lookup?&title=Microsoft%20concept%20graph%3A%20mining%20semantic%20concepts%20f
or%20short%20text%20understanding&journal=Data%20Intelligence&doi=10.1162%2Fdint_a_00013&volume=1&issue=3&pages=238-270&
publication_year=2019&author=Ji%2CL&author=Wang%2CY&author=Shi%2CB&author=Zhang%2CD&author=Wang%2CZ&author=Yan%2CJ
[117]: https://doi.org/10.1016%2Fj.artint.2012.07.001
[118]: http://www.ams.org/mathscinet-getitem?mr=2988877
[119]: http://scholar.google.com/scholar_lookup?&title=Babelnet%3A%20the%20automatic%20construction%2C%20evaluation%20an
d%20application%20of%20a%20wide-coverage%20multilingual%20semantic%20network&journal=Artif%20Intell.&doi=10.1016%2Fj.art
int.2012.07.001&volume=193&pages=217-250&publication_year=2012&author=Navigli%2CR&author=Ponzetto%2CSP
[120]: http://scholar.google.com/scholar_lookup?&title=Language%20models%20are%20unsupervised%20multitask%20learners&jou
rnal=OpenAI%20blog&volume=1&issue=8&publication_year=2019&author=Radford%2CA&author=Wu%2CJ&author=Child%2CR&author=Luan%
2CD&author=Amodei%2CD&author=Sutskever%2CI
[121]: https://doi.org/10.1162%2Ftacl_a_00370
[122]: http://scholar.google.com/scholar_lookup?&title=Did%20Aristotle%20use%20a%20laptop%3F%20A%20question%20answering%
20benchmark%20with%20implicit%20reasoning%20strategies&journal=Trans%20Assoc%20Comput%20Linguist.&doi=10.1162%2Ftacl_a_0
0370&volume=9&pages=346-361&publication_year=2021&author=Geva%2CM&author=Khashabi%2CD&author=Segal%2CE&author=Khot%2CT&a
uthor=Roth%2CD&author=Berant%2CJ
[123]: http://scholar.google.com/scholar_lookup?&title=Neurocompositional%20computing%3A%20from%20the%20central%20parado
x%20of%20cognition%20to%20a%20new%20generation%20of%20ai%20systems&journal=AI%20Mag.&volume=43&issue=3&pages=308-322&pub
lication_year=2022&author=Smolensky%2CP&author=McCoy%2CR&author=Fernandez%2CR&author=Goldrick%2CM&author=Gao%2CJ
[124]: https://citation-needed.springer.com/v2/references/10.1007/s12559-024-10345-6?format=refman&flavour=references
[125]: /search?sortBy=newestFirst&contributor=Qian%20Liu
[126]: https://www.ncbi.nlm.nih.gov/entrez/query.fcgi?cmd=search&term=Qian%20Liu
[127]: https://scholar.google.co.uk/scholar?as_q=&num=10&btnG=Search+Scholar&as_epq=&as_oq=&as_eq=&as_occt=any&as_sautho
rs=%22Qian%20Liu%22&as_publication=&as_ylo=&as_yhi=&as_allsubj=all&hl=en
[128]: /search?sortBy=newestFirst&contributor=Sooji%20Han
[129]: https://www.ncbi.nlm.nih.gov/entrez/query.fcgi?cmd=search&term=Sooji%20Han
[130]: https://scholar.google.co.uk/scholar?as_q=&num=10&btnG=Search+Scholar&as_epq=&as_oq=&as_eq=&as_occt=any&as_sautho
rs=%22Sooji%20Han%22&as_publication=&as_ylo=&as_yhi=&as_allsubj=all&hl=en
[131]: /search?sortBy=newestFirst&contributor=Erik%20Cambria
[132]: https://www.ncbi.nlm.nih.gov/entrez/query.fcgi?cmd=search&term=Erik%20Cambria
[133]: https://scholar.google.co.uk/scholar?as_q=&num=10&btnG=Search+Scholar&as_epq=&as_oq=&as_eq=&as_occt=any&as_sautho
rs=%22Erik%20Cambria%22&as_publication=&as_ylo=&as_yhi=&as_allsubj=all&hl=en
[134]: /search?sortBy=newestFirst&contributor=Yang%20Li
[135]: https://www.ncbi.nlm.nih.gov/entrez/query.fcgi?cmd=search&term=Yang%20Li
[136]: https://scholar.google.co.uk/scholar?as_q=&num=10&btnG=Search+Scholar&as_epq=&as_oq=&as_eq=&as_occt=any&as_sautho
rs=%22Yang%20Li%22&as_publication=&as_ylo=&as_yhi=&as_allsubj=all&hl=en
[137]: /search?sortBy=newestFirst&contributor=Kenneth%20Kwok
[138]: https://www.ncbi.nlm.nih.gov/entrez/query.fcgi?cmd=search&term=Kenneth%20Kwok
[139]: https://scholar.google.co.uk/scholar?as_q=&num=10&btnG=Search+Scholar&as_epq=&as_oq=&as_eq=&as_occt=any&as_sautho
rs=%22Kenneth%20Kwok%22&as_publication=&as_ylo=&as_yhi=&as_allsubj=all&hl=en
[140]: mailto:cambria@ntu.edu.sg
[141]: https://s100.copyright.com/AppDispatchServlet?title=PrimeNet%3A%20A%20Framework%20for%20Commonsense%20Knowledge%2
0Representation%20and%20Reasoning%20Based%20on%20Conceptual%20Primitives&author=Qian%20Liu%20et%20al&contentID=10.1007%2
Fs12559-024-10345-6&copyright=The%20Author%28s%29%2C%20under%20exclusive%20licenc

[Content truncated]
```
