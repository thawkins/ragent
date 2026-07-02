# Web source

- URL: https://noveum.ai/en/docs/integration-examples/langgraph/iterative-research
- Title: [[Noveum.ai Logo]][1]
- Captured (UTC): 2026-06-30T09:39:07.079353895+00:00

```text
[[Noveum.ai Logo]][1]
Solutions
[AI Agent Monitoring][2][LLM Observability][3][Agent Evaluation][4][Debugging & Tracing][5][Platform
Comparison][6][Noveum vs Arize & Braintrust][7][Noveum vs Langfuse][8][Noveum vs Arize Phoenix][9][Case Study:
MyOperator][10]
[Enterprise][11][Scorers][12][Blog][13][Docs][14][SDK][15][Careers][16][Contact][17]
[Login / Sign Up][18]
[**Documentation**][19]
[**Documentation**][20]
Search
⌘K
[🚀 Noveum.ai Overview][21]
Getting Started
[Quick Setup - 5 Minute Start][22][SDK Integration Guide][23][The Evaluation Pipeline][24]
Integration Examples
[Simple LLM Integration][25]
LangChain Integration
LangGraph Integration
[LangGraph Integration Overview][26][Basic LangGraph Agent][27][Iterative Research Agent][28]
LiveKit
Pipecat
CrewAI Integration
Core Concepts
[Traces - Request Journeys][29][Spans - Individual Operations][30][Attributes - Metadata and Context][31][Events -
Point-in-Time Occurrences][32][Context Managers - Flexible Tracing][33]
Datasets
[What is a Dataset][34][StandardData Schema][35]
ETL Jobs
[ETL Jobs][36][Using the AI Mapper][37]
NovaEval
[NovaEval - AI Evaluation Engine][38][Scorers Reference][39][Running Evaluations][40]
NovaPilot
[NovaPilot][41]
NovaSynth
[NovaSynth Overview][42][NovaSynth Setup][43][Personas, Scenarios & Runs][44]
Best Practices
[Observability Best Practices][45][Traces Best Practices][46][Spans Best Practices][47][Attributes Best
Practices][48][Events Best Practices][49]
Products
[Noveum Trace - Lightweight Tracing SDK][50][NovaEval - AI Evaluation Engine][51][NovaPilot - Intelligent Analysis
Orchestrator][52][Noveum SDK - Python Client][53]
Platform
[Platform Overview][54][Dashboard Overview][55][Python SDK][56][MCP server: resources, prompts & agents][57]
On this page
Integration Examples/LangGraph Integration/[Iterative Research Agent][58]

# Iterative Research Agent

Learn how to trace iterative research agents with self-loops using Noveum Trace

This guide shows you how to trace iterative research agents that can loop back to refine their work. You'll learn how to
monitor self-loops, state evolution, and iterative refinement processes.

## [🎯 Use Case][59]

**Research Assistant Agent**: An agent that conducts research on a topic, evaluates the quality of information gathered,
and can loop back to gather more information if needed. We'll trace the complete iterative process.

## [🚀 Complete Working Example][60]

Here's a complete, working example based on `langgraph_agent_example.py`:

`import os
from typing import Annotated, Literal, TypedDict
from dotenv import load_dotenv
import noveum_trace
from noveum_trace import NoveumTraceCallbackHandler
from langchain_core.messages import AIMessage, HumanMessage
from langchain_core.tools import tool
from langchain_openai import ChatOpenAI
from langgraph.graph import END, StateGraph

load_dotenv()

# Initialize Noveum Trace
noveum_trace.init(
    api_key=os.getenv("NOVEUM_API_KEY"),
    project="customer-support-bot",
    environment="development"
)

# Define the research state
class ResearchState(TypedDict):
    messages: Annotated[list, "The messages in the conversation"]
    research_topic: str
    research_notes: Annotated[list, "Research notes gathered"]
    evaluation_score: float
    max_iterations: int
    current_iteration: int
    research_complete: bool

# Define research tools
@tool
def search_web(query: str) -> str:
    """Search the web for information about a query."""
    # Simulate web search with realistic results
    search_results = {
        "artificial intelligence": "AI is a branch of computer science focused on creating intelligent machines...",
        
"machine learning": "Machine learning is a subset of AI that enables computers to learn without explicit programming..."
,
        "deep learning": "Deep learning uses neural networks with multiple layers to process data...",
        
"natural language processing": "NLP is a field of AI that focuses on the interaction between computers and human languag
e..."
    }

    # Return relevant results based on query
    for key, value in search_results.items():
        if key in query.lower():
            return f"Search results for '{query}': {value}"

    return f"Search results for '{query}': General information about the topic."

