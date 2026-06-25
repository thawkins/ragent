# Web source

- URL: https://pmc.ncbi.nlm.nih.gov/articles/PMC7748156
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-25T05:52:54.280271753+00:00

```text
[ Skip to main content ][1]

An official website of the United States government

Here's how you know
Here's how you know

**Official websites use .gov**
A **.gov** website belongs to an official government organization in the United States.

**Secure .gov websites use HTTPS**
A **lock** ( [Lock] ) or **https://** means you've safely connected to the .gov website. Share sensitive information
only on official, secure websites.

[ [NCBI home page] ][2]
Search
Log in
* [ Dashboard ][3]
* [ Publications ][4]
* [ Account settings ][5]
* Log out
Search… Search NCBI

Primary site navigation

[Close] Search [Search]

Logged in as: ****
* [ Dashboard ][6]
* [ Publications ][7]
* [ Account settings ][8]

Log in
[PMC search open icon] [PMC search close ison]
Search PMC Full-Text Archive Search in PMC [Search]
* [ Journal List ][9]
* [ User Guide ][10]
* [Open resources icon]
* [ [View on publisher site icon] ][11]
* [ [Download PDF icon] ][12]
* [Collections icon] [Collections icon]
* [Cite icon]
* [Show article permalink icon]
  
  ## PERMALINK
  
  [Copy icon] Copy
[Open article navigation icon]
As a library, NLM provides access to scientific literature. Inclusion in an NLM database does not imply endorsement of,
or agreement with, the contents by NLM or the National Institutes of Health.
Learn more: [PMC Disclaimer][13] | [ PMC Copyright Notice ][14]
[PLOS One logo]
PLoS One
. 2020 Dec 18;15(12):e0243628. doi: [10.1371/journal.pone.0243628][15]
* [Search in PMC][16]
* [Search in PubMed][17]
* [View in NLM Catalog][18]
* [Add to search][19]

# Development of swarm behavior in artificial learning agents that adapt to different foraging environments

[Andrea López-Incera][20]

### Andrea López-Incera

¹Institute for Theoretical Physics, University of Innsbruck, Innsbruck, Austria
Conceptualization, Formal analysis, Investigation, Methodology, Software, Visualization, Writing – original draft,
Writing – review & editing
Find articles by [Andrea López-Incera][21]
^{1,}^{*}, [Katja Ried][22]

### Katja Ried

¹Institute for Theoretical Physics, University of Innsbruck, Innsbruck, Austria
Conceptualization, Formal analysis, Methodology, Supervision, Visualization, Writing – review & editing
Find articles by [Katja Ried][23]
¹, [Thomas Müller][24]

### Thomas Müller

²Fachbereich Philosophie, Universität Konstanz, Konstanz, Germany
Conceptualization, Writing – review & editing
Find articles by [Thomas Müller][25]
², [Hans J Briegel][26]

### Hans J Briegel

¹Institute for Theoretical Physics, University of Innsbruck, Innsbruck, Austria
²Fachbereich Philosophie, Universität Konstanz, Konstanz, Germany
Conceptualization, Supervision, Writing – review & editing
Find articles by [Hans J Briegel][27]
^{1,}²
Editor: Thilo Gross³
* Author information
* Article notes
* Copyright and License information
¹Institute for Theoretical Physics, University of Innsbruck, Innsbruck, Austria
²Fachbereich Philosophie, Universität Konstanz, Konstanz, Germany
³University Of Bristol, UNITED KINGDOM

**Competing Interests: **The authors have declared that no competing interests exist.

^{✉}

* E-mail: andrea.lopez-incera@uibk.ac.at

#### Roles

**Andrea López-Incera**: Conceptualization, Formal analysis, Investigation, Methodology, Software, Visualization,
Writing – original draft, Writing – review & editing
**Katja Ried**: Conceptualization, Formal analysis, Methodology, Supervision, Visualization, Writing – review & editing
**Thomas Müller**: Conceptualization, Writing – review & editing
**Hans J Briegel**: Conceptualization, Supervision, Writing – review & editing
**Thilo Gross**: Editor

Received 2020 Jul 1; Accepted 2020 Nov 24; Collection date 2020.

© 2020 López-Incera et al

This is an open access article distributed under the terms of the [Creative Commons Attribution License][28], which
permits unrestricted use, distribution, and reproduction in any medium, provided the original author and source are
credited.

[PMC Copyright notice][29]
PMCID: PMC7748156  PMID: [33338066][30]

## Abstract

Collective behavior, and swarm formation in particular, has been studied from several perspectives within a large
variety of fields, ranging from biology to physics. In this work, we apply Projective Simulation to model each
individual as an artificial learning agent that interacts with its neighbors and surroundings in order to make decisions
and learn from them. Within a reinforcement learning framework, we discuss one-dimensional learning scenarios where
agents need to get to food resources to be rewarded. We observe how different types of collective motion emerge
depending on the distance the agents need to travel to reach the resources. For instance, strongly aligned swarms emerge
when the food source is placed far away from the region where agents are situated initially. In addition, we study the
properties of the individual trajectories that occur within the different types of emergent collective dynamics. Agents
trained to find distant resources exhibit individual trajectories that are in most cases best fit by composite
correlated random walks with features that resemble Lévy walks. This composite motion emerges from the collective
behavior developed under the specific foraging selection pressures. On the other hand, agents trained to reach nearby
resources predominantly exhibit Brownian trajectories.

## 1 Introduction

Collective behavior is a common but intriguing phenomenon in nature. Species as diverse as locusts, and some families of
fish or birds exhibit different types of collective motion in very different environments and situations. Although the
general properties of swarms, schools and flocks have been widely studied (see e.g. [[1][31]] for a review), the
emergence of global, coordinated motion from the individual actions is still a subject of study. Different approaches,
ranging from statistical physics to agent-based models, have led to new insights and descriptions of the phenomenon.
Statistical physics models are very successful at describing macroscopic properties such as phase transitions and
metastable states [[2][32]–[4][33]], but in order to apply the powerful tools of statistical mechanics, these models
normally simplify the individuals to particles that interact according to certain rules dictated by the physical model
adopted, as for instance the Ising-type interaction of the spins in a lattice. A different type of models are the
so-called self-propelled particle (SPP) models [[5][34]–[8][35]], which enable higher complexity in descriptions at the
individual level but still allow one to employ the tools of statistical physics. They describe individuals as particles
that move with a constant velocity and interact with other individuals via fixed sets of rules that are externally
imposed. In SPP models, the description of the interactions is not restricted to physically accepted first principles,
but can include ad hoc rules based on specific experimental observations.

In this work, we follow a different approach and model the individuals as artificial learning agents. In particular, we
apply Projective Simulation (PS) [[9][36]], which is a model of agency that can incorporate learning processes via a
reinforcement learning mechanism. The individuals are thus described as PS agents that interact with their surroundings,
make decisions accordingly and learn from them based on rewards provided by the environment. This framework allows for a
more detailed, realistic description in terms of the perceptual apparatus of the agent. One of the main differences with
respect to previous models is that the interaction rules between agents are not imposed or fixed in advance, but they
emerge as the result of learning in a given task environment. This type of agent-based models that employ artificial
intelligence to model behavior are gaining popularity in the last few years. Artificial neural networks (ANN) have been
used, for instance, in the context of navigation behaviors [[10][37], [11][38]] and reinforcement learning (RL)
algorithms have been applied to model collective behavior in different scenarios, such as pedestrian movement [[12][39]]
or flocking [[13][40], [14][41]].

In contrast to other learning models such as neural networks, PS provides a transparent, explicit structure that can be
analyzed and interpreted. This feature is particularly useful in modeling collective behavior, since we can study the
individual decision making processes, what the agents learn and why they learn it. This way, we can directly address the
questions of how and why particular individual interactions arise that in turn lead to collective behaviors. Initial
work by Ried et al. [[15][42]], where the authors use PS to model the density-dependent swarm behavior of locusts, laid
the foundations of the present work.

Since the interaction rules are developed by the agents themselves, the challenge is to design the environment and
learning task that will give rise to the individual and, consequently, collective behavior. In previous works, the
agents are directly rewarded for aligning themselves with the surrounding agents [[15][43]] or for not losing neighbours
[[14][44]]. Instead of rewarding a specific behavior, in this work we set a survival task that the agents need to
fulfill in order to get the reward, and then analyze the emergent behavioral dynamics.

As a starting hypothesis, we consider the need to forage as an evolutionary pressure and design a learning task that
consists in finding a remote food source. Due to this particular survival task, our work relates to the investigation of
foraging theories and optimal searching behavior.

There is a vast number of studies devoted to the analysis of foraging strategies in different types of environments
e.g., [[16][45]–[19][46]]. In the particular case of environments with sparsely distributed resources (e.g. patchy
landscapes), there are two main candidates for the optimal search model: Lévy walks [[20][47]–[22][48]] and composite
correlated random walks (CCRW) [[23][49], [24][50]]. The former are described by a single distribution of step lengths
that is characterized by a power-law *p*(*ℓ*) ∼ *ℓ*^{−*μ*} with exponent 1 < *μ* ≤ 3, whereas the latter consider that
the movement is composed of two different modes, characterized by two exponential distributions with different decay
rates. Although the mathematical models behind them are fundamentally different, they have some common features that
make the movement patterns hard to distinguish [[24][51]–[28][52]]. In broad terms, both models can produce trajectories
that are a combination of short steps (with large turning angles in 2D), which are useful for exploring the patch area,
and long, straight steps, which are efficient to travel the inter-patch distances. Even though both models have
theoretical [[22][53], [23][54]] and experimental (e.g. [[29][55], [30][56]]) support, it is not yet clear if animal
foraging patterns can be described and explained by such models or if they are too complex to admit such
simplifications.

Furthermore, regarding the Lévy walks, there is an ongoing debate on the question whether they emerge under certain
animal foraging strategies. Currently there exist two main hypotheses, referred to as the evolutionary and the
emergentist. The evolutionary hypothesis (also called Lévy flight foraging (LFF) hypothesis) states that certain species
have evolved according to natural selection to develop an optimal foraging strategy consisting of Lévy walk movement
patterns (see e.g. [[31][57]] and references therein). On the other side, the emergentist hypothesis argues that the LFF
hypothesis is not sufficient to account for the complexity of animal behavior since it does not explain certain
anomalies observed experimentally (see [[32][58]] and references therein). It argues that Lévy walks can emerge
spontaneously as a consequence of the features of the environment, which lead to certain responses from the foraging
organism. Thus, these responses are not part of an evolved strategy developed over the course of generations, but can
arise from innate behaviors and lead to Lévy patterns spontaneously when the animal is confronted with certain
environmental conditions.

Due to the fact that our learning task is directly related to foraging strategies, we link the present work to the
aforementioned studies by analyzing the individual trajectories the agents produce as a consequence of the behavior
developed in the different learning contexts.

The paper is organized as follows: an introduction to Projective Simulation and a detailed description of the model and
the learning setup are given in Sec. 2. In Sec. 3, we present different learning tasks and analyze the resulting learned
behaviors. In Sec. 4, we study the emergent group dynamics and individual trajectories within the framework of search
models to determine if they can be described as Lévy walks or composite correlated random walks. Finally, we summarize
the results and conclude in Sec. 5.

## 2 Methods and model

A wide range of models and techniques have been applied to the study of collective behavior. In this work, we apply
Projective Simulation, a model for artificial agency [[9][59], [33][60]–[37][61]]. Each individual is an artificial
agent that can perceive its surroundings, make decisions and perform actions. Within the PS model, the agent’s decision
making is integrated into a framework for reinforcement learning (RL) that allows one to design concrete scenarios and
tasks that the individuals should solve and then study the resulting strategies developed by the agents. We remark that
the notion of *strategy* employed throughout this work does not imply that the agents are able to *plan*. We use the
word “strategy” to refer to the behavior the agents develop given a certain learning task. In addition, each agent’s
motor and sensory abilities can be modeled in a detailed, realistic way.

In our model of collective behavior, the interaction rules with other individuals are not fixed in advance; instead the
agents develop them based on their previous experience and learning. The most natural interpretation of this approach is
that it describes how a group of given individuals change their behavior over the course of their interactions, for
example human children at play. However, our artificial learning agents can also be used to model simpler entities that
do not exhibit learning in the sense of noticeable modifications of their responses over the course of a single
individual’s lifetime, but only change their behavior over the course of several generations. In this case, a single
simulated agent does not correspond to one particular individual, in one particular generation, but rather stands as an
avatar for a generic individual throughout the entire evolution of the species. The environmental pressures driving
behavioural changes over this time-scale can be easily encoded in a RL scenario, since the reward scheme can be designed
in such a way that only the behaviors that happen to be beneficial under these pressures are rewarded. This allows us to
directly test whether the environmental pressures are a possible *causal* explanation for the observed behavior or not.
Our approach interprets the reinforcement of certain responses from an evolutionary perspective. It differs from genetic
algorithms, extended classifier systems [[38][62]], and similar advanced machine learning methodology in that it does
not model evolution in an explicit manner. Such machinery, e.g., the encoding of genes, mutations, and crossover,
usually comes at the cost of a larger model complexity (number of free parameters; see [[33][63]]) and additional
computational overhead. Alternatively, neural network models might be employed, but these are typically difficult to
interpret and thus not useful in our context. Unlike genetic algorithms, Projective Simulation provides a model of
agency that describes a stochastic decision-making process of each individual, which can be used beyond mere
optimization by focusing on the resulting causal explanations.

Although other reinforcement learning algorithms may be used to model a learning agent, Projective Simulation is
particularly suitable for the purpose of modeling collective behavior, since it provides a clear and transparent
structure that gives direct access to the internal state of the agent, so that the deliberation process can be analyzed
in an explicit way and can be related to the agent’s behavior. This analysis can help us gain new insight into how and
why the individual interactions that lead to collective behaviors emerge.

### 2.1 Projective simulation

Projective Simulation (PS) is a model for artificial agency that is based on the notion of episodic memory [[9][64]].
The agent interacts with its surroundings and receives some inputs called percepts, which trigger a deliberation process
that leads to the agent performing an action on the environment.

In the PS model, the agent processes the percepts by means of an internal structure called episodic and compositional
memory (ECM), whose basic units are called clips and represent an episode of the agent’s experience. Mathematically, the
ECM can be represented as a directed, weighted graph, where each node corresponds to a clip and each edge corresponds to
a transition between two clips. All the edge weights are stored in the adjacency matrix of the graph, termed *h* matrix.
For the purpose of this work, the most basic two-layered structure is sufficient to model simple agents. Percept-clips
are situated in the first layer and are connected to the action-clips, which constitute the second layer (see [Fig
1][65]). Let us define these components of the ECM more formally.

#### Fig 1. Structure of the ECM.

[[Fig 1]][66]

[Open in a new tab][67]

The ECM consists of two layers, one for the percepts and one for the actions. Percepts and actions are connected by
edges whose weight *h**ij* determines the transition probability from the given percept to each action (see Sec. 2.2 for
details on the model).
* The *percepts* are mathematically defined as *N*-tuples s=(s1,s2,…,sN)∈S, where S is the Cartesian product
  S≡S1×S2×…×SN. As it can be seen from this mathematical definition, the percept *s* has several categories, represented
  by Si. Each component of the tuple is denoted by si∈{1,…,|Si|}, where |Si| is the number of possible states of Si. The
  total number of percepts is thus given by |S1|···|SN|.
* Analogously, the *actions* are defined as a=(a1,a2,…,aN)∈A, where A≡A1×A2×…×AN and ai∈{1,…,|Ai|}, where |Ai| is the
  number of possible states of Ai. The total number of actions is given by |A1|···|AN|.

As an example, consider an agent that perceives both its internal state, denoted by S1, with two possible percepts
S1={hungry,nothungry}, and some visual input, denoted by S2, with S2={Iseefood,Idonotseefood}. Thus, one out of the four
possible percepts could be *s* = (hungry, I see food). In this case, the possible actions may be
A={goforfood,turnaround}.

[Fig 1][68] represents the structure of the ECM in our model, which consists of a total of 25 percepts and 2 actions
(see Sec. 2.2 for a detailed description).

Let us introduce how the agent interacts with the environment and makes decisions via the ECM. When the agent receives a
percept, the corresponding percept-clip inside the ECM is activated, starting a random walk that only ends when an
action-clip is reached, which triggers a real action on the environment. The transition probability *P*(*j*|*i*) from a
given percept-clip *i* to an action-clip *j* is determined by the corresponding edge weight *h**ij* as,

────────────────┬───
P(j|i)=hij∑khik,│(1)
────────────────┴───

where the normalization is done over all possible edges connected to clip *i*. This process, starting with the
presentation of a perceptual input that activates a percept clip and finishing when the agent performs an action on the
environment, is termed an (individual) *interaction round*.

The structure of the ECM allows one to easily model learning by just updating the *h* matrix at the end of each
interaction round. The *h* matrix is initialized with all its elements being 1, so that the probability distribution of
the actions is uniform for each percept. Reinforcement learning is implemented by the environment giving a reward to the
agent every time that it performs the correct action. The reward increases the *h*-values, and thus the transition
probabilities, of the successful percept-action pair. Hence, whenever the agent perceives again the same percept, it is
more likely to reach the correct action. However, in the context of this work, we are setting a learning task in which
the agent should perform a sequence of several actions to reach the goal and get the reward. If the reward is given only
at the last interaction round, only the last percept-action pair would be rewarded. Thus, some additional mechanism is
necessary in order to store a sequence of several percept-action pairs in the agent’s memory. This mechanism is called
*glow* and the matrix that stores the information about this sequence is denoted by *g*. The components *g**ij*,
corresponding to the percept-action transition *i* → *j*, are initialized to zero and are updated at the end of *every*
interaction round according to:

─────────────────────────────────────────────────────────────────┬───
gij(t+1)=(1-η)gij(t)+{0ifedgewasnottraversed1ifedgewastraversed,}│(2)
─────────────────────────────────────────────────────────────────┴───

where 0 ≤ *η* ≤ 1 is the glow parameter, which damps the intensity of the given percept-action memory. For *η* close to
one, the actions that are taken at interaction rounds in temporal vicinity to the rewarded action are more intensely
remembered that the initial actions. If *η* = 0, all actions the agent performed until the rewarded interaction are
equally remembered. The *g* matrix is updated in such a way that the percept-action pairs that are used more often to
get to the reward are proportionally more rewarded than the pairs that were rarely used. Note that the agent is not able
to distinguish an *ordered* sequence of actions, but this is not necessary for the purpose of this work.

In the context of our learning task, the agent receives a reward from the environment at the end of the interaction
round at which it reaches a goal. Then, the learning is implemented by updating the *h* matrix with the rule,

────────────────┬───
h(t+1)=h(t)+R·g,│(3)
────────────────┴───

where *R* ≥ 0 is the reward (only non-zero if the agent reached the goal at the given interaction round) and *g* is the
updated glow matrix. Technically, the glow matrix is updated first, and then, if the agent is rewarded, the *h* matrix
is updated.

Since we model collective behavior, we consider a group of several agents, each of which has its own and independent ECM
to process the surrounding information. Details on the specific learning task and the features of the agents are given
in the following section.

### 2.2 Details of the model

We consider an ensemble of *N* individuals that we model as PS learning agents, which possess the internal structure
(ECM) and the learning capabilities described in section 2.1. This description of the agents can be seen as a simplified
model for species with low cognitive capacities and simple deliberation mechanisms, or just as a theoretical approach to
study the optimal behavior that emerges under certain conditions.

With respect to the learning, we set up a concrete task and study the strategy agents develop to fulfill it. In
particular, we consider a one-dimensional circular world with sparse resources, which mimics patchy landscapes such as
deserts, where organisms need to travel long distances to find food. Inspired by this type of environments, we model a
task where agents need to reach a remote food source to get rewarded. The strategy the agents learn via the
reinforcement learning mechanism does not necessarily imply that the individual organisms should be able to *learn* to
develop it, but can also be interpreted as the optimal behavior that a species would exhibit under the given
environmental pressures.

Let us proceed to detail the agents’ motor and sensory abilities. The positions that the agents can occupy in the world
are discretized {0, 1, 2…*W*}, where *W* is the world size (total number of positions). Several agents can occupy the
same position. At each interaction round, the agent can decide between two actions: either it continues moving in the
same direction or it turns around and moves in the opposite direction. The agents move at a fixed speed of 1 position
per interaction round. For the remainder of this work, we consider the distance between two consecutive positions of the
world to be our basic unit of length. Therefore, unless stated otherwise, all distances given in the following are
measured in terms of this unit. We remark that, in contrast to other approaches where the actions are defined with
respect to other individuals [[39][69]], the actions our agents can perform are purely motor and only depend on the
previous orientation of the agent.

Perception is structured as follows: a given agent, termed the focal agent, perceives the relative positions and
orientations of other agents inside its visual range (radius with center at the agent’s position) *V**R*, termed its
neighbors. The percept space *S* (see Sec. 2.1) is structured in the Cartesian product form *S* ∈ *S**f* × *S**b*, where
*S**f* is the region in front of the focal agent and *S**b* the region at the back. More precisely, each percept *s* =
(*s**f*, *s**b*) contains the information of the orientation of the neighbors in each region with respect to the focal
agent and if the density of individuals in this region is high or low (see [Fig 2][70]). Each category of percepts can
take the values *s**f*, *s**b* ∈ {0, <3*r*, ≥3*r*, <3*a*, ≥3*a*} (25 percepts in total), which mean:

#### Fig 2. Graphical representation of the percepts’ meaning.

[[Fig 2]][71]

[Open in a new tab][72]

Only the front visual range (colored region) is considered, which corresponds to the values that category *s**f* can
take. The focal agent is represented with a larger arrow than the frontal neighbors. The agent can only see its
neighbors inside the visual range and it can distinguish if the majority are receding (light blue) or approaching (dark
blue) and if they are less or more than three.
* 0. No agents
* <3*r*. There are less than 3 neighbors in this region and the majority of them are receding from the focal agent.
* ≥3*r*. There are 3 or more neighbors in this region and the majority of them are receding from the focal agent.
* <3*a*. There are less than 3 neighbors in this region and the majority of them are approaching the focal agent.
* ≥3*a*. There are 3 or more neighbors in this region and the majority of them are approaching the focal agent.

In the following discussions, we refer to the situation where the focal agent has the same orientation as the neighbors
as a percept of *positive flow* (majority of neighbors are receding at the front and approaching at the back). If the
focal agent is oriented against its neighbors (these are approaching at the front and receding at the back), we denote
it as a percept of *negative flow*. Note that the agents can only perceive information about the neighboring agents
inside their visual range, but they are not able to see any resource or landmark present in the surroundings. This
situation can be found in realistic, natural environments where the distance between resources is large and the searcher
has no additional input while moving from one patch to another. Furthermore, the important issue of body orientation is
thereby taken into account in our model [[32][73]].

The interactions between agents are assumed to be sequential, in the sense that one agent at a time receives a percept,
deliberates and then takes its action before another agent is given its percept. Technically, agents are given a label
at the beginning of the simulation to keep track of the interaction sequence but we remark that they are placed at
random positions in the world. There are two reasons for assuming a sequential interaction. For one, in a group of real
animals (or other entities), different individuals typically take action at slightly different times, with perfect
synchronization being a remarkable and costly exception. The second argument in favor of sequential updating is that it
ensures that a given agent’s circumstances do not change from the time it receives its percept until the time when its
acts. If the actions of all agents were applied simultaneously, a given focal agent would not be able to react to the
actions of the other agents in the same round. Such a simplification would not allow us to take into account any
sequential, time-resolved interactions between different agents of a group. In the real situation, while one focal agent
is deliberating, other agents’ actions may change its perceptual input. Therefore, an action that may have been
appropriate at the beginning of the round, would no longer be appropriate at this agent’s turn.

The complete simulation has the structure displayed in [Fig 3][74], where:

#### Fig 3. Structure of the simulation.

[[Fig 3]][75]

[Open in a new tab][76]

Each ensemble of agents is trained for 10⁴ trials, where each trial consists of 50 global interaction rounds (g.i.r.).
At each g.i.r., the agents interact sequentially (see text for details).
* With each ensemble of *N* = 60 agents, we perform a simulation of 10⁴ trials during which the agents develop new
  behaviors to get the reward (RL mechanism). This process is denoted as *learning process* or *training* from this
  point on.
* Each trial consists of *n* = 50 global interaction rounds. At the beginning of each trial, all agents of the ensemble
  are placed in random positions within the initial region (see [Fig 4][77]).
* We define a global interaction round to be the sequential interaction of the ensemble, where agents take turns to
  perform their individual interaction round (perception-deliberation-action). Note that each agent perceives, decides
  and moves only once per global interaction round.

#### Fig 4. 1D environment (world).

[[Fig 4]][78]

[Open in a new tab][79]

Agents are initialized randomly within the first 2*V**R* positions. Food is located at positions *F* and *F*′. *d**F* is
the distance from the center of the initial region *C* to the food positions.

The learning task is defined as follows: at the beginning of each trial, all the agents are placed at random positions
within the first 2*V**R* positions of the world, with orientations also randomized. Each agent has a fixed number *n* of
interaction rounds over the course of a trial to get to a food source, located at positions *F* and *F*′ ([Fig 4][80]).
At each interaction round, the agent first evaluates its surroundings and gets the corresponding percept. Given the
percept, it decides to perform one out of the two actions (“go” or “turn”). After a decision is made, it moves one
position. If the final position of the agent at the end of an interaction round is a food point, the agent is rewarded
(*R* = 1) and its ECM is updated according to the rules specified in Sec. 2.1. Each agent can only be rewarded once per
trial. Note that the *h* matrices of the agents are only updated following [Eq (3)][81], and we do not consider any
other transformation to explicitly model evolution as it could be done, in principle, using genetic algorithms to
explicitly represent evolutionary mechanisms of mutation, crossover etc. (see also the discussion at the beginning of
Sec. 2).

We consider different learning scenarios by changing the distances *d**F* at which food is positioned. However, note
that a circular one-dimensional world admits a trivial strategy for reaching the food without any interactions, namely
going straight in one direction until food is reached. Thus, in order to emulate the complexity that a more realistic
two-dimensional scenario has in terms of degrees of freedom of the movement, we introduce a noise element that
randomizes the orientation of each agent every *s**r* steps (it changes orientation with probability 1/2). Not all
agents randomize the orientations at the same interaction round, which would lead to random global behavior. This
randomization can be also interpreted biologically as a fidgeting behavior or even as a built-in behavior to escape
predators [[40][82]]. Protean movement has been observed in several species [[41][83]–[44][84]] and there exist
empirical studies that show that unpredictable turns [[45][85]] and complex movement patterns [[46][86]] decrease the
risk of predation. In addition, if the memory of the organism is not very powerful, we can also consider that, at these
randomization points, it forgets its previous trajectory and needs to rely on the neighbors’ orientations in order to
stabilize its trajectory. The agent can do so, since the randomization takes place right before the agent starts the
interaction round.

Under these conditions, we study how the agents get to the food when the only input information available to them is the
orientation of the agents around them.

## 3 Results I: Learned behavior in different scenarios

We consider different learning scenarios characterized by the distance *d**F* (see [Fig 4][87]). We study how the
dynamics that the agents develop in order to reach the food source change as the distance *d**F* increases. In
particular, we focus on two extreme scenarios: one where the resource is within the initial region (*d**F* < *V**R*)
—agents are initialized within the first 2*V**R* positions of the world—, and the other one where the resource is at a
much larger distance. As a scale for this distance, we consider how far an agent can travel on average with a random
walk, which is dRW=n providing that it moves one position per interaction round. Hence, the other extreme scenario is
such that *d**F* ≫ *d**RW*. Note that the scale of *d**F* for this regime depends on the total number *n* of interaction
rounds that the agents perform in one trial. The maximum value of *d**F* that we can choose thus depends on the maximum
distance the agents can travel within *n* rounds following an unbiased random walk (for *n* = 50, this threshold is
approximately at *d**F* = 21).

The situation where *d**F* < *V**R* mimics an environment with densely distributed resources, whereas the regime with
*d**F* ≫ *d**RW* resembles a resource-scarce environment where a random walk is no longer a valid strategy for reaching
food sources.

The parameters of the model that are used in all the learning processes are given in [Table 1][88]. Providing that
dRW=50≃7, we consider values of *d**F* ranging from 2 to 21 and focus on the cases with *d**F* = 4, 21 as the
representative examples of resource-dense and resource-scarce environments, respectively. All agents start the learning
process with a newly initialized *h* matrix, so they perform each action (“go” or “turn”) with equal probability. [Fig
5][89] shows the learning curves for three different scenarios, where the food is placed at *d**F* = 4, 10, 21. The
learning processes are independent from each other, that is, the distance *d**F* does not change within one complete
simulation of 10⁴ trials. In this way, we can analyze the learned behaviors separately for each *d**F*. The learning
curve displays the percentage of agents that reach the food source and obtain a reward at each trial. As a baseline for
comparison, we also set the same learning task with *d**F* = 21 for non-interacting (n.i.) agents (we set *V**R* = 0, so
they cannot see the neighbors). The n.i. agents learn to go straight almost deterministically —the probability for the
action “go” at the end of the learning process is almost 1 for percept (0, 0)—. Therefore, these agents perform a random
walk with *n*/*s**r* = 50/5 = 10 steps of length *s**r* = 5, which allows it to cover a distance of 510≃16 positions.
The rest of percepts are never encountered, so the initial *h* values remain the same. Due to the periodic randomization
of the agents’ orientation, it can be seen that they do not reach the efficiency rate of the interacting agents (see
[Fig 5][90]) and only one out of three agents reaches the reward at each trial. [Fig 5][91] shows that, for *d**F* = 4,
the food source is so close (inside the initial region) that the agents get the reward in all the trials from the
beginning. On the other hand, the tasks with *d**F* = 10, 21 show a learning process that takes more trials for the
agents to come up with a behavior that allows them to get to the reward. In particular, only 40% of the agents are able
to reach the goal with the initial behavior (Brownian motion) in the scenario with *d**F* = 10 and this percentage drops
to almost 0% in the case with *d**F* = 21. Note that it takes more trials for the agents to learn how to get to the
furthest point (*d**F* = 21) than it takes for *d**F* = 10 (see inset in [Fig 5][92]). The interacting agents start
outperforming the n.i. agents in the task with *d**F* = 21 at trial 200, where they start to form aligned swarms, as one
can also see from the increase in the alignment parameter at the same trial (see Sec. 3.2.1 for details).

### Table 1. Description of the parameters used in the learning simulations with PS.

──────────────────────────────┬─────────────────────────────────
Agent                         │Environment                      
────────────────────────┬─────┼───────────────────────────┬─────
Description             │Value│Description                │Value
────────────────────────┼─────┼───────────────────────────┼─────
Visual range (*V**R*)   │6    │Number of agents (*N*)     │60   
────────────────────────┼─────┼───────────────────────────┼─────
Reorient. freq. (*s**r*)│5    │World size (*W*)           │500  
────────────────────────┼─────┼───────────────────────────┼─────
Glow (*η*)              │0.2  │Int. rounds per trial (*n*)│50   
────────────────────────┼─────┼───────────────────────────┼─────
Reward (*R*)            │1    │Number of trials           │10⁴  
────────────────────────┴─────┴───────────────────────────┴─────
[Open in a new tab][93]

### Fig 5. Learning curves for *d**F* = 4, 10, 21 and *d**F* = 21 for non-interacting (n.i.) agents.

[[Fig 5]][94]

[Open in a new tab][95]

The curve shows the percentage of agents that reach the food source and obtain a reward of *R* = 1 at each trial. For
each task, the average is taken over 20 (independent) ensembles of 60 agents each and the shaded area indicates the
standard deviation. Zooming into the initial phase of the learning process, the inset figure shows a faster learning in
the task with *d**F* = 10 than in the task with *d**F* = 21. In the case of *d**F* = 21, no agent is able to reach the
food source in the first trial, and it takes the interacting agents approx. 200 trials to outperform the n.i. agents.

### 3.1 Individual responses

The behavior the agents have learned at the *end* of the training can be studied by analyzing the final state of the
agents’ ECM, from where one obtains the final probabilities for each action depending on the percept the agents get from
the environment (see [Eq (1)][96]). These final probabilities are given in [Fig 6][97] for the learning tasks with
*d**F* = 4, 21.

#### Fig 6. Learned behavior at the end of the training process.

[[Fig 6]][98]

[Open in a new tab][99]

The final probabilities in the agents’ ECM for the action “go” are shown for each of the 25 percepts (5*x*5 table). (a)
and (b) Final probabilities learned in the scenarios with *d**F* = 21 and *d**F* = 4 respectively. The average is taken
over 20 ensembles (each learning task) of 60 agents each. Background colors are given to easily identify the learned
behavior, where blue denotes that the preferred action for that percept is “go” and orange denotes that it is “turn”.
More specifically, the darker the color is, the higher the probability for that action, ranging from grey (*p* ≃ 0.5),
light (0.5 < *p* < 0.7) and normal (0.7 ≤ *p* < 0.9) to dark (*p* ≥ 0.9). Figures (c) and (d) show what the tables would
look like if the behavior is purely based on alignment (agent aligns to its neighbors with probability 1) or cohesion
(agent goes towards the region with higher density of neighbors with probability 1), respectively. See text for details.

Tables of [Fig 6][100] show the probability of taking the action “go” for each of the 25 percepts. We focus on the
learning tasks with *d**F* = 4, 21, which represent the two most distinctive behaviors that we observe.

Let us start with the case of *d**F* = 21 ([Fig 6(a)][101]), which corresponds to a task where the food is located much
further away than the distance reachable with a random walk. In this case, highly aligned swarms emerge as the optimal
collective strategy for reaching the food (see also Sec. 3.2 and figures therein), since the orientations of the
surrounding neighbors allow the focal agent to stabilize its orientation against the periodic randomization. The
individual responses that lead to such collective behavior can be studied by looking at table (a): the diagonal
corresponds to percepts with a clear reaction leading to alignment, i.e. to keep going when there is a positive flow of
neighbors and to turn if there is a negative flow. More specifically, one can see that when the agent is in the middle
of a swarm and aligned with it, the probability that it keeps going is 0.99 for dense swarms [percept (≥3*r*, ≥3*a*)]
and 0.90 for sparse swarms [percept (<3*r*, <3*a*)]. In the same situations, the agent that is not aligned turns around
with probability 0.97 for dense swarms [percept (≥3*a*, ≥3*r*)] and 0.57 for sparse swarms [percept (<3*a*, <3*r*)].
Outside the diagonal, one observes that the probability of turning is high when a high density of agents are approaching
the focal individual from the front (last row) and the agents in the back are not approaching. We can also analyze the
learned behavior at the back edge of the swarm, which is important to keep the cohesion of the swarm. When an agent is
at the back of a dense swarm and aligned with it [percept (≥3*r*, 0)], the probability of keeping the orientation is
0.81. If instead, the agent is oriented against the swarm [percept (0, ≥3*r*)] the probability of turning around to
follow the swarm is 0.65. This behavior is less pronounced when the swarm is not so dense [percepts (<3*r*, 0), (0,
<3*r*)], in fact, when a low density of neighbors at the back are receding from the focal agent [percept (0, <3*r*)],
the focal agent turns around to rejoin the swarm with probability 0.4, which results in this agent leaving the swarm
with higher probability. If the agent is alone [percept (0, 0)], it keeps going with probability 0.77.

A very different table is observed for *d**F* = 4 ([Fig 6(b)][102]). In this task, the food source is located inside the
initial region where the agents are placed at the beginning of the trials, so the agents perceive, in general, high
density of neighbors around them. For this reason, they rarely encounter the nine percepts encoding low density —that
correspond to the ones at the center of the table, with grey background (Table (b) in [Fig 6][103])— throughout the
interaction rounds they perform until they get the reward. The corresponding probabilities are the initialized ones,
i.e. 1/2 for each action. For the remaining percepts, we observe that the agents have learned to go to the region with
higher density of neighbors, which leads to very cohesive swarms (see also Sec. 3.2.2). Since the food source is placed
inside the initialization region in this case —which is also within the region agents can cover with a random walk—,
there is a high probability that there are several agents already at the food source when an agent arrives there, so
they learn to go to the regions with higher density of agents. This behavior can be observed, for instance, for percepts
in the first column (high density at the back) and second, third and fourth row (low/no density at the front), where the
agents turn around with high probability. In addition, we observe that there is a general bias towards continuing in the
same direction, which can be seen for example in percepts with the same density in both regions (e.g. percepts at the
corners of the table). The tendency to keep walking is always beneficial in one-dimensional environments to get to the
food source (non-interacting agents learn to do so deterministically, as argued for [Fig 5][104]). In general, we
observe that, in order to find the resource point at *d**F* = 4, agents do not need to align with their neighbors
because the food is close enough that they can reach it by performing a Brownian walk.

[Fig 6(c) and 6(d)][105] show what the tables would look like if the agents had deterministically (with probability 1)
learned just to align with the neighbors (c) or just to go to the region inside the visual range with higher density of
neighbors (d). In these figures, percepts for which there is no pronounced optimal behavior have grey background.

In [Fig 7][106], we select four representative percepts that show the main differences in the individual behaviors and
plot the average probability of taking the action “go” at the end of a wide range of different learning scenarios where
the distance to the food source is increasingly large. We observe that there are two clear regimes with a transition
that starts at *d**F* = 6. This is the end of the initial region (see [Fig 4][107], with *V**R* = 6 in our simulations)
where the agents are positioned at the beginning of each trial (see [S1 Appendix][108] for details on why this
transition occurs at *d**F* = 6). The main difference between regimes is that, when the food is placed near the initial
positions of the agents, they learn to “join the crowd”, whereas, if the food is placed farther away, they learn to
align themselves and “go with the flow”. More specifically, for *d**F* < 6, the orientations of the surrounding
neighbors do not play a role, but the agents learn to go to the region (front/back) with higher number of neighbors,
which leads to unaligned swarms with high cohesion. On the contrary, for the tasks with *d**F* > 6, the agents tend to
align with their neighbors. This difference in behavior can be observed, for instance, in the dark blue (squares) curve
of [Fig 7][109], which corresponds to the percept “positive flow and higher density at the back”. We observe that for
*d**F* = 2, 4, the preferred action is “turn” (the probability of taking action “go” is low), since there are more
neighbors at the back. However, for *d**F* = 10, 14, 21, the agents tend to continue in the same direction, since there
is a positive flow (neighbors have the same orientation as the focal agent). Analogously, the brown curve (triangles)
shows the case where there is a negative flow and higher density at the front, so agents trained to find nearby food
(*d**F* = 2, 4) have high probability of going, whereas agents trained to find distant food (*d**F* = 10, 14, 21) have
high probability of turning.

#### Fig 7. Final probability of taking the action “go” depending on the learning task (increasing distance to food
#### source *d**F*) for four significant percepts.

[[Fig 7]][110]

[Open in a new tab][111]

The percepts are (< 3*r*, < 3*a*), (< 3*r*, ≥ 3*a*), (< 3*a*, < 3*r*), (≥ 3*a*, < 3*r*), respectively (see legend). The
average is taken over the agents’ ECM of 20 independently trained ensembles (1200 agents) at the end of the learning
process. Each ensemble performs one task per simulation (*d**F* does not change during the learning process).

We remark that, even though the learning task is defined in terms of the distances *d**F*, the results from this section
and [S1 Appendix][112] show that the main features of these two types of dynamics do not only depend o

[Content truncated]
```
