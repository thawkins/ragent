# Web source

- URL: http://kv-emptypages.blogspot.com/2021/01/adding-commonsense-reasoning-to-natural.html
- Title: # [ eMpTy Pages ][1]
- Captured (UTC): 2026-06-29T16:30:32.627594091+00:00

```text
# [ eMpTy Pages ][1]

Comments about translation technology, new collaboration models, and inspiration

## Pages
* [Home][2]
* [About][3]

## Tuesday, January 19, 2021

### [Adding Commonsense Reasoning to Natural Language Processing Applications][4]

This article is reprinted with permission from the original poster [@VeredShwartz][5] . This post might be challenging
reading for the usual reader of this blog, but I think that even skimming through this might be useful to many to get a
sense for possibly the most formidable challenge in the artificial intelligence community: building common sense
capabilities into existing and emerging AI deployments.  

Commonsense knowledge consists of facts about the everyday world, that all humans are expected to know. Commonsense
knowledge helps to solve problems in the face of incomplete information. It is currently considered an unsolved problem
in [AGI][6] and is a focus of the [Allen Institute for Artificial Intelligence][7] which the author is associated with. 

[Deep learning][8] is self-education for machines; you feed a machine learning system huge amounts of data, and
eventually it begins to discern patterns all by itself.  But despite their remarkable achievements, and occasional
ability to produce human-like outputs, machine learning algorithms are at their core complex mathematical functions that
map observations to outcomes. Or are able to forecast patterns that they have previously seen and explicitly learned.
Therefore, they’re as good as their data and they start to break as the data they face in the world starts to deviate
from examples they’ve seen during training. Neural MT is an example, great progress indeed, but far from having solved
the translation problem.  

We hear continuously about the relentless "big data" that is driving AI progress, but we are finding more and more cases
where the current approach of deep learning and more data is not enough. The path to machine commonsense is unlikely to
be brute force training of larger neural networks with deeper layers on more data.  Whilst deep learning excels at
pattern recognition, it’s very poor at adapting to changing situations even when small modifications of the original
case are encountered, and often has to be re-trained with large amounts of data from scratch. 

"**The great irony of common sense—and indeed AI itself—is that it is stuff that pretty much everybody knows, yet nobody
seems to know what exactly it is or how to build machines that possess it,**" said [Gary Marcus][9], CEO and founder of
Robust.AI. "Solving this problem is, we would argue, the single most important step towards taking AI to the next level.
Common sense is a critical component to building AIs that can understand what they read; that can control robots that
can operate usefully and safely in the human environment; that can interact with human users in reasonable ways. [Common
sense is not just the hardest problem for AI][10]; in the long run, it's also the most important problem." 

Common sense has been called the [“dark matter of AI][11]” — both essential and frustratingly elusive. That’s because
common sense consists of implicit information — the broad (and broadly shared) set of unwritten assumptions and rules of
thumb that humans automatically use to make sense of the world. Critics of over-exhuberant AI claims frequently point
out that two-year children have more common sense than existing deep-learning based AI systems whose "understanding" is
often quite brittle and easily distracted and deranged.

**Common sense is easier to detect than to define. The implicit nature of most common-sense knowledge makes it difficult
and tedious to represent explicitly. **

[DARPA,][12] the US defense department’s research agency, has also recognized the absence of common sense as being an
important issue. They recently launched a project called Machine Common Sense. As they say:“ The absence of common sense
prevents intelligent systems from understanding their world, behaving reasonably in unforeseen situations, communicating
naturally with people, and learning from new experiences. Its absence is considered the most significant barrier between
the narrowly focused AI applications of today and the more general, human-like AI systems hoped for in the future”. 

Gary Marcus suggests combining traditional AI approaches together with deep learning as a way forward. 

> "First, classical AI actually IS a framework for building cognitive models of the world that you can then make
> inferences over. The second thing is, classical AI is perfectly comfortable with rules. It’s a strange sociology right
> now in deep learning where people want to avoid rules. They want to do everything with neural networks, and do nothing
> with anything that looks like classical programming. But there are problems that are routinely solved this way that
> nobody pays attention to, like making your route on Google maps.
> 
> We actually need both approaches. The machine-learning stuff is pretty good at learning from data, but it’s very poor
> at representing the kind of abstraction that computer programs represent. Classical AI is pretty good at abstraction,
> but it all has to be hand-coded, and there is too much knowledge in the world to manually input everything. So it
> seems evident that what we want is some kind of synthesis that blends these approaches."



[Yejin Choi][13] and her collaborators at the Allen Institute have united traditional symbolic AI approaches with newer
machine learning approaches in an attempt to address the commonsense challenge. One initiative, [COMET][14] (short for
“commonsense transformers”) extends traditional symbolic reasoning with the latest advances in neural language modeling
— a kind of deep learning that aims to imbue computers with a [statistical “understanding” of written language][15].
COMET is a  fusion of symbolic reasoning with a neural network and tries to solve the coverage and brittleness problems,
of purely DL-approaches, at the same time.  COMET works by reimagining common-sense reasoning as a process of generating
plausible (if imperfect) responses to novel input, rather than making airtight deductions by consulting a vast
encyclopedia-like database.

Gary Marcus, a critic of the deep-learning fanboys and girls, often points out DL-only shortcomings to challenge the
over-exhuberance of these fans. To put progress in AI into a more realistic context he says: “Just because you can build
a better ladder doesn’t mean you can build a ladder to the moon.” To him and others, COMET’s approach suffers from a
fundamental limitation of deep learning: “[statistics ≠ understanding][16].”

Regardless, Vered presents a comprehensive picture of the many challenges faced and attempts at developing solutions in
introducing commonsense to NLP applications in arguably one of the most challenging problems in computing today. I
think  her post is a great resource for anybody who wants to quickly get a sense for the issue and the SOTA.





****** 

### Commonsense Reasoning for Natural Language Processing

This long-overdue blog post is based on the [Commonsense Tutorial][17] taught by Maarten Sap, Antoine Bosselut, Yejin
Choi, Dan Roth, and myself at [ACL 2020][18]. Credit for much of the content goes to the co-instructors, but any errors
are mine. 

In the last 5 years, popular media has made it seem that AI is nearly---if not already---solved by deep learning, with
reports on super-human performance on speech recognition, image captioning, and object recognition. The release of
Google Translate’s neural models in 2016 [reported][19] large performance improvements: “60% reduction in translation
errors on several popular language pairs”. But looking under the hood, these numbers seem to be misleading. Neural
models find shortcuts to the correct answers through [dataset-specific input-output correlations][20], essentially
solving the dataset but not the underlying task. When models are challenged with adversarial out-of-domain examples,
they perform poorly. Small unnoticeable noise added to images [confuses object recognition models and changes their
predictions][21]. Visual question answering models [guess the answer][22] based on the frequency of answers for the same
type of question in the training set, e.g. replying "2" to any "how many" question. Image captioning models often [learn
to recognize objects based solely on their typical environment][23] and fail to recognize them outside their typical
environment. In NLP, dialogue systems generate [highly generic responses][24] such as “I don’t know” even for simple
questions. Open-ended generation is [prone to repetition][25]. Question answering systems are [easily distracted by the
addition of an unrelated sentence][26] to the passage. And more. 


───────────────────────────────────────────────────────────────────────────────────────────────────────
Figure 1: adversarial examples in computer vision (left) and natural language processing tasks (right).
───────────────────────────────────────────────────────────────────────────────────────────────────────

Machine learning models today perform reasonably well on perception tasks (image and speech recognition). However, they
mostly lack the ability to perform simple intuitive commonsense inferences that humans do in every minute of their
waking hours, regarding pre-and post-conditions of events, understanding other people's motivations and intents, mental
and emotional states, etc. 


#### **Table of contents:** 
1. What is commonsense? 
2. Is commonsense knowledge already captured by pre-trained language models? 
3. How to create benchmarks to measure commonsense reasoning capabilities? 
4. How to gather and represent machine-readable commonsense knowledge? 
5. How to enhance neural models for commonsense reasoning tasks with symbolic knowledge? 
6. Summary

**What is commonsense? **
The boundaries of commonsense are quite challenging to define, but we will go with this working definition:

> Commonsense is the basic level of **practical knowledge** and **reasoning** concerning everyday **situations** and
> **events** that are **commonly** shared among **most** people. 

For example, it's common sense that it's OK to keep the closet door open, but not the fridge door, as the food inside
might go bad. 

**Types of commonsense: **

Commonsense knowledge can be categorized according to types, including but not limited to:
* **Social commonsense: **people are capable of making inferences about other people's mental states, e.g. what
  motivates them, what they are likely to do next, etc. This kind of inference is captured by the [ATOMIC][27] knowledge
  base discussed later. In addition, we each have a set of social norms of accepted behavior, e.g. knowing that “it's
  impolite to comment on someone's weight”. While these are often implicit in our actions and decisions, machines need
  to be taught them [explicitly][28]. 
  
* **Temporal commonsense: **natural language rarely communicates explicit temporal information. Instead, it's vague and
  relies on the commonsense knowledge of the listener. For example, when told that "Dr. Porter is taking a vacation" we
  can predict that Dr. Porter **will not** **be** able to see us soon, as opposed to when "Dr. Porter is taking a walk".
  This requires knowing the typical duration of "taking a walk" (minutes) and that of "taking a vacation" (days). Other
  temporal knowledge is typical times, order, frequency, etc. of events which are addressed by the [MC-TACO][29] dataset
  and the [TACO-LM][30] time-aware contextual language model. 
  
* **Physical commonsense: **a glass will likely shatter if it falls to the floor, which is a fact most people (and
  [arguably cats][31]) know. Physical commonsense includes knowledge about the physical properties and affordances of
  everyday objects, as tested in the [PIQA][32] dataset.
Commonsense is essential for humans to navigate everyday situations seamlessly and interact with each other in a
reasonable and safe way, and for AI to understand human needs and actions better. Yet, endowing machines with such
human-like commonsense reasoning capabilities has remained an elusive goal of AI research for decades. Past attempts, in
the 1960s and 1970s, resulted in an AI winter, i.e. reduced interest and funding for AI research due to failed
over-hyped research directions. In recent years, a new interest in machine commonsense has emerged, with the
availability of stronger computing power and huge amounts of data. With that said, the path to machine commonsense is
unlikely to be brute force training larger neural networks with deeper layers.   


#### Is commonsense knowledge already captured by pre-trained language models?

In the last 3 years, language models have been ubiquitous in NLP. [Language models][33] are pre-trained once, in a
self-supervised manner that requires only a large text corpus. Traditionally, language models are trained to predict the
next word in a sentence (top part of Figure 2, in blue), but they can also predict hidden (masked) words in the middle
of the sentence, as in Google's [BERT][34] model (top part of Figure 2, in orange). This pre-training phase yields a
function that gets a sequence of words (sentence, short paragraph) and returns a vector for each word in the sequence. 
  


───────────────────────────────────────────────────────
Figure 2: Language models pre-training and fine-tuning.
───────────────────────────────────────────────────────


As opposed to word embeddings which are static, language model-based word vectors are dynamic and re-computed for each
context. At the very basic level, they assign different vectors to words when they are used in different senses, as in
Figure 3. 



──────────────────────────────────────────────────
Figure 3: Static vs. dynamic word representations.
──────────────────────────────────────────────────


**Do off-the-shelf pre-trained language models already capture commonsense knowledge? **

✅  They are capable to some extent, of [filling incomplete commonsense facts][35] or [ranking candidate facts][36]. For
example, the language model score (≈ statement plausibility) of a fact like "a musician plays a musical instrument" is
higher than "a dancer plays a musical instrument". This is a proof that, in addition to lexical and syntactic knowledge,
language models capture general knowledge about the world.  

✅  They can, to some extent, [associate concepts with their properties][37]. They distinguish concepts associated with
a given set of properties, i.e. complete a statement such as "A        has fur, is big, and has claws, has teeth, is an
animal, ..." with bear (just like playing the "20 question game"). They perform better when they are shown encyclopedic
properties (e.g. is an animal) as opposed to perceptual properties (e.g. smooth). They can also, pretty successfully,
list the properties associated with given concepts, e.g. complete the sentence "Everyone knows that a bear has       "
with fur, claws, teeth, etc. 

However, knowledge generated from language models is noisy! 

🚫 [Several][38] [papers][39] have shown that language models are not sensitive to negation, i.e. they consider the
negated version of facts ("birds can't fly") as similarly plausible. 

🚫 They are sensitive to phrasing:


🚫  In [distributional word vectors][40], the vector representing a (sub-)word is learned from the contexts in which it
appeared, leading to similar representation for semantically-similar words. In language models, the representation of
similar contexts are similar, so the model learns which type of word should appear next (or instead of a masked token).
This is generally a positive thing, but it sometimes [over-generalizes][41], leading to examples such as this: 



────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Figure 4: BERT guesses that the masked token should be a color, but fails to predict the correct color. Using           
the [AllenNLP demo][42].                                                                                                
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


Here, BERT has seen in its training corpus enough sentences of the type "The color of something is [color]" to know to
suggest different colors as substitutes for the masked word. Unfortunately, not every color is suitable in every context
that calls for a color. BERT likely didn't see enough sentences discussing the color of a dove, thus it defaults to just
predicting any color.  

**So knowledge in language models is not the most accurate and reliable. Is it still useful?**

Yes, to some extent. One way to show it is through evaluation on tasks requiring commonsense knowledge. We will discuss
several such tasks, but for now, let's focus on [WinoGrande][43] as an example. It is the large-scale version of the
[Winograd Schema Challenge][44]. Given a sentence with a cloze, the goal is to fill in the blank with a previously
mentioned entity or concept, out of two answer choices. For example: 

Because Brett found an internship while in college but Ian was unable to, _____ found a job less quickly after
graduation. 
Choices: Brett, Ian

What makes this task especially difficult is that every instance has a twin sentence which is minimally changed such
that the correct answer is the other one (for instance, replacing "less quickly" with "more quickly" will change the
correct answer from Ian to Brett). 

Language model-based models top the leaderboards of WinoGrande and other commonsense tasks, but since they are trained
on task-specific training data, which often contains tens or hundreds of thousands of training examples, it's hard to
attribute the success to the knowledge captured in language models from the pre-training step. A better way to estimate
it is with zero-shot (unsupervised) models. Typically, the way zero-shot models address multiple-choice tasks is by
phrasing a statement from the instance and each answer choice, and computing the language model score as a proxy for
plausibility:

PLM(The answer is answer1) 
PLM(The answer is answer2) 
...
PLM(The answer is answerk)

And then predicting the answer choice with the best language model score (highest probability, which is usually computed
as the lowest [perplexity][45]). 

In our [recent EMNLP paper][46], we took it one step further and asked whether we can use language models to generate
what would otherwise be missing or implicit knowledge needed for solving a multiple-choice commonsense question
answering instance. We proposed the unsupervised "self-talk" framework, that uses language models to generate
information-seeking questions such as "what is the definition of..." and their corresponding answers (clarifications) to
discover additional background knowledge. In the example in Figure 5, knowing that internship experience may help a
person get a job is crucial for answering the question (which of Brett and Ian found a job less quickly?). On most
benchmarks, the self-talk model performed better than unsupervised models with no additional knowledge, while competing
with models that have access to knowledge bases. This is despite the inaccurate and noisy knowledge language models
generate. However, when we showed people some of the clarifications that helped the model choose the correct answer
choice, they judged only 40% of them as actually providing helpful information. This discrepancy means that our model
doesn't imitate the human reasoning process - it works differently. Check out our [demo][47]! It's not always accurate
but it's often funny :) 


───────────────────────────────────────────────────────────────────────────────────
Figure 5: An example of clarification generation for an instance from WinoGrande.  
                                                                                   
───────────────────────────────────────────────────────────────────────────────────

The best performance on commonsense tasks is achieved by fine-tuning language models, i.e. training them on
task-specific data. Let's look at some of the benchmarks and the issues we face with supervised learning.  


#### How to measure commonsense reasoning capabilities? 

Multiple commonsense benchmarks have been released over the last few years. Some of them will be discussed here (see
examples in Figure 6), along with the main differences and design choices when creating a benchmark.


──────────────────────────────────────────────────────────────────────
Figure 6: Some commonsense benchmarks along with an example instance. 
──────────────────────────────────────────────────────────────────────


**Type of knowledge:** some benchmarks focus on a specific type of commonsense knowledge, such as social commonsense
(e.g. [Social IQa][48]),  physical commonsense (e.g. [PIQA][49]), temporal commonsense (e.g. [MC-TACO][50]),  or causes
and effects (e.g. [COPA][51]), while others target a broader domain of general commonsense knowledge and reasoning (e.g.
[WSC][52], [WinoGrande][53], [CommonsenseQA][54], [ROCStories][55]).  

**Size: **most recent datasets include a large training set, in order to facilitate training large neural models. One
way to create a benchmark is to hire experts to curate a high-quality dataset such as for WSC and COPA. These datasets
are rather expensive to collect and are therefore typically small. The common alternative is to collect data through
[crowdsourcing][56] or semi-automatically, and split it randomly to train, validation, and test sets. Models that
learned data-specific shortcuts in the training set instead of generalized phenomena are likely to perform well on a
test set [drawn from the same distribution][57], but this performance is misleading and is likely a lot better than on
real-world instances of the task.  Despite this understanding, this is still the dominant approach. 

**Format:** the vast majority of datasets are in the format of multiple-choice questions, as exemplified in Figure 6.
This format is the easiest to evaluate automatically: models are judged for their accuracy, i.e. what percent of the
questions they answered correctly. Unfortunately, this type of tasks also makes it possible for a model to guess the
correct answer. We're not talking about a random guess, which would leave enough room for improvement. A random guess is
expected to result in an accuracy of 100/k %, where k is the number of answer choices, e.g. 50% accuracy for binary
tests, 33.3% for tests with 3 choices, 25% for 4 choices, etc. The risk is that the model makes an "educated guess"
based on - yes, you guessed it correctly - spurious correlations between the questions and the correct/incorrect
answers. 

How do you make sure a model is right for the right reasons?

That's the million-dollar question. We don't have a perfect solution for this problem yet. For a start, when collecting
a new benchmark, the process of collecting incorrect answers (=distractors) should be well-designed such that
distractors are plausible but unlikely. Using random answers as distractors (e.g. naturally-occurring sentences or
correct answers of different questions) would create topically-different distractors, which are easy to detect
(remember, [relatedness is one of the strengths of distributional text representations][58]). Asking people to come up
with the distractors may introduce other annotation artifacts, such as exaggerations, going off-topic, or producing
overly emotional texts, which are [easy for models to detect][59]. Some solutions have been proposed: for example, the
distractors in Social IQa are answers for different questions asked on the same context. In Figure 7, the context "Alex
spilt food all over the floor and it made a huge mess." appears in the dataset with two questions: "what happens next?"
and "what happened before?". The distractors of "what happens next?" are the correct answers of "what happened before?",
e.g. that Alex has slippery hands. A similar approach is taken in CommonsenseQA. 


────────────────────────────────────────────────────────────────────────────────────
Figure 7: Creating distractors for a Social IQa instance. Image credit: Maarten Sap.
────────────────────────────────────────────────────────────────────────────────────

An alternative solution is to filter out easy questions through "[adversarial filtering][60]", i.e. training a weaker
model and iteratively removing instances that it succeeds in answering. Variants of adversarial filtering were applied
to WinoGrande and PIQA. 

Finally, I believe the future is in generative tasks, in which the model needs to produce a free-text answer without
being provided with the candidate answers. Several recent benchmarks are generative, such as [TimeTravel][61]
(counterfactual reasoning), [ART][62] (abductive reasoning), [CommonGen][63], and [ProtoQA][64]. The challenge in
generative tasks is the lack of reliable automatic evaluation metrics. Given the gold standard reference answer(s), we
would like a metric to (1) reward correct generated answers that are different from the reference answer, while (2)
penalizing incorrect answers that are similar (e.g. lexically) to the reference. Human evaluation is reliable, but it is
costly and is typically done once on the test set. In order to be able to improve models during development, we need
automatic metrics. We currently settle for metrics based on lexical overlap such as [BLEU][65] and [ROUGE][66] which are
pretty terrible at (1) and have [little correlation with human judgments][67], or model-based metrics such as [BERT
score][68] that are not great at (2). 


#### How to gather and represent machine-readable commonsense knowledge?

Commonsense resources provide machine-readable knowledge about the world. Resources are expected to be large-scale and
accurate, consist of diverse knowledge types, and be usable in downstream tasks. [ConceptNet][69] is a large (21 million
assertions), commonly-used resource consisting of general commonsense knowledge, in over 85 languages. [ATOMIC][70]
consists of 880,000 triplets reasoning about causes and effects of everyday situations. Other resources are listed in
Figure 8.


─────────────────────────────────────────────────────────────────────────────────
Figure 8: Overview of existing commonsense resources. Image credit: Maarten Sap. 
─────────────────────────────────────────────────────────────────────────────────


Existing resources differ in several aspects:

**Representation**: how is knowledge represented in the resource? ConceptNet and ATOMIC represent knowledge in natural
language (Figure 9), while NELL and Cyc represent knowledge in symbolic logic:

(#$implies (#$and (#$isa ?OBJ ?SUBSET) (#$genls ?SUBSET ?SUPERSET)) (#$isa ?OBJ ?SUPERSET)) 



─────────────────────────────────────────────────────────────────────────────────────────────
Figure 9: example knowledge extracted from ConceptNet and ATOMIC. Image credit: Maarten Sap. 
─────────────────────────────────────────────────────────────────────────────────────────────

**
**
**Knowledge type: **ConceptNet consists of semantic knowledge, i.e. properties of concepts (e.g. reading is a type of
activity). ATOMIC, on the other hand, is inferential: given a templated event with "PersonX" representing the subject
and "PersonY" an optional object(s) (e.g. PersonX yells at PersonY), and one of 9 pre-defined relation dimensions (e.g.
PersonX's motivation) it provides a second event (e.g. PersonX wanted to express anger). 

**Collection method:** knowledge can be collected from humans, either experts or crowdsourcing workers. Expert-curated
resources are more uniform and accurate and may use complex representations, but it is an expensive collection method,
and it is very [time-consuming][71]. Alternatively, non-experts can write knowledge in natural language, making the
collection faster and more scalable.

The alternative approach is to extract knowledge automatically from texts, as in [NELL][72]. This approach works, but it
produces less accurate knowledge. In addition, the approach suffers from [reporting bias][73]: over-representing the
rare at the expense of the trivial. For example, people are reported to murder more often than they are reported to
breathe. Default properties of concepts (yellow banana) are mentioned less often than their alternatives (green banana),
etc. 


**How to enhance neural models for commonsense reasoning tasks with symbolic knowledge?**

#### Most models developed for solving commonsense benchmarks today are based on language models. Typically, each answer
#### choice, along with the context, forms a statement. The language model computes a vector representing each
#### statement. These vectors are then fed into a classifier that assigns a plausibility score for each candidate
#### answer:

**
**

#### ──────────────────────────────────────────────────────────────────────────────────────────────
#### [****][74]                                                                                    
#### ──────────────────────────────────────────────────────────────────────────────────────────────
#### Figure 10: An illustration of using BERT to score the answer choices of a WinoGrande instance.
#### ──────────────────────────────────────────────────────────────────────────────────────────────


#### Static neuro-symbolic integration

The knowledge in commonsense resources may enhance models built for solving commonsense benchmarks. For example, we can
extract from ConceptNet the assertions that a job is used for making money, that spending money requires making money,
that buying requires spending money and that car is something you can buy. Ideally, we would also need the knowledge
that a high-paying job is a type of job, specifically one used for making a lot of money, which is required for spending
a lot of money, which is required for buying something that costs a lot of money, a car being one of them. Finally, we
may want to remove the edge from "buy" to "car" so we can only get to "car" from the node "buy something that costs a
lot of money". 



────────────────────────────────────────────────────────────────────────────────────────────
Figure 12: Knowledge extracted from ConceptNet for the WinoGrande instance discussed above. 
────────────────────────────────────────────────────────────────────────────────────────────


How do we incorporate knowledge from knowledge resources into a neural model?

The simple recipe (success not guaranteed) calls for 4 ingredients: the task addressed, the knowledge resource used, the
neural component, and the combination method. We have already discussed tasks and knowledge resources, so I would only
add here that ConceptNet is the main resource utilized for downstream models, although some models incorporate other
knowledge sources, such as other knowledge bases (WordNet, ATOMIC), knowledge mined from text, and tools (knowledge base
embeddings, sentiment analysis models, COMET - see below). 



────────────────────────────────────────────────────────────────────────
Figure 13: Resources used by most knowledge-informed commonsense models.
────────────────────────────────────────────────────────────────────────

The neural component is the shiny new neural architecture - language models in the last 3 years, biLSTMs in the years
prior, etc. The more interesting component is the combination method. We will look at 3 examples:

**Incorporating into the scoring function:** [Lin et al. (2017)][75] extracted probabilistic "rules" connecting pairs of
terms from multiple sources such as WordNet (restaurant→eatery: 1.0), Wikipedia categories (restaurant→business: 1.0),
script knowledge mined from text (X went to a restaurant→X ate: 0.32), word embedding-based relatedness scores
(restaurant→food: 0.71), and more. The model scores each candidate answer according to the scores of the inference rules
used to get from the context (e.g. "Mary walked to a restaurant" in Figure 14) to the candidate answer (e.g. "She
ordered foods.").  



────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Figure 14: "covering" each candidate answer by the original context and the rules extracted from various sources. Image 
credit: [Lin et al. (2017)][76].                                                                                        
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


**Representing symbolic knowledge as vectors: **[Lin et al. (2019)][77] used BERT as the neural component to represent
the instance (statement vector). For their symbolic component, they extracted subgraphs from ConceptNet pertaining to
concepts mentioned in the instance and learned to represent them as a vector (graph vector). These two vectors were
provided as input to the answer scorer which was trained to predict the correct answer choice. 


────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Figure 15: extracting subgraphs from ConceptNet pertaining to concepts mentioned in the instance. Image credit: [Lin et 
al. (2019)][78].                                                                                                        
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


#### Multi-task learning: [Xia et al. (2019)][79] fine-tuned a BERT model to solve the multiple-choice questions. They
#### also trained two auxiliary tasks supervised by ConceptNet, in which two concepts were given as input and the
#### classifier had to predict whether they are related or not, and the specific ConceptNet property that connects them.
#### The BERT model was shared between the main and the auxiliary tasks, so that commonsense knowledge from ConceptNet
#### was instilled into BERT, improving its performance on the main task.


───────────────────────────────────────────────────────────────────────────────────────
Figure 16: multi-task learning aimed at instilling knowledge from ConceptNet into BERT.
───────────────────────────────────────────────────────────────────────────────────────


#### Dynamic neuro-symbolic integration

There are two main limitations to the neuro-symbolic integration discussed above:
1. **Coverage:** relevant knowledge is often not found as-is in commonsense knowledge resources. As we've seen earlier,
   commonsense knowledge is immeasurably vast, so much of it is not documented. 
   
2. **Precision and context:** knowledge found in the knowledge base about concept X doesn't necessarily apply to all
   contexts in which X appears. For example, when provided with "PersonX adopts a cat", ATOMIC says that PersonX had to
   go to the shelter first (Figure 17), but that's not always the case. It may as well be that PersonX adopted a cat
   they found on the street or got the cat from a friend who was no longer able to care for it. 


───────────────────────────────────────────────────────────────────
Figure 17: ATOMIC inferences for the event "PersonX adopted a cat".
───────────────────────────────────────────────────────────────────


How do we provide machines with large-scale, contextualized commonsense knowledge?

The solution is to leverage manually curated commonsense knowledge resources, such as ConceptNet and ATOMIC, to train a
model that can dynamically produce such knowledge for a given context. Commonsense knowledge resources are typically
sparse, making training a knowledge base completion model to extend the resource less efficient. Pre-trained language
models and their inherent knowledge come in handy here. Language models (such as GPT) implicitly represent knowledge, so
you can re-train them on completing knowledge base assertions (e.g. from ATOMIC) to teach them the structure of
knowledge. This is what [COMET][80] (COMmonsEnse Transformers) does, as illustrated in Figure 18. 



────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
Figure 18: Illustration of the training process of COMET: The language model is fine-tuned to predict the "tail entity" 
(e.g. inference in ATOMIC) given the "head entity" and the relation. Image credit: Antoine Bosselut.                    
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────


COMET is capable of dynamically generating inferences for any context. For example, if we modify the context from ATOMIC
to "David adopted his sister's cat because they found out her husband was allergic.", which for obvious reasons does not
appear in ATOMIC, COMET no longer predicts that PersonX (David) had to go to the shelter, but instead that he, for
example, needed to find out about it.

COMET has been used successfully in various downstream tasks requiring commonsense knowledge. Models trained on ATOMIC
or on ConceptNet are available, and the demo for both ATOMIC and COMET can be found [here][81]. There is also a [Visual
COMET][82] that can generate inferences from images. 


#### Summary

We talked about ways to acquire and represent commonsense knowledge in machine-readable format, ways to measure
commonsense reasoning abilities, and ways to integrate this kind of knowledge into models. None of these is solved yet.
Manually collecting all the commonsense knowledge is infeasible, while extracting it from texts or from language models
suffers from inaccuracies, reporting bias, and societal biases. Looking forward, a promising research direction is
multi-modal commonsense knowledge acquisition, e.g. learning from texts along with images and videos. For example,
looking through enough class photos, you might learn that the kids in the front row typically sit (especially if the
kids in the last row are also seated). 


Machines may reach human performance on commonsense benchmarks but it's often due to being right for the wrong reasons
rather than actually possessing and successfully applying commonsense knowledge and reasoning abilities. Generative
tasks are somewhat less prone to this issue, but we would have to develop reliable automatic evaluation metrics to make
them the standard. 

Machine commonsense reasoning is becoming more and more popular within NLP so I am optimistic about future
breakthroughs! 
Posted by Kirti Vashee at [12:09 PM][83]
Labels: [artificial intelligence][84], [commonsense][85], [NLP][86]

#### No comments:



#### Post a Comment


[Newer Post][87] [Older Post][88] [Home][89]
Subscribe to: [Post Comments (Atom)][90]

##### Get new posts by email:

Subscribe

 [Subscribe in a reader][91]

[[My photo]][92]

* * [View my complete profile][93]


## LinkedIn

[ [View Kirti Vashee's profile on LinkedIn] ][94]

## Twitter X Timeline

[Tweets by @kvashee][95]

## Tweets

[Tweets by kvashee][96]

## Recent Comments

Get [Recent Comments ][97]

## Popular Posts
* [The Changing Face of Localization (Professional Translation)][98]
* [The Premium Translation Market: Hiding In Plain Sight][99]
* [The Expanding Translation Market Driven by Expert Based MT][100]
* [Improving the MT Technology to Translator Dialogue][101]
* [Translator Strategies For Dealing With PEMT][102]
* [Neural MT Technology Looking for a Business Model][103]
* [Human Evaluation of Machine Translation Output][104]
* [Understanding Machine Translation Quality & Risk Prediction][105]
* [The Translation Market – Is it Really Understood?][106]
* [The Google Neural Machine Translation Marketing Deception][107]

## Labels

[Asia][108] [BLEU][109] [Controlled Language][110] [Deception][111] [Google][112] [Internet trends][113] [Moses][114]
[Neural MT][115] [Neural Machine Translation][116] [Post-editing][117] [Systran][118] [Translation Industry][119]
[collaboration][120] [customer care][121] [human evaluation][122] [humor][123] [information quality][124]
[innovation][125] [localization][126] [standards][127] [statistical MT][128] [translation quality][129] [translation
technology][130]

───────────────────────────────────────────────────────────────────────────────────────────┬────────────────────────────
## Search This Blog                                                                        │* [ Brain Pickings][407]    
                                                                                           │  [ Legendary Artist Sheila 
                                                                                           │  Hicks, at 92, on the      
## Pages                                                                                   │  Secret to Creative        
* [Translated Srl][131]                                                                    │  Vitality ][408]           
* [ModernMT Blog][132]                                                                     │* [ Masterword              
                                                                                           │  Services][409]            
## Featured Post                                                                           │  [ Cultural Humility vs.   
                                                                                           │  Cultural Competency in    
### [Comparing MT System Performance][133]                                                 │  Healthcare ][410]         
                                                                                           │* [ Google Translate][411]  
  The advantages of a dynamic adaptive MT system are clarified in this post. Most static MT│  [ Fluid, natural voice    
systems need significant upfront investment to ...                                         │  translation with Gemini   
                                                                                           │  3.5 Live Translate ][412] 
[Empty Pages on FaceBook][134]                                                             │* [ John Hagel][413]        
                                                                                           │  [ Inspiration from the    
* [ ►  ][135] [ 2025 ][136] (2)                                                            │  Dalai Lama ][414]         
  * [ ►  ][137] [ December ][138] (1)                                                      │* [ SYSTRAN Blog][415]      
  * [ ►  ][139] [ April ][140] (1)                                                         │  [ What Is Quebec’s Bill   
* [ ►  ][141] [ 2024 ][142] (5)                                                            │  96? ][416]                
  * [ ►  ][143] [ December ][144] (5)                                                      │* [ zen habits][417]        
* [ ►  ][145] [ 2023 ][146] (9)                                                   

[Content truncated]
```