@tool
def analyze_information(info: str) -> str:
    """Analyze and summarize information."""
    return f"Analysis: {info} contains valuable insights and detailed information about the topic."

def research_node(state: ResearchState):
    """Node that performs research using tools."""
    print(f"🔍 Research iteration {state['current_iteration']}: {state['research_topic']}")

    # Search for information
    search_query = f"research about {state['research_topic']}"
    search_results = search_web(search_query)

    # Analyze the results
    analysis = analyze_information(search_results)

    # Add to research notes
    state["research_notes"].append({
        "iteration": state["current_iteration"],
        "query": search_query,
        "results": search_results,
        "analysis": analysis
    })

    # Add research message
    
state["messages"].append(AIMessage(content=f"Research iteration {state['current_iteration']} completed: {analysis}"))

    return state

def evaluate_node(state: ResearchState):
    """Node that evaluates the quality of research gathered."""
    print(f"📊 Evaluating research quality...")

    # Simple evaluation based on research notes
    total_notes = len(state["research_notes"])
    quality_score = min(0.9, 0.3 + (total_notes * 0.1))

    state["evaluation_score"] = quality_score

    # Add evaluation message
    
evaluation_msg = f"Research evaluation: {quality_score:.2f} quality score based on {total_notes} research iterations"
    state["messages"].append(AIMessage(content=evaluation_msg))

    print(f"📈 Quality score: {quality_score:.2f}")

    return state

def should_continue(state: ResearchState) -> Literal["research", "synthesize", "end"]:
    """Decide whether to continue researching, synthesize, or end."""
    print(f"🤔 Deciding next action...")

    # Check if we've reached max iterations
    if state["current_iteration"] >= state["max_iterations"]:
        print("⏰ Max iterations reached, synthesizing...")
        return "synthesize"

    # Check if quality is sufficient
    if state["evaluation_score"] >= 0.8:
        print("✅ Quality sufficient, synthesizing...")
        return "synthesize"

    # Continue researching
    print("🔄 Quality insufficient, continuing research...")
    state["current_iteration"] += 1
    return "research"

def synthesize_node(state: ResearchState):
    """Node that synthesizes all research into a final report."""
    print("📝 Synthesizing final research report...")

    # Create comprehensive report
    report = f"""
    # Research Report: {state['research_topic']}

    ## Summary
    Based on {state['current_iteration']} research iterations, here's what I found:

    """

    # Add findings from each iteration
    for i, note in enumerate(state["research_notes"], 1):
        report += f"### Iteration {i}\n{note['analysis']}\n\n"

    report += f"""
    ## Final Evaluation
    Quality Score: {state['evaluation_score']:.2f}
    Total Iterations: {state['current_iteration']}

    ## Conclusion
    This research provides comprehensive coverage of {state['research_topic']} with detailed analysis and insights.
    """

    # Add final message
    state["messages"].append(AIMessage(content=report))
    state["research_complete"] = True

    print("✅ Research synthesis completed!")

    return state

def create_iterative_research_agent():
    """Create an iterative research agent with tracing."""
    # Create LLM (without callbacks - will be passed at graph level)
    llm = ChatOpenAI(
        model="gpt-4",
        temperature=0.7
    )

    # Create the graph
    graph = StateGraph(ResearchState)

    # Add nodes
    graph.add_node("research", research_node)
    graph.add_node("evaluate", evaluate_node)
    graph.add_node("synthesize", synthesize_node)

    # Add edges
    graph.add_edge("research", "evaluate")
    graph.add_conditional_edges(
        "evaluate",
        should_continue,
        {
            "research": "research",
            "synthesize": "synthesize",
            "end": END
        }
    )
    graph.add_edge("synthesize", END)

    # Set entry point
    graph.set_entry_point("research")

    return graph.compile()

def run_iterative_research():
    """Run the iterative research agent with tracing."""
    print("=== Iterative Research Agent Tracing ===")

    # Create callback handler
    callback_handler = NoveumTraceCallbackHandler()

    # Create the agent
    agent = create_iterative_research_agent()

    # Run with callbacks via config (recommended approach)
    result = agent.invoke(
        {
            "messages": [HumanMessage(content="Research artificial intelligence and its applications")],
            "research_topic": "artificial intelligence and its applications",
            "research_notes": [],
            "evaluation_score": 0.0,
            "max_iterations": 3,
            "current_iteration": 1,
            "research_complete": False
        },
        config={
            "callbacks": [callback_handler],
            "tags": ["iterative_research"]
        }
    )

    print(f"\n🎉 Research completed!")
    print(f"📊 Final quality score: {result['evaluation_score']:.2f}")
    print(f"🔄 Total iterations: {result['current_iteration']}")
    print(f"📝 Research notes: {len(result['research_notes'])}")

    return result

