# Web source

- URL: https://towardsdatascience.com/llm-routing-intuitively-and-exhaustively-explained-5b0789fe27aa
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T15:44:21.859295764+00:00

```text
[Skip to content][1]
[[Towards Data Science]][2]

Publish AI, ML & data-science insights to a global community of data professionals.

Sign in
[Submit an Article][3]
* [Latest][4]
* [Editor’s Picks][5]
* [Deep Dives][6]
* [Newsletter][7]
* [Write For TDS][8]
* [[Towards Data Science]][9]
Toggle Mobile Navigation
* [LinkedIn][10]
* [X][11]
Toggle Search
Search
[ Artificial Intelligence ][12]

# LLM Routing – Intuitively and Exhaustively Explained

Dynamically Choosing the Right LLM

[Daniel Warfield][13]
Nov 22, 2024
69 min read
Share

### Dynamically Choosing the Right Language Model on Every Query

["Harmony" by Daniel Warfield using Midjourney. All images by the author unless otherwise specified. Article originally
made available on Intuitively and Exhaustively Explained.]"Harmony" by Daniel Warfield using Midjourney. All images by
the author unless otherwise specified. Article originally made available on [Intuitively and Exhaustively
Explained][14].

In this article we’ll discuss "LLM routing", an advanced inferencing technique which can automatically choose the right
language model, out of a selection of language models, for a given prompt; improving the performance, speed, and cost in
LLM-powered systems.

We’ll explore four approaches to LLM routing: three from academia and one from industry, in order to form a thorough
understanding of the concept and technology. In doing so we’ll explore a variety of modeling strategies which are useful
in important AI use cases, like self-evaluation, autonomous systems, and decision making in the face of uncertainty.

**Who is this useful for?** Anyone who wants to forge a deeper understanding of AI, and some of the core approaches
necessary to make cutting edge AI powered systems.

**How advanced is this post?** Earlier sections of this article are accessible to readers of all levels. Later sections
are geared more towards data scientists and developers with some level of experience.

**Pre-requisites:** The earlier sections are accessible to readers of all levels, but later sections have some
supporting content which may prove necessary for some less experienced readers.

**Disclaimer:** During the time of writing, I am not affiliated with unify.ai. All opinions throughout this article are
my own and are unsponsored.

## A Problem: There Are a Lot of Language Models

Here’s [a list of 132,781 generative language models][15] (at the time of writing). How do you choose the right one?

One might choose to use whatever most people say the best model is. Maybe GPT-4o, Claude Opus, or Gemini Pro. You can
pass all your queries through a big fancy language model and you’ll get back state of the art responses. Let’s imagine
what some of those questions and answers might look like.

`User Query: What is the derivative of 3x^32x?
LLM Answer: (3x^32x)(32ln(x)+32)

User Query: what is 2+2?
LLM Answer: 4

User Query: Is water wet?
LLM Answer: The question of whether water is "wet" is a bit of a philosophical and semantic debate rather than a straigh
tforward scientific one. Here's how it breaks down...`

Using a best in class, expensive, multi-trillion parameter language model to answer 2+2=4 seems like a waste of
resources. The issue is, in many AI powered applications, we have no idea if a user will ask a simple or difficult
question. If we want a user to get a good response then we have to assume every query will be difficult, and thus we
need to use a big fancy model on even the simplest of queries.

The idea of LLM routing is to analyze queries coming in, and then decide which LLM might be best suited to answer that
query.

`User Query: What is the derivative of 3x^32x?
Router:     This is a complex query. Let's use GPT-4
GPT-4:      (3x^32x)(32ln(x)+32)

User Query: what is 2+2?
Router:     This is a simple query. Let's use Gemini Nano
Gemini Nano:(3x^32x)(32ln(x)+32)

User Query: Is water wet?
Router:     This is a common question. Let's use GPT-4o
GPT-4o:     The question of whether water is "wet" is a bit of a philosophical and semantic debate rather than a straigh
tforward scientific one. Here's how it breaks down...`

The power of LLM routing chiefly comes into play when one wants to reduce cost while maintaining performance. There are
a lot of different papers and products exploring the idea of LLM Routing in different ways. Let’s start with AutoMix.

## AutoMix and the LLM Cascade

Before we discuss AutoMix, I invite you to think about how you might solve an LLM routing problem. Let’s say you have
three models: Claude Haiku, Sonnet, and Opus, each with very different cost to performance tradeoffs.

[A high-level breakdown of the performance tradeoff of different Anthropic models, and how they compare with a few other
popular LLMs, across a variety of benchmarks. From the Claude 3 Model Family paper.]A high-level breakdown of the
performance tradeoff of different Anthropic models, and how they compare with a few other popular LLMs, across a variety
of benchmarks. From the [Claude 3 Model Family][16] paper.

Imagine you were tasked to build a system that could answer incoming queries correctly while minimizing cost. What
approach would you take?

Your first intuition might be to develop something called a "LLM cascade" which is exactly what is proposed in both the
[FrugalGPT][17] and [AutoMix][18] papers.

In an LLM cascade you pass a query to the least expensive LLM you have, then ask that same model if the query was
answered adequately. If the small model judged that it’s own answer was correct, then you return the answer from the
small model to the user. If the small model’s answer was not deemed correct, you try the same process with a larger
model.

[(1) we pass a query to a small LLM, which (2) generates a response. (3) We then ask the same small LLM if the question
was answered correctly. (4) if it was, return the result. (5) if the small LLM was incorrect, we try again but with a
larger LLM.](1) we pass a query to a small LLM, which (2) generates a response. (3) We then ask the same small LLM if
the question was answered correctly. (4) if it was, return the result. (5) if the small LLM was incorrect, we try again
but with a larger LLM.

