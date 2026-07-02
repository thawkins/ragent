# Web source

- URL: https://www.nature.com/articles/s41598-025-20247-8
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-30T09:41:15.999227968+00:00

```text
[Skip to main content][1]

Thank you for visiting nature.com. You are using a browser version with limited support for CSS. To obtain the best
experience, we recommend you use a more up to date browser (or turn off compatibility mode in Internet Explorer). In the
meantime, to ensure continued support, we are displaying the site without styles and JavaScript.

Advertisement

[ [Advertisement]][2]
[ [Scientific Reports] ][3]
* [ View all journals ][4]
* [ Saved research ][5]
* [ Search ][6]
* [Log in][7]
* [ Content Explore content ][8]
* [ About the journal ][9]
* [ Publish with us ][10]
* [ Sign up for alerts ][11]
* [ RSS feed ][12]
1. [nature][13]
2. [scientific reports][14]
3. [articles][15]
4. article
Blockchain-enhanced incentive-compatible mechanisms for multi-agent reinforcement learning systems
[ Download PDF ][16]
[ Download PDF ][17]
* Article
* [Open access][18]
* Published: 28 November 2025

# Blockchain-enhanced incentive-compatible mechanisms for multi-agent reinforcement learning systems
* [Ke Tian][19]^{[1][20]} 

[Scientific Reports][21] **volume 15**, Article number: 42841 (2025) [Cite this article][22]

[ Save article ][23]
[ View saved research ][24]
* 4756 Accesses
* 3 Citations
* [Metrics details][25]

### Subjects
* [Engineering][26]
* [Mathematics and computing][27]

## Abstract

Ensuring trust, fairness, and long-term efficiency in multi-agent systems poses significant challenges, particularly
under partially competitive and decentralized settings where strategic manipulation and collusion can arise. This paper
proposes a blockchain-enhanced framework that integrates smart contracts with multi-agent reinforcement learning (MARL)
to design incentive-compatible mechanisms for strategic agent coordination. The framework utilizes the decentralized and
tamper-resistant nature of blockchain to record agent behaviors on-chain, enforce transparency, and implement automated
penalty and reward mechanisms through smart contracts. We embed these mechanisms into a Multi-Agent Soft Actor-Critic
(MASAC) algorithm, aligning local decision-making with global system objectives. Experimental validation in two
representative domains—automated market bidding and intelligent traffic control—demonstrates that the proposed approach
significantly improves social welfare, reduces collusion success rates, enhances fairness, and increases behavioral
robustness under noise. Ablation studies further reveal the complementary contributions of each system component. This
work lays the foundation for scalable, transparent, and incentive-aligned coordination in decentralized intelligent
agent systems.

### Similar content being viewed by others

### [A graph attention network-based multi-agent reinforcement learning framework for robust detection of smart contract
### vulnerabilities ][28]

Article Open access 14 August 2025

### [Design of an improved model using federated learning and LSTM autoencoders for secure and transparent blockchain
### network transactions ][29]

Article Open access 10 January 2025

### [Blockchain-enabled supply chain finance risk intelligent assessment and trust mechanism construction ][30]

Article Open access 27 May 2026

## Introduction

In recent years, Distributed Artificial Intelligence (Distributed AI) and Multi-Agent Reinforcement Learning (MARL) have
gained significant traction in a wide variety of applications, often involving complex environments where multiple
intelligent agents must make decisions concurrently, either in competition or cooperation^{[1][31],[2][32]}. Typical
examples include automated market bidding, intelligent traffic signal control, and drone formation tasks; however, as
the number of agents and the complexity of interactions increase, problems such as “cheating” or “collusion” become more
prevalent, degrading efficiency, triggering trust issues, and even destabilizing the entire
system^{[3][33],[4][34],[5][35]}. An emerging technology that holds promise for mitigating these concerns is the
blockchain, whose decentralized, tamper-resistant, and auditable features present a new avenue for establishing trust
among agents without relying on a central authority^{[6][36],[7][37]}. By recording transactions (or agent actions) in
an immutable ledger and automatically enforcing rules through smart contracts, blockchain-based systems provide
transparent and verifiable mechanisms for interactions among agents, allowing on-chain implementation of incentive
structures and mechanism design to ensure that malicious strategies yield lower long-term utility compared to truthful
participation^{[8][38],[9][39]}.

Motivated by these advantages, this paper proposes a blockchain-supported incentive-compatible mechanism for multi-agent
game environments, integrating mechanism design principles with MARL and leveraging blockchain’s inherent properties to
create an environment in which each self-interested agent, while maximizing its own rewards, is disincentivized from
dishonest or collusive strategies^{[10][40],[11][41]}. We present a novel blockchain-based architecture that records
agents’ actions in a decentralized ledger, employs smart contracts to ensure transparent decision-making and reward
allocation, and formalizes an incentive mechanism wherein immutable records and consensus protocols eliminate tampering
or deception, thereby consistently favoring honest behavior over manipulative alternatives. Furthermore, we embed
mechanism design concepts into multi-agent reinforcement learning algorithms, enabling agents to learn optimal policies
under both competitive and cooperative conditions while harnessing the blockchain’s transparent reward system to promote
compliance and enhance overall social welfare. To validate the proposed approach, we conduct extensive simulations in
two representative scenarios—automated market bidding and intelligent traffic signal control—and demonstrate that our
method improves fairness, boosts system-wide efficiency, and maintains robustness against malicious or collusive
attempts, ultimately illustrating how rational agents can be guided to act in ways that benefit both themselves and the
entire system.

## Related work

### Multi-agent reinforcement learning

Multi-Agent Reinforcement Learning (MARL) has shown significant promise in addressing complex tasks that involve
multiple interacting agents, each with individual objectives and varying degrees of cooperation or competition. However,
applying traditional single-agent reinforcement learning methods directly to multi-agent settings often proves
inadequate due to a range of distinctive challenges, including non-stationary environments, intertwined policy updates,
and limited observability of other agents’ states and actions^{[12][42],[13][43]}. As a result, researchers have
proposed specialized MARL frameworks and algorithms to ensure more stable learning, efficient coordination, and robust
performance against adversarial strategies. One of the most straightforward yet foundational approaches involves
treating each agent as a single-agent learner, independently applying algorithms such as Q-Learning or Policy Gradient
without explicit consideration of other agents’ policies^{[14][44]}. Although conceptually simple, this “Independent
Learning” strategy suffers when the environment becomes highly non-stationary, since each agent’s behavior changes over
time, thus leading to convergence instability and limiting overall performance in domains that require tight
coordination or adaptive responses to opponents. To address these limitations, a common strategy is to adopt a
“Centralized Training and Decentralized Execution (CTDE)” paradigm^{[15][45]}, in which algorithms have access to global
information during training (e.g., all agents’ observations, actions, and rewards) but only employ localized
observations at execution time. Classic examples include Multi-Agent Deep Deterministic Policy Gradient (MADDPG) and
Counterfactual Multi-Agent Policy Gradients (COMA), wherein a centralized critic evaluates joint actions while each
agent’s decentralized actor uses local information in real-time decisions^{[16][46]}. By separating training from
execution, CTDE methods effectively balance coordination efficiency and scalability in mixed cooperative-competitive
settings. When agents engage in adversarial or partially cooperative scenarios, researchers further integrate
game-theoretic constructs—such as Nash Equilibria, correlated equilibria, or cooperative game formulations—into MARL,
thus enhancing learning outcomes by addressing strategic uncertainties^{[17][47]}. In zero-sum games, for instance, MARL
methods leverage iterative techniques to approximate Nash policies, enabling agents to adapt optimally to competitive
strategies; in cooperative or general-sum settings, additional concepts like Shapley values and reward shaping encourage
stable and fair joint solutions. Overall, modern MARL research highlights coordination, stability, and scalability in
non-stationary multi-agent environments, with each class of methods—ranging from Independent Learning to CTDE and
game-theoretic approaches—offering a different balance of simplicity, performance, and theoretical guarantees,
ultimately forming the basis for applications such as robotics swarms, autonomous vehicles, and other domains where
strategic interactions strongly influence system-level outcomes.

### Blockchain and smart contracts

Blockchain technology, first popularized by Nakamoto (2008), offers decentralized governance, tamper-resistance, and
traceability, providing a novel foundation of trust for multiple parties without the need for a central authority. By
storing data in an immutable ledger and facilitating programmatic control via smart contracts^{[18][48]}, it enables
autonomous enforcement of predefined rules while minimizing human intervention. These features naturally complement
multi-agent systems, where distributed entities must coordinate, exchange information, and reach consensus under
potential adversarial conditions. Indeed, initial explorations in areas such as supply chain finance and distributed
energy trading highlight the capacity of blockchain to enhance transparency and efficiency in multi-agent
collaborations^{[19][49],[20][50]}. Nevertheless, most existing efforts focus primarily on data storage, simplistic
token-based incentives, or basic consensus mechanisms, offering limited insight into the complexities of multi-agent
strategic interactions and long-term cooperative or competitive behaviors^{[21][51]}. Consequently, there remains a
pressing need to investigate advanced mechanism design that exploits blockchain’s immutable records and executable smart
contracts for incentive alignment and conflict resolution in intricate multi-agent settings. Addressing this gap
requires more sophisticated modeling at the intersection of decentralized ledger technology, game theory, and
reinforcement learning, thereby ensuring that blockchain-based multi-agent systems can transition beyond rudimentary
data-sharing applications and tackle complex, dynamic, and potentially large-scale environments in a secure and
trustless manner.

### Mechanism design and incentive compatibility

Mechanism design is a branch of game theory that focuses on constructing rules and incentive structures to align
individual agents’ self-interested behaviors with system-wide objectives^{[22][52]}. By specifying how actions map to
outcomes and how rewards (or costs) are distributed, mechanism design aims to achieve socially desirable equilibria,
typically maximizing collective welfare or enforcing fairness while still allowing each participant to pursue its own
utility. Central to this paradigm is incentive compatibility, the requirement that rational agents have no motivation to
misreport private information or collude in ways that could distort the mechanism’s intended outcome^{[23][53]}. If a
mechanism is incentive-compatible, then truth-telling or honest participation emerges as the optimal strategy for each
agent, minimizing manipulative practices that might degrade system performance. The integration of mechanism design
principles with blockchain smart contracts significantly augments the reliability of such frameworks, as the immutable
and decentralized nature of the ledger ensures transparent enforcement of rules while reducing the risk of tampering or
collusion^{[24][54]}. In essence, smart contracts can automatically execute the prescribed allocation and payment
schemes, thereby removing trusted third-party dependencies and diminishing operational
overhead^{[25][55],[26][56],[27][57]}. Nevertheless, implementing advanced mechanism design in a blockchain setting
demands careful attention to scalability, consensus latency, and potential strategic attacks on the network layer. These
issues underscore the need for robust protocols and incentive-compatible frameworks that can thrive in distributed,
adversarial, and large-scale environments—areas where multi-agent learning and mechanism design intersect to create
opportunities for novel, automated solutions.

## Problem definition and system architecture

### Multi-agent game environment definition

In this work, we consider a multi-agent game environment with N autonomous agents, each denoted by \(i \in I = \left\{
{1,2, \ldots ,N} \right\}\). The environment’s global state space is represented by S, and each state \(s \in S\)
captures the system’s configuration at a given time. Every agent \(i \in I\) has an associated action set \(A_{i}\), and
a joint action is defined as \(a = (a_{1} ,a_{2} , \ldots ,a_{N} ) \in A = A_{1} \times A_{2} \times \cdots \times
A_{N}\), where \(a_{i} \in A_{i}\) for \(i \in I\). The combined action space is thus \(A = A_{1} \times A_{2} \times
\cdots \times A_{N}\). Upon execution of a joint action \(a \in A\), the environment transitions to a new state
\(s^{\prime} \in S\) and provides each agent \(i \in I\) with an immediate reward \(r_{i}\), which typically encompasses
both individual benefits and collective gains. Due to the mixed cooperative-competitive nature of many real-world
applications, we assume the possibility of “cheating” behaviors, such as falsifying reported actions, concealing
relevant information, or colluding against specific agents. These actions can undermine fairness and overall efficiency,
especially when the agents are driven by self-interest. Consequently, our objective is to devise an incentive-compatible
mechanism that guarantees rational agents will, under the assumption of self-interested maximization, adopt rule-abiding
or cooperative strategies rather than exploit the system for short-term advantage.

In this work the joint action is generated by having each agent independently sample an action from its local policy
based on its own observation, after which the individual actions are combined into the joint action vector a that
represents the system-wide decision at that time step. The environment transition mechanism P then maps the current
global state together with the joint action to a distribution over the next global state, thereby capturing the
stochastic dynamics induced by decentralized decision-making. The long-term return for each agent is defined as the
discounted cumulative reward, where future rewards are weighted by a discount factor γ to reflect the balance between
immediate and delayed outcomes. Throughout training we adopt the centralized training and decentralized execution
paradigm, under which the centralized critic or learner has access to global state and action information during
training to improve stability and coordination, while during execution each agent relies only on its local observations
to select actions in a scalable and decentralized manner.

### Blockchain and smart contract layer

To establish a secure and transparent decision-making process in the multi-agent setting, we integrate a blockchain
network that operates as follows. First, each agent or its designated proxy can function as a node participating in the
consensus procedure, thereby ensuring a decentralized ledger that records every critical interaction. In private or
consortium blockchain scenarios, a subset of “witness nodes” may be authorized to uphold network security and
consistency. Second, smart contracts formally encode the rules and incentives of the game, enabling automated
verification of agent actions, execution of reward-and-penalty computations, and publication of both current system
states and historical activity logs on-chain. By codifying these elements into smart contracts, the framework can
systematically check for irregularities—such as deceptive bidding, unauthorized strategy changes, or unexpected
coordination failures—and respond through preset sanctions or token redistributions. This eliminates the need for a
third-party arbitrator and considerably reduces administrative overhead, as all agreements and transactions are
self-enforced once written into the contract’s code. Finally, all key information, including bidding prices or traffic
control policies, is maintained in the blockchain ledger to facilitate future auditing or incentive distribution.
Because the ledger is tamper-resistant and visible to all authorized stakeholders, it supports an incontrovertible
source of truth that fosters accountability and reduces disputes. Consequently, agents face increased pressure to comply
with established rules and to adopt “honest” behavior over exploitative tactics, while simultaneously retaining the
system’s decentralized advantages.

### Incentive compatibility mechanism principle

In designing the incentive mechanism, we assume that each agent aims to maximize its own expected return (i.e.,
cumulative reward), denoted by \(U_{i}\). Formally, the mechanism must satisfy: \(U_{i}
({\text{honest}}\,{\text{strategy}}) \ge U_{i} ({\text{dishonest}}\,{\text{strategy}})\), indicating that no agent
should find it profitable to deviate from truthful or cooperative participation. Achieving this condition relies on
three key elements: immutable data recording, transparent incentive distribution, and long-term reward design. First,
the immutable data recording capability provided by the blockchain infrastructure ensures that no single agent can
retroactively alter past actions or transaction details for personal gain. Since all critical interactions, from bidding
information to resource allocations, are logged in a shared ledger that is collectively maintained by the network’s
participants, any attempt at retroactive tampering would be immediately detected or rendered computationally infeasible.
As a result, dishonest agents face a diminished probability of extracting value from hidden manipulations, and the
deterrent effect increases with the cost of mounting a successful attack on the blockchain. Second, transparent
incentive distribution guarantees that each agent’s reward is determined solely by verifiable information stored
on-chain, insulating the payout mechanism from unilateral tampering. By codifying the reward function and penalty rules
within smart contracts, the system automatically calculates and distributes payoffs based on actions and outcomes
visible to all authorized parties. This eliminates clandestine deals or back-channel negotiations, while also precluding
any single point of failure or collusion arising from centralized bookkeeping.

In competitive-cooperative scenarios where agents might otherwise conspire to inflate joint returns or ostracize a
subset of participants, the blockchain records serve as a public adjudicator, significantly reducing the incentive to
commit fraud. Finally, long-term reward design plays a pivotal role by explicitly favoring extended compliance and
discouraging short-term exploitation. Through either reinforcement learning or game-theoretic equilibrium analysis, the
mechanism can weight future rewards heavily enough that agents perceive greater gains from sustained honest engagement
than from a momentary breach of protocol. Over repeated rounds of interaction, agents learn that abiding by the
mechanism’s rules yields stable and high expected returns, while cheating or colluding generates only transient
advantages—if any—followed by sanctions or lost trust. By integrating these three pillars into a coherent
incentive-compatibility framework, the system fosters rational cooperation and reduces the strategic payoff of deceptive
practices. This approach is particularly well-suited for environments where agents dynamically adapt to each other’s
strategies and where the cost of information asymmetry can be high, as it realigns short-term interests with collective
long-term efficiency.

In this work incentive compatibility is defined as the condition that under the specified smart contract parameters and
detection rules, neither a unilateral deviation by a single agent nor a coordinated deviation by a coalition yields a
higher expected long-term return than following the prescribed compliant strategy. This definition rests on three
essential elements: the immutable nature of blockchain records that prevents retroactive manipulation, the transparent
settlement and redistribution rules codified in the smart contract that guarantee verifiable payoffs, and the explicit
weighting of future rewards that ensures sustained compliance is more profitable than short-term exploitation.
Concretely, when a deviation is detected by the contract’s monitoring logic, the redistribution coefficient applied to
the deviating agent’s future rewards is reduced, the penalty is permanently logged on-chain, and this adjustment
directly feeds back into the agent’s subsequent learning signals.

## Technical details

### Reinforcement learning methodology

To facilitate efficient multi-agent collaboration and competition, we employ a Centralized Training and Decentralized
Execution (CTDE) paradigm. During the training phase, a centralized component has access to global state information and
can gather each agent’s actions and rewards, thus enabling more stable and informed gradient updates. Conversely, in the
execution phase, each agent relies solely on locally available observations, preserving scalability and reducing
communication overhead. Under this framework, several well-established MARL algorithms may be applied:
* MADDPG (Multi-Agent Deep Deterministic Policy Gradient). This method extends the deterministic policy gradient
  approach to multi-agent settings by maintaining a centralized critic for each agent, which conditions on the global
  state and the actions of all agents.
* MASAC (Multi-Agent Soft Actor-Critic). Particularly well-suited for mixed cooperative-competitive tasks, MASAC
  inherits the benefits of Soft Actor-Critic’s entropy augmentation, rendering it more robust to strategy uncertainties
  and better able to handle continuous action spaces.
* QMIX, QTRAN, and Value Decomposition Methods. When cooperative objectives predominate, value decomposition can
  explicitly factor the joint action-value function into per-agent components, simplifying credit assignment and
  convergence.

In practice, our system leverages MASAC in scenarios that require both cooperative coordination (e.g., managing shared
resources) and competitive decision-making (e.g., strategic bidding), balancing adaptability with robust performance.
Nonetheless, alternative algorithms may be adopted where discrete action spaces dominate or purely cooperative behavior
is prioritized.

### Blockchain network and smart contract design

The blockchain layer underpins a secure, immutable record of agent interactions, anchoring our incentive mechanisms:
* Network layer. We implement a consortium (permissioned) blockchain configuration to balance decentralization and
  performance. Frameworks like Hyperledger Fabric or specialized sidechains can be employed to achieve efficient
  consensus without sacrificing data integrity. This design choice ensures a controlled participant set (e.g.,
  recognized stakeholders or agent proxies) while maintaining cryptographic security against external tampering.
* Smart contract logic. At the heart of our system lies a set of smart contracts that codify the rules of the
  multi-agent game and govern reward distribution. Key operations include:
  1. 1.
     
     Data on-chain. Periodically or event-triggered, each agent’s observed local state, performed action, and immediate
     reward are submitted to the blockchain. Hashes of critical parameters or policy updates may also be stored for
     verifiability.
  2. 2.
     
     Reward/penalty computation. A dedicated function, rewardDistribution(), aggregates relevant data at the end of each
     round or phase, calculating each agent’s total payoff or penalty. The contract subsequently adjusts on-chain
     account balances accordingly.
  3. 3.
     
     Incentive compatibility safeguards. Embedded within the contract are rules for detecting anomalous behavior and
     penalizing proven violations. If consensus nodes or a predefined detection mechanism identify misconduct, the
     offending agent(s) receive reduced future rewards or face confiscation of on-chain tokens.

By automating these processes through smart contracts, the system removes the need for manual oversight, strengthens
trust via transparent execution, and ensures that any violations or questionable transactions are promptly surfaced and
penalized.

### Mechanism design implementation

To fully operationalize our incentive-compatible framework within the blockchain-based multi-agent setting, we
incorporate the following critical steps:
1. 1.
   
   Initial setup. Each agent \(a_{i} \in A\) is assigned a unique on-chain identity and an initial token balance,
   representing its stake or “budget” of potential rewards. These tokens may be transferred, awarded, or forfeited based
   on the agent’s subsequent behavior.
2. 2.
   
   Policy evaluation and submission. After each round, agents perform localized updates to their strategies (e.g.,
   policy gradients or value-function refinements). To preserve traceability, the resulting model parameters or their
   cryptographic hashes are recorded on-chain, providing an immutable history of every agent’s policy evolution and
   facilitating accountability in case of disputes.
3. 3.
   
   Collusion and anomaly detection. Leveraging the chain’s historical data, we apply offline or near-real-time detection
   mechanisms to spot suspicious activity. In an automated bidding example, multiple agents consistently offering
   synchronized bids that stifle competition may be flagged for collusion; upon verification, they are subject to
   penalty assignment as per the smart contract’s terms. By contrast, an agent acting alone to manipulate prices without
   corroborating evidence from other agents might be categorized differently and subjected to alternative sanctions.
4. 4.
   
   Long-term reward maximization. Because our multi-agent reinforcement learning loops extend over multiple rounds, the
   underlying reward structure is carefully designed to prioritize sustained compliance and fair engagement over
   immediate, exploitative gains. Hyperparameters balancing short-term returns against future benefits ensure that
   agents discover and uphold cooperative (or at least non-manipulative) strategies yielding higher expected payoffs
   over repeated interactions. This long-horizon incentive alignment is reinforced by on-chain penalty mechanisms for
   misbehavior, adding the potential of future reward forfeiture to the immediate costs of dishonesty or collusion.

We characterize the blockchain performance by measuring two key quantities: the end-to-end confirmation latency, defined
as the elapsed time from transaction submission to final confirmation, and the transaction throughput, defined as the
number of confirmed transactions per unit of wall-clock time. For experimental load mapping, in the automated market
bidding domain we simulated 50 agents interacting over 200 rounds, while in the traffic signal coordination domain we
simulated 5 intersections operating over 300 steps.

In both cases we explicitly mapped each contract call frequency to the corresponding on-chain record per interaction
step so that the measured throughput and latency faithfully represent the actual execution profile. For sampling and
statistics, the first 10% of interaction steps were treated as a warm-up phase and excluded from analysis. During the
main runs, timestamps of all transaction confirmations were recorded, and we computed mean and standard deviation using
wall-clock time. Each reported value represents the average over three independent runs. The measured results showed
that in the automated market bidding domain the system achieved an average throughput of 15 transactions per second with
a consensus delay of 2.3 s per transaction, while in the traffic signal coordination domain the throughput was 12
transactions per second with a consensus delay of 1.9 s. These values were obtained under a permissioned blockchain
configuration with four validator nodes running a Practical Byzantine Fault Tolerance consensus protocol.

## Algorithmic framework

To illustrate the training and execution pipeline, we adopt Multi-Agent Soft Actor-Critic (MASAC) as a representative
algorithm. The following steps outline how the process is orchestrated in conjunction with a blockchain-based contract
interface.

In this description, let:
* \(\varepsilon\) be the multi-agent environment,
* C be the blockchain contract interface,
* T be the total number of training episodes.
1. 1.
   
   Initialization.

Each agent \(a_{i} \notin A\) begins with parameterized policy and value networks. Specifically, agent \(a_{i}\) has a
policy \(\pi_{{\theta_{i} }}\) and a value network \(Q_{{\phi_{i} }}\). The parameters \(\theta_{i}\) and \(\phi_{i}\)
are randomly initialized within stable bounds (e.g., using Xavier or orthogonal initialization). In parallel, the
blockchain smart contract is configured with the reward distribution logic, including detection thresholds for anomalous
behaviors (e.g., potential collusion) and predefined penalty rules. This setup ensures that upon completion of every
episode, the chain can automatically validate actions, distribute incentives, and penalize misconduct.
1. 2.
   
   Iterative training (for \(t = 1:t = 1\,{\text{to}}\,T\)).

Each training cycle is composed of an execution phase and a centralized training phase, followed by an on-chain reward
settlement.
1. 3.
   
   Environment reset.

At the start of each episode t, the environment ε is reset to an initial global state s0. All agent memories (e.g.,
replay buffers) are prepared to record subsequent transitions.
1. 4.
   
   Execution phase.

For each time step k within an episode, agent \(a_{i} \in A\) observes its local partial state \(o_{i}^{k}\) and samples
an action \(a_{i}^{k}\) from its current policy \(\pi _{{\theta _{i} }} (a_{i}^{k} \left| {o_{i}^{k} } \right.)\). The
environment then transitions to a new global state \(s_{{k + 1}}\) and furnishes each agent with an immediate reward
\(r_{i}^{k}\). Alongside these transitions, the quadruples \((o_{i}^{k} ,a_{i}^{k} ,r_{i}^{k} )\) (plus a timestamp) are
recorded on-chain via \(C\) to provide a transparent, immutable ledger of interactions.
1. 5.
   
   Centralized training.

After collecting trajectories from all time steps of the episode, a centralized learner aggregates the experience tuples

$$\left\{ {(s_{k} ,a_{k} ,r_{k} ,s_{{k + 1}} )} \right\} = \left\{ {(s_{k} ,\boldsymbol{a}_{k} ,\boldsymbol{r}_{k}
,s_{{k + 1}} )} \right\}$$

for all agents. These experiences are used to update the value and policy parameters via batch optimization:

$$\begin{aligned} & \nabla _{{\phi _{i} }} J_{Q} (\phi _{i} ),\nabla _{{\theta _{i} }} J_{\pi } (\theta _{i} ),\nabla
_{{\phi _{i} }} E[(Q_{{\phi _{i} }} (o_{i} ,a_{i} ) - y)^{2} ],\nabla _{{\theta _{i} }} J_{\pi } (\theta _{i} ) \\ &
\quad = \nabla _{{\theta _{i} }} E[ - Q_{{\phi _{i} }} (o_{i} ,\pi _{{\theta _{i} }} (o_{i} ))] \\ \end{aligned}$$

where \(J_{Q} (\phi _{i} )\) and \(J_{\pi } (\theta _{i} )\) represent the objective functions for updating the Q-value
network and the policy network, respectively. The exact forms of these losses follow the Soft Actor-Critic or MADDPG
frameworks, tailored to multi-agent settings^{[28][58],[29][59]}. Typically, the centralized critic or global state
information can be leveraged in the gradient computations to enhance training stability and address non-stationarity.
1. 6.
   
   Incentive/penalty settlement.

Upon completion of an episode, the blockchain contract automatically executes its rewardDistribution()function. Rewards
(or punishments) are computed based on aggregated performance metrics and consistency checks. If collusion or other
policy infractions are flagged by the on-chain detection logic, the offending agent(s) receive partial or total
forfeiture of tokens. Conversely, rule-abiding agents maintain or increase their token balances. These token adjustments
reinforce the incentive-compatible nature of the system, where cooperative or lawful strategies align with long-term
returns.
1. 7.
   
   Progression to the next episode.

Updated agent networks are retained for the next iteration, while any penalties or credits are permanently recorded
on-chain. Agents thus face evolving conditions in which past misbehavior influences future earning potential, further
encouraging compliant strategies over multiple episodes.
1. 8.
   
   Output.

After TTT training episodes, each agent \(a_{i} \in A\) possesses an updated policy \(\pi _{{\theta _{i} }} = \pi
(\theta _{i} )\) that ideally satisfies the designed incentive-compatibility criteria, balancing its own cumulative
reward with on-chain constraints. Additionally, a comprehensive record of every transaction, penalty, and reward
distribution is accessible on the blockchain, certifying that the final multi-agent mechanism is transparent, traceable,
and resistant to manipulative behaviors.

During the data collection phase, at each interaction step we continue to store the tuple ⟨local observation, action,
raw environment reward, timestamp, contract call handle⟩. At the end of an episode the rewardDistribution contract is
triggered, and the verified reward R̄_i together with the detection flags is synchronously written back into the replay
buffer. In the training phase, the critic updates are performed using R̄_i instead of the raw environment reward, with
the explicit rationale that this verified reward already incorporates redistribution and penalties, thereby correcting
for short-term gains that would otherwise be obtained from improper behavior. In the policy update objective we
introduce an additional weighting term to capture the impact of future settlement rules, described textually as
“reducing preference for action–state pairs that may trigger penalties and increasing preference for long-term compliant
behaviors,” while the entropy regularization remains unchanged. The settlement and update sequence follows a strict
“end-of-episode synchronous settlement” policy, and asynchronous write-backs are deliberately avoided to prevent signal
drift. We also specify that the order of random seed initialization, replay buffer sampling, and target network updates
is fixed across runs, which ensures full reproducibility of the training dynamics.

## Experimental design and evaluation

To validate the effectiveness of our proposed framework, we conducted experiments in two representative multi-agent game
environments, each reflecting a distinct combination of competitive and cooperative interactions. All trials were
carried out on a permissioned blockchain testbed, with a set of validator nodes responsible for consensus and execution
of smart contracts. Data were analyzed in Python using the numpy and matplotlib libraries to produce quantitative
metrics and visualizations.

### Automated market bidding

In this scenario, N agents represent bidders competing for a shared resource, such as advertising slots, across a series
of auction rounds. Each round follows a format akin to Vickrey or multi-price mechanisms, determining the ultimate
payment based on the second-highest or averaged bids. We deployed a consortium blockchain (with five validator nodes) to
record all bidding and settlement transactions. Smart contracts embedded collusion-detection logic, automatically
penalizing patterns that indicated coordinated underbidding or price manipulation.

We ran simulations with N = 100 bidders, each participating in 200 consecutive auction rounds. Agents used the MASAC
algorithm described in “Technical details”, with hyperparameters fine-tuned to balance exploration and exploitation.
Table [1][60] summarizes key metrics—Social Welfare, Individual Profit, Collusion Success Rate, and Incentive
Compatibility—in three configurations:
1. 1.
   
   Baseline (no blockchain): Standard multi-agent RL without on-chain enforcement.
2. 2.
   
   Blockchain without collusion detection: Bidding recorded on-chain, but no penalty for detected collusion.
3. 3.
   
   Full blockchain mechanism: All records on-chain plus the automated detection and penalty contract.

**Table 1 Performance metrics of different configurations in the automated market bidding environment.**
[Full size table][61]

From the table, the presence of blockchain alone improved social welfare by around 4% relative to the baseline, while
the introduction of explicit collusion detection and automated penalties further raised the overall welfare by roughly
10% compared to the baseline. Similarly, individual profits increased as fewer “price wars” and manipulative tactics
occurred, while the Collusion Success Rate (i.e., instances of effective underbidding coalitions) dropped substantially.
The Incentive Compatibility Index, measured as the ratio of honest-strategy returns to potential cheating-strategy
returns over 200 rounds, rose significantly under the full mechanism, indicating that rational bidders found greater
benefit in transparent, rule-abiding behavior (see Fig. [1][62]).

**Fig. 1**
[Fig. 1]
[Full size image][63]

Comparison of social welfare and collusion success rate under different market bidding configurations. The Full
Blockchain Mechanism achieves the highest social welfare while reducing collusion success rate to under 4%. In contrast,
the absence of blockchain results in lower welfare and a significantly higher rate of successful collusion.

In addition to the existing baselines, we have implemented MADDPG and COMA under settings aligned with our proposed
method. For both, the training duration, learning rate, network width, and exploration temperature were matched to the
values used in our framework to ensure fair comparison. Beyond MARL baselines, we also included two non-blockchain
mechanisms: the centralized trust authority baseline, in which a single coordinator collects agent actions, computes
rewards, and distributes outcomes while maintaining an internal log; and the cryptographic commitment scheme baseline,
in which agents first commit to their actions using hash-based commitments and later reveal them to enable verification
without a full blockchain. All baselines shared identical state and reward interfaces with our method to isolate the
effect of the coordination and enforcement mechanisms rather than input design.

The results demonstrate clear relative differences among the three categories of mechanisms. In terms of social welfare,
the blockchain-based MASAC consistently achieved the highest values, followed by the centralized trust authority, with
the commitment scheme trailing slightly above the no-blockchain condition. For collusion success rates, the blockchain
setup maintained the lowest values, the commitment scheme achieved moderate reduction, while the centralized authority
remained vulnerable to manipulation due to its single point of control. For the incentive compatibility index, the
blockchain configuration again scored highest, while the commitment scheme reached intermediate levels and the
centralized authority achieved only marginal improvements. Importantly, the centralized authority baseline exposed the
risk of single-point failure and limited transparency, as it could be both a bottleneck and a collusion target, while
the commitment scheme lacked automated enforcement and thus weakened long-term compliance. In contrast, the
blockchain-enhanced framework provided auditability and traceability by design, ensuring that rule violations were both
recorded and penalized without reliance on trust in a central party.

### Intelligent traffic coordination

Here, multiple agents each control the traffic signals at an intersection within a simulated city grid of five major
crossroads, aiming to reduce overall congestion during peak flow periods. Although each agent strives to minimize queue
length at its own intersection, overly extended green phases can increase wait times for connecting roads, thus
establishing a hybrid cooperative-competitive dynamic.

We integrated a lightweight blockchain system with three validator nodes, recording the timing of signal changes,
average vehicle queue lengths, and any emergent anomalies. The smart contract specified traffic-flow rules and a
congestion-penalty scheme wherein an intersection excessively favoring its own traffic would incur demerit points. If a
node’s penalty points exceeded a threshold, its subsequent token rewards were automatically reduced.

In our experiments, each simulation ran for 300 time steps under varying traffic inflow rates. We contrasted a baseline
multi-agent RL approach (QMIX) with our blockchain-enhanced MASAC strategy. Table [2][64] highlights three metrics:

**Table 2 Traffic coordination performance under two control strategies in a multi-agent intersection environment.**
[Full size table][65]

The Avg Wait Time was reduced by around 8% when blockchain-based cooperation was introduced, dropping from 45.3 s to
41.8 s. Fairness, measured as the variance of average wait times across intersections, improved significantly,
suggesting that intersections no longer attempted to “offload” their congestion at the expense of neighboring roads.
Finally, the Stability Index, computed as the proportion of time steps in which the system remained at or near a locally
optimal traffic distribution (within a certain threshold of queue lengths), increased from 0.72 to 0.83 under the
blockchain model.

We further assessed the system’s resilience to abrupt changes—such as a lane closure at one intersection—where
blockchain records enabled rapid detection and reallocation of signal priorities (see Fig. [2][66]). Traffic recovered
to its steady state in under 15 steps (on average), compared to over 25 steps for the baseline. A time-series comparison
of queue lengths is illustrated by the Python code snippet below, generating a line plot:

**Fig. 2**
[Fig. 2]
[Full size image][67]

Average queue length over time under a simulated traffic disruption event. The blockchain-enhanced MASAC model
demonstrates faster recovery and lower congestion levels compared to the baseline QMIX approach. This reflects superior
resilience in dynamic urban traffic conditions.

To further illustrate the system’s dynamic response and temporal-spatial coordination effectiveness, we visualized three
aspects of the experiment in Fig. [3][68]. Figure [3][69]a presents the recovery trajectory of average queue length
across all intersections following a simulated lane closure at time step $t = 0$. The Blockchain-MASAC model rapidly
reduces congestion, stabilizing queue lengths below 30 units within 15 steps, while the baseline QMIX system takes more
than 25 steps to reach comparable conditions. This supports the earlier observation of improved resilience and faster
convergence. Figure [3][70]b,c provide heatmap visualizations of per-intersection vehicle wait times over a 50-step
simulation window, contrasting the baseline and blockchain settings. In the baseline scenario, prolonged localized
congestion can be observed, especially in intersection 2 and 4, which persist throughout the episode. Conversely, the
Blockchain-MASAC approach results in visibly lower wait times with reduced temporal-spatial variance, indicating more
balanced signal allocation across intersections. Together, these visualizations offer strong evidence that the proposed
blockchain-enhanced coordination framework not only improves macro-level traffic efficiency but also enhances
fine-grained fairness and responsiveness in localized decision-making. It further confirms that decentralized,
incentive-driven control policies can outperform centralized heuristics under high-stakes, dynamic congestion
environments.

**Fig. 3**
[Fig. 3]
[Full size image][71]

Spatiotemporal performance comparison of traffic control strategies under disruption. (**a**) Recovery curve showing the
reduction in average queue length after a lane closure event. (**b**) Heatmap of intersection-level wait times using
baseline QMIX, with visible localized congestion. (**c**) Corresponding heatmap under Blockchain-MASAC, exhibiting lower
queue lengths and more balanced traffic distribution.

We further implemented QMIX-V2 and a centralized critic variant without blockchain enforcement as comparison points.
QMIX-V2 captures value decomposition learning under cooperative settings, while the centralized critic variant retains a
shared evaluator but omits any on-chain redistribution or penalty enforcement. Both methods achieved reasonable fairness
and partial stability but consistently underperformed compared to the blockchain-enhanced MASAC. Specifically, fairness
scores improved relative to plain QMIX but lagged behind the blockchain variant, and stability remained sensitive to
agents adopting extreme strategies without effective penalty mechanisms. In contrast, the introduction of on-chain
redistribution directly improved both fairness and stability, highlighting the unique contribution of blockchain
enforcement beyond the baseline cooperative learning architectures.

The observed improvement in stability index is directly attributable to the effect of on-chain penalties, which suppress
extreme signal extension strategies that otherwise destabilize coordination cycles. The reduction in variance across
runs reflects enhanced cross-intersection balance, since redistribution discourages opportunistic exploitation by single
agents and promotes more even outcomes across all controlled nodes.

### Policy robustness under noisy rewards

To further examine the resilience of our blockchain-enhanced multi-agent framework, we evaluated its robustness under
stochastic perturbations applied to the reward signals during training. In real-world decentralized systems such as
market bidding or distributed traffic control, agents may encounter noisy observations or delayed, imprecise reward
feedback due to latency, sensor errors, or adversarial interference. Therefore, an effective mechanism should not only
optimize incentives in ideal environments but also remain stable and effective under imperfect conditions.

We simulated an auction-based multi-agent system as described in “Automated market bidding”, involving N = 100 bidding
agents over 200 auction rounds. Three configurations were tested:
* Baseline (no blockchain): Standard multi-agent reinforcement learning without any blockchain infrastructure.
* Blockchain (no detection): All bidding data recorded on-chain, but without collusion penalty.
* Full blockchain mechanism: Complete blockchain integration including on-chain record-keeping, automated collusion
  detection, and incentive-compatible reward redistribution.

To simulate environmental uncertainty, we introduced Gaussian noise \(N(0,\sigma ^{2} )\) to the reward signals during
policy updates, with \(\sigma \in 0.0,0.1,0.2,0.3,0.4\). For each noise level, we repeated the training process over
five runs and measured the average cumulative reward across agents at convergence.

The results are illustrated in Fig. [4][72], which plots the average reward performance for each configuration under
increasing levels of reward noise.

**Fig. 4**
[Fig. 4]
[Full size image][73]

Reward robustness under noisy environment.

From the plot, it is evident that the Full Blockchain Mechanism consistently yields higher average returns than the
other two baselines, across all noise levels. Specifically:
* Under zero noise, the Full Blockchain agents achieve an average cumulative reward of approximately 1300, compared to
  1250 and 1200 for the partial and no-blockchain cases, respectively.
* As noise increases to \(\sigma = 0.4\), all configurations experience some degradation in performance. However, the
  performance drop is most severe in the No Blockchain setup, where rewards fall below 1020. In contrast, the Full
  Blockchain mechanism maintains an average above 1170, demonstrating greater resistance to perturbation.
* The gap between configurations becomes more pronounced under higher noise. For example, at \(\sigma = 0.3\), the Full
  Blockchain mechanism outperforms the baseline by approximately 14% in average reward.

These results validate the robustness of on-chain enforcement, particularly the role of immutable records and automated
smart contract-based redistribution in stabilizing learning dynamics. The system’s ability to preserve incentive
compatibility even in the presence of noisy or distorted feedback makes it suitable for deployment in real-wor

[Content truncated]
```