if __name__ == "__main__":
    run_iterative_research()`

## [📋 Prerequisites][61]

`pip install noveum-trace langchain-openai langgraph python-dotenv`

Set your environment variables:

`export NOVEUM_API_KEY="your-noveum-api-key"
export OPENAI_API_KEY="your-openai-api-key"`

## [🔧 How It Works][62]

### [1. **Iterative Process**][63]

The agent follows this flow:
1. **Research**: Gather information using tools
2. **Evaluate**: Assess the quality of information
3. **Decide**: Continue research or synthesize results
4. **Synthesize**: Create final report (if quality sufficient)

### [2. **State Management**][64]

The `ResearchState` tracks:
* Research topic and notes
* Current iteration count
* Quality evaluation score
* Completion status

### [3. **Self-Loop Tracing**][65]

Each iteration is traced as a separate span:
* Research node execution
* Tool calls and results
* Evaluation process
* Decision-making logic

## [🎨 Advanced Examples][66]

### [Adaptive Research Agent][67]

`def create_adaptive_research_agent():
    """Create an agent that adapts its research strategy."""
    llm = ChatOpenAI()

    def adaptive_research_node(state: ResearchState):
        """Adapt research strategy based on previous results."""
        # Analyze previous research to determine next steps
        if state["current_iteration"] > 1:
            # Look for gaps in previous research
            previous_queries = [note["query"] for note in state["research_notes"]]
            # Adapt search strategy based on gaps
            pass

        # Continue with research
        return research_node(state)

    # Rest of the implementation...`

### [Multi-Source Research][68]

`@tool
def search_academic(query: str) -> str:
    """Search academic databases."""
    return f"Academic search results for: {query}"

@tool
def search_news(query: str) -> str:
    """Search news sources."""
    return f"News search results for: {query}"

def multi_source_research_node(state: ResearchState):
    """Research using multiple sources."""
    # Search different sources
    academic_results = search_academic(state["research_topic"])
    news_results = search_news(state["research_topic"])
    web_results = search_web(state["research_topic"])

    # Combine results
    combined_analysis = f"""
    Academic: {academic_results}
    News: {news_results}
    Web: {web_results}
    """

    # Add to research notes
    state["research_notes"].append({
        "iteration": state["current_iteration"],
        "sources": ["academic", "news", "web"],
        "results": combined_analysis
    })

    return state`

## [📊 What You'll See in the Dashboard][69]

After running this example, check your Noveum dashboard:

### [**Trace View**][70]
* Complete iterative workflow
* Each research iteration as a separate span
* Tool calls and results
* Evaluation and decision-making process

### [**Span Details**][71]
* Individual iteration performance
* Tool execution times
* Quality score evolution
* State changes over time

### [**Analytics**][72]
* Iteration patterns and efficiency
* Quality improvement over time
* Tool usage statistics
* Research effectiveness metrics

## [🔍 Troubleshooting][73]

### [**Common Issues**][74]

**Infinite loops?**
* Set appropriate `max_iterations` limit
* Ensure evaluation criteria are realistic
* Monitor quality score thresholds

**Poor research quality?**
* Adjust evaluation criteria
* Improve tool implementations
* Add more diverse research sources

**Performance issues?**
* Monitor iteration execution times
* Optimize tool calls
* Consider parallel research strategies

## [🚀 Next Steps][75]

Now that you've mastered iterative research agents, explore these patterns:
* **[Basic Agent][76]** - Simple agent workflows

## [💡 Pro Tips][77]
1. **Set iteration limits**: Prevent infinite loops with max iteration counts
2. **Monitor quality scores**: Track research quality over iterations
3. **Use diverse sources**: Combine multiple research tools
4. **Adapt strategies**: Modify research approach based on results
5. **Track state evolution**: Monitor how state changes through iterations

Exclusive Early Access

## Get Early Access to Noveum.ai Platform

Be the first one to get notified when we open Noveum Platform to more users. All users get access to Observability suite
for free, early users get free eval jobs and premium support for the first year.

Get Started Now

Sign up now. We send access to new batch every week.

Early access members receive premium onboarding support and influence our product roadmap. Limited spots available.

[

Previous

Basic LangGraph Agent

][78][

Next

LiveKit Integration Overview

][79]

### On this page

[🎯 Use Case][80][🚀 Complete Working Example][81][📋 Prerequisites][82][🔧 How It Works][83][1. Iterative
Process][84][2. State Management][85][3. Self-Loop Tracing][86][🎨 Advanced Examples][87][Adaptive Research
Agent][88][Multi-Source Research][89][📊 What You'll See in the Dashboard][90][Trace View][91][Span
Details][92][Analytics][93][🔍 Troubleshooting][94][Common Issues][95][🚀 Next Steps][96][💡 Pro Tips][97]
[Noveum.ai Logo]