This approach can be practical because smaller models can be much, much less expensive than larger models.

[Even though using and evaluating the small model can add cost, that additional cost is much less than the cost savings
of ocassionally not using the larger model.]Even though using and evaluating the small model can add cost, that
additional cost is much less than the cost savings of ocassionally not using the larger model.

Naturally, an LLM cascade is very dependent on both the users queries and the models chosen, which is where the
simplicity of the approach can be a tremendous asset. Because there is no training involved it’s incredibly easy to set
up and modify a cascade at will.

If you can whip this up in 5 minutes and see a 10x reduction in LLM inference cost with negligible performance impact,
then that’s pretty neat. However, an issue with this simple approach is that you are likely to see a significant
performance drop.

The issue lies in self-evaluation. A lot of the time our smaller language model will be able to tell if it got the
answer wrong, but sometimes the smaller model won’t be able to detect its own mistakes.

[Notice how in step (2) the small model said that stonehenge was 10 years old. In step (3), the small model thought the
result sounded plausible and chose to return that result rather than pass the result to the larger model.]Notice how in
step (2) the small model said that stonehenge was 10 years old. In step (3), the small model thought the result sounded
plausible and chose to return that result rather than pass the result to the larger model.

Because LLM cascades rely on self-evaluations to decide whether to continue to a larger model or return the current
response, a poor self-evaluation can significantly inhibit the quality of the final output. AutoMix employs something
called a "Partially Observable Markov Decision Process" based on "Kernel Density Estimation" to alleviate this problem.
Let’s unpack those ideas:

## A High-Level Intro to POMDPs

A "Partially Observable Markov Decision Process" (POMDP) is an extension of something called a "Markov Decision Process"
(MDP).

A Markov Decision Process (MDP) is a way of modeling the types of states a system can be in, and the actions that system
can take to transition between states. Say you have a robot, for instance, and you want to allow that robot to navigate
through an environment.

[Imagine we want to get the robot to the goal. Robot image generated with MidJourney.]Imagine we want to get the robot
to the goal. Robot image generated with MidJourney.

You can construct a graph of the possible states that robot can occupy, as well as the cost to transition between
states.

[We can put several pre-defined states within the space. We can also define a cost of every transition, like the
distance traveled, which we would like to minimize.]We can put several pre-defined states within the space. We can also
define a cost of every transition, like the distance traveled, which we would like to minimize.

Once the graph is set up, you can analyze that graph to calculate the best course of action.

[The best course of action to reach the goal, by traversing through states, with respect to minimizing total distance
covered.]The best course of action to reach the goal, by traversing through states, with respect to minimizing total
distance covered.

This is called a "Markov Decision Process", and is a super powerful tool in modeling complex systems.

One problem with a classic Markov Decision Process is instability. Imagine we tell our robot to follow the plan to reach
the goal, and then the robot’s wheel slips halfway down a hallway; resulting in the robot turning slightly. If we
continue executing our pre-defined instructions as before, the robot will get stuck in a corner rather than reach its
destination.