© 2026 Noveum.ai. All rights reserved.

[Blog][98][Features][99][Pricing][100]
[Privacy policy][101][Terms and conditions][102]
Iterative Research Agent | Documentation | Noveum.ai

[1]: /en
[2]: /en/solutions/ai-agent-monitoring
[3]: /en/solutions/llm-observability
[4]: /en/solutions/agent-evaluation
[5]: /en/solutions/debugging
[6]: /en/comparison
[7]: /en/comparison/noveum-vs-arize-braintrust
[8]: /en/comparison/noveum-vs-langfuse
[9]: /en/comparison/noveum-vs-arize
[10]: /en/case-studies/myoperator
[11]: /en/enterprise
[12]: /en/solutions/scorers
[13]: /en/blog
[14]: /en/docs
[15]: https://github.com/Noveum/noveum-trace
[16]: /en/careers
[17]: /en/contact
[18]: /auth/login
[19]: /docs
[20]: /docs
[21]: /en/docs
[22]: /en/docs/getting-started/quick-setup
[23]: /en/docs/getting-started/sdk-integration
[24]: /en/docs/getting-started/evaluation-pipeline
[25]: /en/docs/integration-examples/simple-llm
[26]: /en/docs/integration-examples/langgraph/overview
[27]: /en/docs/integration-examples/langgraph/basic-agent
[28]: /en/docs/integration-examples/langgraph/iterative-research
[29]: /en/docs/concepts/traces
[30]: /en/docs/concepts/spans
[31]: /en/docs/concepts/attributes
[32]: /en/docs/concepts/events
[33]: /en/docs/concepts/context-managers
[34]: /en/docs/datasets/overview
[35]: /en/docs/datasets/standard-data-schema
[36]: /en/docs/etl-jobs/overview
[37]: /en/docs/etl-jobs/ai-mapper
[38]: /en/docs/nova-eval/overview
[39]: /en/docs/nova-eval/scorers
[40]: /en/docs/nova-eval/running-evals
[41]: /en/docs/novapilot/overview
[42]: /en/docs/novasynth/overview
[43]: /en/docs/novasynth/setup
[44]: /en/docs/novasynth/personas-scenarios
[45]: /en/docs/best-practices/tracing-concepts-best-practices
[46]: /en/docs/best-practices/traces-best-practices
[47]: /en/docs/best-practices/spans-best-practices
[48]: /en/docs/best-practices/attributes-best-practices
[49]: /en/docs/best-practices/events-best-practices
[50]: /en/docs/noveum-products/noveum-trace
[51]: /en/docs/noveum-products/nova-eval
[52]: /en/docs/noveum-products/novapilot
[53]: /en/docs/noveum-products/noveum-sdk
[54]: /en/docs/platform/overview
[55]: /en/docs/platform/dashboard
[56]: /en/docs/platform/python-sdk
[57]: /en/docs/platform/mcp-server-reference
[58]: /en/docs/integration-examples/langgraph/iterative-research
[59]: #-use-case
[60]: #-complete-working-example
[61]: #-prerequisites
[62]: #-how-it-works
[63]: #1-iterative-process
[64]: #2-state-management
[65]: #3-self-loop-tracing
[66]: #-advanced-examples
[67]: #adaptive-research-agent
[68]: #multi-source-research
[69]: #-what-youll-see-in-the-dashboard
[70]: #trace-view
[71]: #span-details
[72]: #analytics
[73]: #-troubleshooting
[74]: #common-issues
[75]: #-next-steps
[76]: /docs/integration-examples/langgraph/basic-agent
[77]: #-pro-tips
[78]: /en/docs/integration-examples/langgraph/basic-agent
[79]: /en/docs/integration-examples/livekit/overview
[80]: #-use-case
[81]: #-complete-working-example
[82]: #-prerequisites
[83]: #-how-it-works
[84]: #1-iterative-process
[85]: #2-state-management
[86]: #3-self-loop-tracing
[87]: #-advanced-examples
[88]: #adaptive-research-agent
[89]: #multi-source-research
[90]: #-what-youll-see-in-the-dashboard
[91]: #trace-view
[92]: #span-details
[93]: #analytics
[94]: #-troubleshooting
[95]: #common-issues
[96]: #-next-steps
[97]: #-pro-tips
[98]: /en/blog
[99]: #features
[100]: /#pricing
[101]: /en/legal/privacy-policy
[102]: /en/legal/terms
```