[Imagine the robot slips, and turns slightly. Because the robot executed the defined steps to reach state 2, it believes
it's in state 2. If the robot continues to execute the original plan, it will go further and further off course.]Imagine
the robot slips, and turns slightly. Because the robot executed the defined steps to reach state 2, it believes it’s in
state 2. If the robot continues to execute the original plan, it will go further and further off course.

"Partially Observable" Markov Decision Processes (POMDP) are designed to alleviate this problem. The idea of a POMDP is
that we assume we never know the true state of the robot, but rather we can make observations and form a probabilistic
belief about the state of the system.

If we slap some sensors on our robot, and have it navigate our environment, we can use our sensors to check if we think
the robot has ended up in the correct spot. The sensor might not be able to perfectly identify where we are, but we can
use our best guess to make a reasonable decision.

[The robot makes an observation about its surroundings then decides on an action based on a guess about it's true
state.]The robot makes an observation about its surroundings then decides on an action based on a guess about it’s true
state.

Let’s explore how AutoMix employs POMDP’s to support the LLM Cascade. In doing so, we’ll explore some of the inner
workings of POMDP’s more in depth.

## AutoMix’s POMDP Supported LLM Cascade

Full Code for the AutoMix Portion of this article can be found here:

> [**MLWritingAndResearch/AutoMix.ipynb at main · DanielWarfield1/MLWritingAndResearch**][19]

Recall that an LLM Cascade uses self evaluation to either return the response from a smaller language model or pass the
prompt to a larger model.

[Recall the diagram of an LLM cascade from earlier in the article, where (1) we pass a query to a small LLM, which (2)
generates a response. (3) We then ask the same small LLM if the question was answered correctly. (4) if it was, return
the result. (5) if the small LLM was incorrect, we try again but with a larger LLM.]Recall the diagram of an LLM cascade
from earlier in the article, where (1) we pass a query to a small LLM, which (2) generates a response. (3) We then ask
the same small LLM if the question was answered correctly. (4) if it was, return the result. (5) if the small LLM was
incorrect, we try again but with a larger LLM.

The main idea of AutoMix is, instead of taking the self-evaluations of a model at face value, we turn them into a
probabilistic "observation" which hints at the performance of the LLM, then we use that probabilistic observation to
decide what action we should take.

To turn a binary "yes or no" self-evaluation into a probability, the authors of AutoMix ask the language models to
self-evaluate numerous times with a high temperature setting. Temperature increases how erratic a language models
responses are by allowing an LLM to accept output that is occasionally less optimal. If we choose a very high
temperature rating, and ask the model to self-evaluate a few times, it allows us to build a probability distribution of
self evaluation based on how many times the model says the answer was acceptable or not.

First, we can use langChain’s `with_structured_output` to get a binary true or false evaluation for if an LLM thinks the
answer is correct.

`"""Creating an "evaluator" using langchain's "with_structured_output".
Basically, this function defines a class which represents the data we want from
the LLM (SelfEval), then langchain uses that class to format the LLMs response into a true
or false judgement of if the model was accurate or not. This allows us to ask an LLM if an
answer was correct or not, and then get back a boolean.

I also have the model form a rationale, before constructing the boolean, serving as a form of
chain of thought.

we specify a high temperature, meaning using an evaluator multiple times
can result in a distribution of evaluations due to high model randomness
"""

from typing import TypedDict
from langchain_openai import ChatOpenAI
from langchain_core.prompts import ChatPromptTemplate

def create_evaluator(model):

    #Defines the structure of the output
    class SelfEval(TypedDict):
        rationale: str
        judgement: bool

    #The system prompt provided to the model
    #prompt lightly modified from AutoMix paper
    self_eval_prompt = ChatPromptTemplate.from_messages(
        [
            (
                "system",
                """Instruction: Your task is to evaluate if the AI Generated Answer is correct or incorrect based on the
provided context and question. Provide ultimate reasoning that a human would be satisfied with, then choose between
Correct (True) or Incorrect (False).
                """,
            ),
            ("placeholder", "{messages}"),
        ]
    )

    #creating a lang chang that outputs structured output
    evaluator = self_eval_prompt | ChatOpenAI(
        model=model, temperature=1 #setting a high temperature
    ).with_structured_output(SelfEval)

    return evaluator`

Then we can have a model answer some question

`"""Having an LLM answer a riddle

There was a plane crash in which every single person was killed.
Yet there were 12 survivors. How?
"""

from openai import OpenAI

model = 'gpt-3.5-turbo'

context = """There was a plane crash in which every single person was killed. Yet there were 12 survivors. How?"""
question = "Solve the riddle"

client = OpenAI(api_key=api_key)
response = client.chat.completions.create(
        model=model,
        messages=[
            {"role": "user", "content": f"context:n{context}nnquestion:n{question}"}
            ],
    )

answer = response.choices[0].message.content.strip()`

We can use the evaluator we defined to ask the model to evaluate it’s own answer a few times, and construct a normal
distribution based on how many true and false self evaluations there were.

`""" Constructing a normal distribution (a.k.a. bell curve, a.k.a gaussian),
based on 40 self evaluations, showing how likely an answer was right or
wrong based on several LLM self-evaluations.

Gaussians have two parameters:
- the mean, or center of the distribution: calculated as just the average value
- the standard deviation: which is how spread out the values are.

The funciton `gaussianize_answer` runs self eval some number of times,
gets a distribution of self evaluations saying the response was good
or poor, then constructs a gaussian describing that overall distribution.
"""

import numpy as np
import matplotlib.pyplot as plt
from scipy.stats import norm

def gaussianize_answer(context, question, answer):
    num_evaluations = 40
    evaluations = []
    evaluator = create_evaluator(model)

    for _ in range(num_evaluations):

        for i in range(2):
            #allowing the evaluator to make several attempts at judgements
            #and wrapping it in a try/catch to deal with the odd parsing error.
            try:
                evaluation = evaluator.invoke({"messages": [("user", f"""Context: {context}
            Question: {question}
            AI Generated Answer: {answer}""")]})

                evaluations.append(evaluation['judgement'])
                break
            except KeyboardInterrupt as e:
                raise e
            except:
                print('evaluator error')
        else:
            print('too many errors, skipping evaluation step')

    # Calculate probability (mean) of evaluations
    probability = sum(evaluations) / len(evaluations)

    # Calculating mean and standard deviation, which define a gaussian
    mean = probability
    std_dev = np.sqrt(probability * (1 - probability) / len(evaluations))

    return mean, std_dev

mean, std_dev = gaussianize_answer(context, question, answer)

#cant draw gaussian if there's perfect consensus
if mean != 0 and mean !=1:
    # Create a range for x values
    x = np.linspace(0, 1, 100)
    y = norm.pdf(x, mean, std_dev)

    # Plot the Gaussian
    plt.plot(x, y, label=f'Gaussian DistributionnMean={mean:.2f}, Std Dev={std_dev:.2f}')
    plt.title("Gaussian Distribution of True Value Probability")
    plt.xlabel("Probability")
    plt.ylabel("Density")
    plt.legend()
    plt.show()`

[This curve can be thought of as the confidence that the LLM answered the question correctly. The model appears to be
unsure as to whether it's own answer was correct or incorrect, as the probability that the answer was correct is near
0.5]This curve can be thought of as the confidence that the LLM answered the question correctly. The model appears to be
unsure as to whether it’s own answer was correct or incorrect, as the probability that the answer was correct is near
0.5

Without using a POMDP, one could simply apply a threshold to these probabilities and use them to make decisions about
how to navigate through the cascade, possibly seeing an improvement over using individual self evaluation results.
However, self evaluations are know to be noisy and unreliable. Let’s do some self-evaluations on several answers and
overlay the distributions of correct and incorrect answers to explore just how noisy self-evaluation can be:

`"""Creating normal distributions based on self evaluations for a few answers.
Recall that the riddle was the following:

There was a plane crash in which every single person was killed. Yet there were 12 survivors. How?
"""

#A selection of a few hardcoded LLM answers
llm_answers = []
#correct
llm_answers.append("The 12 survivors were married couples.")
llm_answers.append("The people on the plane were all couples - husbands and wives.")
llm_answers.append("The answer to this riddle is that the 12 survivors were married couples.")

#incorrect 
llm_answers.append("The riddle is referring to the survivors being the 12 months of the year.")
llm_answers.append("The riddle is referring to the survivors as the numbers on a clock (numbers 1-12). So, the answer is
 that the "12 survivors" are actually the numbers on a clock.")

#evaluating all answers
distributions = []
for llm_answer in llm_answers:
    mean, std = gaussianize_answer(context, question, llm_answer)
    distributions.append((mean, std))`

Here, the first three answers are correct answers to the riddle, while the final two answers are incorrect answers to
the riddle. We can plot these distributions to see how well our auto-evaluation strategy can separate good and bad
answers.

`"""Plotting the gaussians we created in the previous code block.
Correct answers are dotted, wrong answers are solid.
"""

fig = plt.figure(figsize=(10, 6))

#plotting all gaussians
for i, dist in enumerate(distributions):
    #unpacking tuple
    mean, std = dist

    name = f'LLM answer {i}'

    #labeling the two clearly wrong answers as dotted lines (i=3 and i=4)
    if i>=3:
        stroke = '-'
    else:
        stroke=':'

    if std == 0:
        plt.plot([mean,mean],[0,1], linestyle=stroke, label=name)
    else:
        # Create a range for x values
        x = np.linspace(0, 1, 100)
        y = norm.pdf(x, mean, std)
        plt.plot(x, y, linestyle=stroke, label=name, alpha=0.5)
plt.title("Gaussian Distribution of True Value Probability Across Answers")
plt.xlabel("Probability")
plt.ylabel("Density")
plt.legend()
plt.show()`

[The probability distribution of self-evaluations for wrong answers (solid) and correct answers (dotted)]The probability
distribution of self-evaluations for wrong answers (solid) and correct answers (dotted)

And… It did a pretty bad job. The distributions for good and bad answers are all mixed up. Language models are
frustratingly bad at knowing if their own answers are wrong or not. If we use this self-evaluation strategy as-is, our
LLM cascade will likely make wrong decisions; sometimes triggering expensive models when it shouldn’t and sometimes
returning bad responses when it shouldn’t.

This is where the "partially observable" aspect of POMDPs comes in. Instead of assuming the self-evaluations we’re
constructing are accurate, we can treat it like an observation and use that observation to try to predict if an answer
is really good or not.

In the AutoMix paper they do that through a process called Kernel Density Estimation (KDE). Say you have a model, and
you have a handful of examples where you know the model answered the question correctly and incorrectly. You can do
self-eval on question-answer pairs to figure out where self-evaluations typically end up when the models answer is
actually correct or incorrect.

In other words, we’re going to build a dataset of self-evaluation scores where we know the model’s answer is right, and
another set where we know the models answer is wrong, then we’re going to use that data to make routing decisions.

Here I’m constructing a synthetic dataset of self-evaluations for correct and incorrect answers, but you might imagine
running the `gaussianize_answer` function we previously defined on a bunch of questions and answers to create this
dataset.

`import numpy as np
import matplotlib.pyplot as plt
import pandas as pd
import seaborn as sns

# Creating a synthetic dataset with self evaluation results
# and actual performance results.
np.random.seed(0)

n_each = 50

selfeval_bad = np.random.normal(0.55, 0.05, n_each)    # Lower confidence around 0.4
selfeval_good_1 = np.random.normal(0.9, 0.03, n_each)  # Higher confidence around 0.7
selfeval_good_2 = np.random.normal(0.6, 0.03, n_each)  # Higher confidence around 0.6
selfeval_good = np.concatenate([selfeval_good_1, selfeval_good_2]) #combining both good distributions
self_eval = np.concatenate([selfeval_good_1, selfeval_good_2, selfeval_bad])
true_performance = [1] * (n_each*2) + [0] * n_each

#plotting a swarm plot
df = pd.DataFrame()
df['true_performance'] = true_performance
df['self_eval'] = self_eval

ax = sns.swarmplot(x="true_performance", y="self_eval", data=df)
plt.title("Training Dataset with True Performance vs Self Evaluation")

plt.show()`

[Imagine we had 150 question and answer pairs, where 50 of them were actually bad answers (true_performance=0) and 100
of them were actually good answers (true_performance=1). Then we ran self-evaluation to construct a probability of if
each of those answers were right, and plotted the mean of those distributions (self_eval)]Imagine we had 150 question
and answer pairs, where 50 of them were actually bad answers (true_performance=0) and 100 of them were actually good
answers (true_performance=1). Then we ran self-evaluation to construct a probability of if each of those answers were
right, and plotted the mean of those distributions (self_eval)

The idea of kernel density estimation (KDE) is to turn these individual examples into smooth density distributions such
that we can use them to calculate the probability of a truly good answer. In other words, we want to be able to say "I
know self-eval said there was a 50% chance the answer is good, but based on a dataset of self-evaluations I know that
almost certainly means the answer is wrong because bad answers at a self evaluation of 0.5 are way more dense than good
answers at 0.5". KDEs allow us to do that.

To construct a KDE you first put a gaussian (a.k.a. kernel, a.k.a bell curve) on every point in a distribution:

`"""Placing a gaussian with an average value equal to every point
in the distribution of self evaluation results for good predictions
The standard deviation can be modified as necessary. Here I'm defining the
standard deviation as 0.05 for all gaussians, but they could be larger
or smaller, resulting in smoother or more sensitive KDEs
"""

import matplotlib.gridspec as gridspec
from scipy.stats import norm

fig = plt.figure(figsize=(10, 6))

# Creating a gaussian distribution with a small deviation on every point in a set of data
gs = gridspec.GridSpec(2, 1, height_ratios=[2, 1])
ax1 = fig.add_subplot(gs[0])
std = 0.05
for mean in selfeval_good:
    x = np.linspace(0, 1, 100)
    y = norm.pdf(x, mean, std)
    plt.plot(x,y)
plt.xlim([0, 1])
plt.xlabel("Gaussians built on good evaluations")

ax2 = fig.add_subplot(gs[1])
sns.swarmplot(x = 'self_eval', data = df[df['true_performance']==1])
plt.xlim([0, 1])
plt.xlabel("Individual good self evaluations")`

[gaussians centered on every point in the distribution of self evaluations for answers that are actually good]gaussians
centered on every point in the distribution of self evaluations for answers that are actually good

Then we can add them all up to create a smooth volume of predictions. The more predictions there are in a region, the
larger the volume is.

`"""modifying the code of the previous code block. Instead of saving
many gaussians, each gaussian is added to the same vector of values
In other words, wer're stacking all the gaussians on top of eachother
"""

import matplotlib.gridspec as gridspec
fig = plt.figure(figsize=(10, 6))

# Creating a gaussian distribution with a small deviation on every point in a set of data
gs = gridspec.GridSpec(2, 1, height_ratios=[2, 1])
ax1 = fig.add_subplot(gs[0])
std = 0.05
y = np.zeros(100)
for mean in selfeval_good:
    x = np.linspace(0, 1, 100)
    y += norm.pdf(x, mean, std)
plt.plot(x,y)
plt.xlim([0, 1])
plt.xlabel("Sum of all gaussians built on all good evaluations")

ax2 = fig.add_subplot(gs[1])
sns.swarmplot(x = 'self_eval', data = df[df['true_performance']==1])
plt.xlim([0, 1])
plt.xlabel("Individual good self evaluations")`

[Volume of predictions from distribution]Volume of predictions from distribution

In this graph the y axis doesn’t really mean anything. The more data you have, the taller this graph will be, meaning
the y axis is dependent on both density and the number of total predictions. Really, we’re just interested in density,
so we can calculate the area under the curve of the sum of gaussians then divide by that area. That will mean,
regardless of how many predictions you have the area under the curve will always be 1, and the height of the curve will
be the relative density of one region vs another, rather than being influenced by the number of samples you have.

`"""Modification of the previous code block to create a density plot
Calculating the area under the curve, then dividing the values
by that area. This turns a vague volume of results into a density
distribution, essentially getting rid of the impact that larger numbers
of samples tend to make the y axis taller.
"""

import matplotlib.gridspec as gridspec
fig = plt.figure(figsize=(10, 6))

# Creating a gaussian distribution with a small deviation on every point in a set of data
gs = gridspec.GridSpec(2, 1, height_ratios=[2, 1])
ax1 = fig.add_subplot(gs[0])
std = 0.05
y = np.zeros(100)
for mean in selfeval_good:
    x = np.linspace(0, 1, 100)
    y += norm.pdf(x, mean, std)

#converting to density (total area under the curve, regardless of the number
#of samples, will be equal to 1. Densities beteen distributions are comperable even
#if the number of samples are different)
area_under_curve = np.trapz(y, dx=1/100)
y = y/area_under_curve

plt.plot(x,y)
plt.xlim([0, 1])
plt.xlabel("Density of good evaluations")

ax2 = fig.add_subplot(gs[1])
sns.swarmplot(x = 'self_eval', data = df[df['true_performance']==1])
plt.xlim([0, 1])
plt.xlabel("Individual good self evaluations")`

And thus we’ve constructed a KDE. I know I’ve been throwing terms around like crazy , so I want to take a moment to
reiterate what this graph represents.

We start with a bunch of answers to questions by an LLM, and we get each of those dots in the graph above by asking the
LLM to self evaluate a few times with a high temperature on the same question and answer, turning each answer into a
probability. Self-evaluations are often noisy and inconsistent, so what we would like to do is find what self-evaluation
scores are the most likely to actually correspond to a correct answer. To do that, we’re looking at the self-evaluation
scores of actually good answers, and finding the region in which self-evaluation scores are more dense. If we have a
self evaluation score which lies in a region where there is a high density of actual good answers, then it’s probably
more likely to be a good self-evaluation.

All that code we made to construct the kernel density estimation can be replaced with `scipy.stats.gaussian_kde`. We can
use that function for the self evaluation of both truly good answers and truly bad answers to get an idea of which
self-evaluation values are more likely given a good or bad answer.

`from scipy.stats import gaussian_kde

# Perform KDE for each performance state
good_kde = gaussian_kde(selfeval_good, bw_method=0.3)
poor_kde = gaussian_kde(selfeval_bad, bw_method=0.3)

# Define a range of confidence scores for visualization
confidence_range = np.linspace(0, 1.0, 200)

# Evaluate KDEs over the range of confidence scores
good_density = good_kde(confidence_range)
poor_density = poor_kde(confidence_range)

# Plot the KDE for each performance state
plt.figure(figsize=(10, 6))
plt.plot(confidence_range, poor_density, label="Density of Self Eval for Poor Results")
plt.plot(confidence_range, good_density, label="Density of Self Eval for Good Results")
plt.title("KDE of Confidence Scores for Good and Poor Performance")
plt.xlabel("Confidence Score Through Self Evaluation")
plt.ylabel("Density of Predictions")
plt.legend()
plt.show()`

[The kernel density estimation for good answers and bad answers, overlayed.]The kernel density estimation for good
answers and bad answers, overlayed.

So, if we had a self evaluation of 60%, for instance, we could calculate the probability of if it would actually be a
good answer by comparing the relative densities of good and bad answers in that region.

`# Plot the KDE for each performance state
plt.figure(figsize=(10, 6))
plt.plot(confidence_range, poor_density, label="Density of Self Eval for Poor Result", alpha=0.7)
plt.plot(confidence_range, good_density, label="Density of Self Eval for Good Result", alpha=0.7)

#plotting the probabilities of good or bad at a given location
sample_confidence = 0.6
conf_poor = poor_kde(sample_confidence)[0]
conf_good =  good_kde(sample_confidence)[0]
label = f'Good Probability: {int(conf_good/(conf_poor+conf_good)*100)}%'
plt.plot([sample_confidence]*3,[0,conf_poor, conf_good], 'r', linewidth=3, label=label)
plt.plot([sample_confidence]*2,[conf_poor, conf_good], 'r', marker='o', markersize=10)

plt.title("KDE of Confidence Scores for Good and Poor Performance States")
plt.xlabel("Confidence Score Through Self Evaluation")
plt.ylabel("Density of Predictions")
plt.legend()
plt.show()`

[At a self-evaluation of 60%, poor answers are more dense than good answers, so we might say the probability of a good
answer is based on the relative size of those densities.]At a self-evaluation of 60%, poor answers are more dense than
good answers, so we might say the probability of a good answer is based on the relative size of those densities.

We can sweep through the entire x axis and calculate the probability that an answer is good, based on the ratio of
densities of the known training data, across all possible self-evaluation results

`plt.figure(figsize=(10, 6))
plt.title("Probability that the prediction is correct based on the difference of the self evaluation distributions")
good_probability = np.array([good/(good+poor) for (good, poor) in zip(good_density, poor_density)])
plt.plot(confidence_range, good_probability)
plt.xlabel("Confidence Score Through Self Evaluation")
plt.ylabel("Probability Correct")`

This is pretty nifty, but you might notice a problem. At a self-evaluation score of 0.2, for instance, the probability
(based on the ratio of densities) is 100% not because there are a lot of examples of good predictions at that point, but
because there aren’t any samples in that region.

To deal with this, the AutoMix paper also talks about constructing a KDE across all the data, and only keeping results
that have self-evaluations that lie within dense regions of the dataset.

`"""Creating a graph of probability that an answer is right
and also a graph of the density of all the training data
"""

import matplotlib.gridspec as gridspec
fig = plt.figure(figsize=(10, 6))

total_kde = gaussian_kde(self_eval, bw_method=0.3)
total_density = total_kde(confidence_range)
height_normalized_total_density = total_density/max(total_density)
confidence_threshold = 0.5
density_threshold = 0.1

#marking 
gs = gridspec.GridSpec(2, 1, height_ratios=[2, 1])
ax1 = fig.add_subplot(gs[0])
plt.plot(confidence_range, good_probability)
threshold = 0.5
plt.fill_between(confidence_range, good_probability, 0, where=(good_probability > confidence_threshold), color='blue', a
lpha=0.2, label=f'Probability based on density ratios > {confidence_threshold}')
plt.ylabel("Probability Correct")
plt.legend()

ax2 = fig.add_subplot(gs[1])
plt.plot(confidence_range, height_normalized_total_density, label="Normalized density of all self evaluations", alpha=1)
plt.fill_between(confidence_range, height_normalized_total_density, 0, where=(height_normalized_total_density > density_
threshold), color='blue', alpha=0.2, label=f'Total normalized density > {density_threshold}')
plt.xlabel("Confidence Score Through Self Evaluation")
plt.ylabel("Density of Predictions")
plt.legend()
plt.show()`

[As well as calculating the probability of an answer being correct based on the ratio of densities (top) we can also
construct a density plot across all data, effectively allowing us to know where our probabilities are derived from
sufficient data.]As well as calculating the probability of an answer being correct based on the ratio of densities (top)
we can also construct a density plot across all data, effectively allowing us to know where our probabilities are
derived from sufficient data.

So, we can say a self-evaluation is good if the probability of a prediction is good *and* if the self evaluation exists
within a region where there is a fair amount of training data. Even though the probability might be high at 0.2, we know
there’s no data at that point, so we would be skeptical of that self-evaluation.

With LLMs, there is generally a tradeoff between cost and performance. We might be willing to accept different
probabilities that an answer is good depending on the cost and performance constraints of our use case. We can balance
cost and performance by changing the threshold at which we decide to call the larger LLM given a smaller LLM’s
self-evaluation.

`"""Rendering a gradient of cost/performance tradeoff over the graphs
"""

import matplotlib.gridspec as gridspec
fig = plt.figure(figsize=(10, 6))

total_kde = gaussian_kde(self_eval, bw_method=0.3)
total_density = total_kde(confidence_range)
height_normalized_total_density = total_density/max(total_density)
confidence_threshold = 0.5
density_threshold = 0.1

cost_based_thresholds = [0.0,0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8,0.9]

#marking 
gs = gridspec.GridSpec(2, 1, height_ratios=[2, 1])
ax1 = fig.add_subplot(gs[0])
plt.plot(confidence_range, good_probability)
threshold = 0.5

threshold = cost_based_thresholds[0]
plt.fill_between(confidence_range, good_probability, 0, where=(np.logical_and(good_probability > threshold, height_norma
lized_total_density > density_threshold)), color='red', alpha=0.2, label=f'bound by cost')

for threshold in cost_based_thresholds[1:-2]:
    
plt.fill_between(confidence_range, good_probability, 0, where=(np.logical_and(good_probability > threshold, height_norma
lized_total_density > density_threshold)), color='red', alpha=0.2)

threshold = cost_based_thresholds[-1]
plt.fill_between(confidence_range, good_probability, 0, where=(np.logical_and(good_probability > threshold, height_norma
lized_total_density > density_threshold)), color='red', alpha=1, label=f'bound by performance')

plt.ylabel("Probability Correct")
plt.legend()

ax2 = fig.add_subplot(gs[1])
plt.plot(confidence_range, height_normalized_total_density, label="Normalized density of all self evaluations", alpha=1)
for threshold in cost_based_thresholds:
    
plt.fill_between(confidence_range, height_normalized_total_density, 0, where=(np.logical_and(good_probability > threshol
d, height_normalized_total_density > density_threshold)), color='red', alpha=0.2)
plt.xlabel("Confidence Score Through Self Evaluation")
plt.ylabel("Density of Predictions")
plt.legend()
plt.show()`

[We probably only want self-evaluation results that are within a region of self-evaluation scores that have training
examples, so that we're making decisions based on known data (bottom graph), but we might be willing to tolerate lower
probabilities of correct output at a lower cost in some use cases (top graph).]We probably only want self-evaluation
results that are within a region of self-evaluation scores that have training examples, so that we’re making decisions
based on known data (bottom graph), but we might be willing to tolerate lower probabilities of correct output at a lower
cost in some use cases (top graph).

So, based on training data we created of self-evaluations, paired with annotations of if the answers that were
self-evaluated were good or bad, we can build a probability distribution of if the answer is actually good or bad, and
have a degree of confidence based on the density of our training data within that region. We can use this data to make
decisions, thus allowing us to make "observations" about what we think our true performance likely is.

I’ll be skipping some fairly verbose code. Feel free to check out the full code:

> [**MLWritingAndResearch/AutoMix.ipynb at main · DanielWarfield1/MLWritingAndResearch**][20]

Imagine we have three LLMS with different abilities to self-evaluate.

[Three different models where, when you ask them to self evaluate, some are more able to distinguish good from bad
answers much better than others. We know that by comparing the KDEs of these three models. Notice how the KDE for the
small model is much more overlapped than the KDE for the large model.]Three different models where, when you ask them to
self evaluate, some are more able to distinguish good from bad answers much better than others. We know that by
comparing the KDEs of these three models. Notice how the KDE for the small model is much more overlapped than the KDE
for the large model.

Based on this data, you could use this information to say "I have __% confidence that this answer is actually correct,
based on a comparison of a given self evaluation with a dataset of self-evaluations on answers of known quality."

[Ability to evaluate confidence because of the KDEs. Essentially, this graph is comparing a models self-evaluation score
for some new answer with the self-evaluation scores the model gave for known good and bad answers.]Ability to evaluate
confidence because of the KDEs. Essentially, this graph is comparing a models self-evaluation score for some new answer
with the self-evaluation scores the model gave for known good and bad answers.

We can feed a query to our tiniest model, have it self-evaluate, and decide if the self evaluation is good enough based
on our KDEs for that model. If it’s not, we can move onto a bigger model. We do that until we’re happy with our output.

[Using KDEs to make decisions about routing, which is the POMDP. Code can be found here]Using KDEs to make decisions
about routing, which is the POMDP. Code can be found [here][21]

You might notice text about "reward" and "cost" in that output. In LLM routing there’s a fundamental tradeoff between
performance and cost. If you want better performance it’ll be more expensive. If you want lower cost you need to deal
with less performance. AutoMix uses the parameter λ (lambda) to control that tradeoff.

We want to consistently achieve high performance at a low cost, but are willing to balance between the two based on λ.
If we care more about performance we might use a small λ value, and if we care more about cost we might choose a large λ
value. This tradeoff is defined in the reward function:

[The reward function of AutoMix. If you have a high probability that the model is right then the reward goes up. If you
have a high cost, the reward goes down.]The reward function of AutoMix. If you have a high probability that the model is
right then the reward goes up. If you have a high cost, the reward goes down.

This can actually get pretty complicated. Formally, in a POMDP we need to account for all probabilities and all costs of
all models when making a decision. I’m planning on tackling POMDPs more in depth at some point in the future, and this
article has taken long enough to come out, so I simply made the reward function equal to the probability of the current
output being good minus λ times the cost of the next model. So, instead of looking at all possibilities of all future
models, I’m simply saying "is the current output good enough vs the cost to try again with the next model, based on λ".

Again, full code can be found [here][22].

`"""Constructing the POMDP and running it with simulated inferences
This is a simplification of the POMDP which just looks at the probability
of the current self evaluation and the cost of the next model.
"""
class Model:
    def __init__(self, name, kde_good, kde_poor, kde_density, good_threshold, cost):
        """
        Initialize each model with KDEs for good/poor predictions, a good_threshold for trusting its output,
        and a cost associated with using this model.
        """
        self.name = name
        self.kde_good = kde_good
        self.kde_poor = kde_poor
        self.kde_density = kde_density
        self.good_threshold = good_threshold
        self.cost = cost

    def evaluate(self, self_eval, density_threshold=0.2):
        """Calculate the probability that the prediction is good based on the self evaluation score."""
        prob_good = observe_good_probability(self_eval, self.kde_good, self.kde_poor, self.kde_density, 
                                             
normalized_density_threshold=density_threshold, model_name=self.name, plot=True)
        plt.show()
        return prob_good

class POMDP:
    def __init__(self, models, lambda_param=0.1):
        """
        Initialize the POMDP with a list of models and the lambda parameter that balances performance vs. cost.
        """
        self.models = models
        self.lambda_param = lambda_param  # Parameter to balance cost vs. performance in reward function

    def compute_reward(self, prob_good, cost):
        """
        Compute the reward based on the performance (prob_good) and the cost of the model.
        """
        return prob_good - self.lambda_param * cost

    def run_simulation(self, n_examples=5):
        """Run the POMDP decision process across multiple examples."""
        for example in range(n_examples):
            print(f"Example {example + 1}. Cost factor is {self.lambda_param}:")
            for model_iter, model in enumerate(self.models):
                self_eval = np.random.uniform(0.6, 1.0)  # Generate a random self-evaluation score
                print(f"  {model.name}'s self-evaluation score: {self_eval:.2f}")
                prob_good = model.evaluate(self_eval)

                # Compute reward based on the current model's performance and cost
                if model_iter<len(self.models)-1:
                    reward = self.compute_reward(prob_good, self.models[model_iter+1].cost)
                    
print(f"  Reward for {model.name}: {reward:.2f} (probability good: {prob_good*100:.2f}%, cost of next: {self.models[mode
l_iter+1].cost})")
                else:
                    reward = 1 #no more models to escelate to

                # Decision: Should we trust this model or escalate?
                if reward > 0:  # If the reward is positive, we trust the model
                    print(f"  Decision: Stick with {model.name}. Probability of good prediction: {prob_good*100:.2f}%n")
                    break  # Stop traversing as we trust this model
                else:
                    print(f"  Escalating from {model.name}. Reward was {reward:.2f}n")
            else:
                print("  No suitable model found, escalating failed.n")

# Define models dynamically with their respective KDEs, thresholds, and costs
lm1 = Model("LM1", kde_good_lm1, kde_poor_lm1, kde_density_lm1, good_threshold=0.8, cost=1)
lm2 = Model("LM2", kde_good_lm2, kde_poor_lm2, kde_density_lm2, good_threshold=0.85, cost=1.2)
lm3 = Model("LM3", kde_good_lm3, kde_poor_lm3, kde_density_lm3, good_threshold=0.9, cost=1.5)

# Initialize the POMDP with the list of models and the lambda parameter (balancing performance vs cost)
pomdp = POMDP(models=[lm1, lm2, lm3], lambda_param=0.5)

# Run the simulation
pomdp.run_simulation(n_examples=5)`

And here’s a few examples of it in action:

[Here, we simulated sending a query to our smallest model, it answering that query, and self evaluating with a score of
0.84. Because our self-evaluation dataset has data in the region of 0.84, and good answers are more dense in that region
than wrong answers ("probability correct", being a ratio of those densities) we say that it's not worth the cost to
escalate to a larger LLM.]Here, we simulated sending a query to our smallest model, it answering that query, and self
evaluating with a score of 0.84. Because our self-evaluation dataset has data in the region of 0.84, and good answers
are more dense in that region than wrong answers ("probability correct", being a ratio of those densities) we say that
it’s not worth the cost to escalate to a larger LLM. [After passing the query to the first LLM we get a very high
self-evaluation, which is suspiciously high given that very few actually good answers acheived anywhere near a similar
score within our dataset. Being skeptical, we pass the query to the second LLM, which self evaluates at a point where
there is a lot of data, but that data is for poor answers, so the second LLM's answer is probably wrong. We then
escalate to the third model, which has a high probability of being correct in a dense region within our training data,
so we say that answer is probably good.]After passing the query to the first LLM we get a very high self-evaluation,
which is suspiciously high given that very few actually good answers acheived anywhere near a similar score within our
dataset. Being skeptical, we pass the query to the second LLM, which self evaluates at a point where there is a lot of
data, but that data is for poor answers, so the second LLM’s answer is probably wrong. We then escalate to the third
model, which has a high probability of being correct in a dense region within our training data, so we say that answer
is probably good.

And, ta-da, we have made an AutoMix style POMDP that allows us to balance cost and performance tradeoffs by routing to
different LLMs in a cascade. Let’s check out a different example of LLM Routing which approaches the problem in a few
different ways.

## A Few Approaches from RouteLLM

[The Route LLM paper][23] presents a few compelling approaches to LLM Routing:
* Similarity Weight Ranking
* Matrix Factorization
* BERT Classification
* Causal LLM Classification

We’re going to focus on Similarity Weight Ranking and BERT Classification in this article, but before we dive in I’d
like to talk about how RouteLLM differs from AutoMix in terms of the data it uses.

## RouteLLM: A Different Approach to Data

The AutoMix approach we discussed previously focuses on being able to whip up an LLM router with very little data. With
only a few tens, or perhaps hundreds, of examples of questions and answers you can use that data to build out density
estimations and develop AutoMix. This is incredibly powerful

[Content truncated]
```
