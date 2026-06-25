# Web source

- URL: https://microsoft.github.io/autogen/stable//user-guide/agentchat-user-guide/swarm.html
- Title: [Skip to main content][1]
- Captured (UTC): 2026-06-25T05:52:54.701796455+00:00

```text
[Skip to main content][1]
Back to top Ctrl+K
[ [AutoGen - Home] [AutoGen - Home] ][2]
Choose version
* [ AgentChat ][3]
* [ Core ][4]
* [ Extensions ][5]
* [ Studio ][6]
* [ API Reference ][7]
* [ .NET ][8]
* More
  * [ 0.2 Docs ][9]
Search Ctrl+K
* [ GitHub][10]
* [ Discord][11]
* [ Twitter][12]
Search Ctrl+K
* [ AgentChat ][13]
* [ Core ][14]
* [ Extensions ][15]
* [ Studio ][16]
* [ API Reference ][17]
* [ .NET ][18]
* [ 0.2 Docs ][19]
* [ GitHub][20]
* [ Discord][21]
* [ Twitter][22]
* [Installation][23]
* [Quickstart][24]
* [Migration Guide for v0.2 to v0.4][25]

Tutorial
* [Introduction][26]
* [Models][27]
* [Messages][28]
* [Agents][29]
* [Teams][30]
* [Human-in-the-Loop][31]
* [Termination][32]
* [Managing State][33]

Advanced
* [Custom Agents][34]
* [Selector Group Chat][35]
* [Swarm][36]
* [Magentic-One][37]
* [GraphFlow (Workflows)][38]
* [Memory and RAG][39]
* [Logging][40]
* [Serializing Components][41]
* [Tracing and Observability][42]

More
* [Examples][43]
  * [Travel Planning][44]
  * [Company Research][45]
  * [Literature Review][46]
* [ API Reference ][47]
* [ PyPi ][48]
* [ Source ][49]
* [AgentChat][50]
* Swarm

# Swarm[#][51]

[`Swarm`][52] implements a team in which agents can hand off task to other agents based on their capabilities. It is a
multi-agent design pattern first introduced by OpenAI in [Swarm][53]. The key idea is to let agent delegate tasks to
other agents using a special tool call, while all agents share the same message context. This enables agents to make
local decisions about task planning, rather than relying on a central orchestrator such as in [`SelectorGroupChat`][54].

Note

[`Swarm`][55] is a high-level API. If you need more control and customization that is not supported by this API, you can
take a look at the [Handoff Pattern][56] in the Core API documentation and implement your own version of the Swarm
pattern.

## How Does It Work?[#][57]

At its core, the [`Swarm`][58] team is a group chat where agents take turn to generate a response. Similar to
[`SelectorGroupChat`][59] and [`RoundRobinGroupChat`][60], participant agents broadcast their responses so all agents
share the same message context.

Different from the other two group chat teams, at each turn, **the speaker agent is selected based on the most recent
[`HandoffMessage`][61] message in the context.** This naturally requires each agent in the team to be able to generate
[`HandoffMessage`][62] to signal which other agents that it hands off to.

For [`AssistantAgent`][63], you can set the `handoffs` argument to specify which agents it can hand off to. You can use
[`Handoff`][64] to customize the message content and handoff behavior.

The overall process can be summarized as follows:
1. Each agent has the ability to generate [`HandoffMessage`][65] to signal which other agents it can hand off to. For
   [`AssistantAgent`][66], this means setting the `handoffs` argument.
2. When the team starts on a task, the first speaker agents operate on the task and make localized decision about
   whether to hand off and to whom.
3. When an agent generates a [`HandoffMessage`][67], the receiving agent takes over the task with the same message
   context.
4. The process continues until a termination condition is met.

Note

The [`AssistantAgent`][68] uses the tool calling capability of the model to generate handoffs. This means that the model
must support tool calling. If the model does parallel tool calling, multiple handoffs may be generated at the same time.
This can lead to unexpected behavior. To avoid this, you can disable parallel tool calling by configuring the model
client. For [`OpenAIChatCompletionClient`][69] and [`AzureOpenAIChatCompletionClient`][70], you can set
`parallel_tool_calls=False` in the configuration.

In this section, we will show you two examples of how to use the [`Swarm`][71] team:
1. A customer support team with human-in-the-loop handoff.
2. An automonous team for content generation.

## Customer Support Example[#][72]

[Customer Support]

This system implements a flights refund scenario with two agents:
* **Travel Agent**: Handles general travel and refund coordination.
* **Flights Refunder**: Specializes in processing flight refunds with the `refund_flight` tool.

Additionally, we let the user interact with the agents, when agents handoff to `"user"`.

### Workflow[#][73]
1. The **Travel Agent** initiates the conversation and evaluates the user’s request.
2. Based on the request:
   * For refund-related tasks, the Travel Agent hands off to the **Flights Refunder**.
   * For information needed from the customer, either agent can hand off to the `"user"`.
3. The **Flights Refunder** processes refunds using the `refund_flight` tool when appropriate.
4. If an agent hands off to the `"user"`, the team execution will stop and wait for the user to input a response.
5. When the user provides input, it’s sent back to the team as a [`HandoffMessage`][74]. This message is directed to the
   agent that originally requested user input.
6. The process continues until the Travel Agent determines the task is complete and terminates the workflow.

from typing import Any, Dict, List

from autogen_agentchat.agents import AssistantAgent
from autogen_agentchat.conditions import HandoffTermination, TextMentionTermination
from autogen_agentchat.messages import HandoffMessage
from autogen_agentchat.teams import Swarm
from autogen_agentchat.ui import Console
from autogen_ext.models.openai import OpenAIChatCompletionClient

### Tools[#][75]

def refund_flight(flight_id: str) -> str:
    """Refund a flight"""
    return f"Flight {flight_id} refunded"

### Agents[#][76]

model_client = OpenAIChatCompletionClient(
    model="gpt-4o",
    # api_key="YOUR_API_KEY",
)

travel_agent = AssistantAgent(
    "travel_agent",
    model_client=model_client,
    handoffs=["flights_refunder", "user"],
    system_message="""You are a travel agent.
    The flights_refunder is in charge of refunding flights.
    If you need information from the user, you must first send your message, then you can handoff to the user.
    Use TERMINATE when the travel planning is complete.""",
)

flights_refunder = AssistantAgent(
    "flights_refunder",
    model_client=model_client,
    handoffs=["travel_agent", "user"],
    tools=[refund_flight],
    system_message="""You are an agent specialized in refunding flights.
    You only need flight reference numbers to refund a flight.
    You have the ability to refund a flight using the refund_flight tool.
    If you need information from the user, you must first send your message, then you can handoff to the user.
    When the transaction is complete, handoff to the travel agent to finalize.""",
)

termination = HandoffTermination(target="user") | TextMentionTermination("TERMINATE")
team = Swarm([travel_agent, flights_refunder], termination_condition=termination)

task = "I need to refund my flight."


async def run_team_stream() -> None:
    task_result = await Console(team.run_stream(task=task))
    last_message = task_result.messages[-1]

    while isinstance(last_message, HandoffMessage) and last_message.target == "user":
        user_message = input("User: ")

        task_result = await Console(
            team.run_stream(task=HandoffMessage(source="user", target=last_message.source, content=user_message))
        )
        last_message = task_result.messages[-1]


# Use asyncio.run(...) if you are running this in a script.
await run_team_stream()
await model_client.close()

---------- user ----------
I need to refund my flight.
---------- travel_agent ----------
[FunctionCall(id='call_ZQ2rGjq4Z29pd0yP2sNcuyd2', arguments='{}', name='transfer_to_flights_refunder')]
[Prompt tokens: 119, Completion tokens: 14]
---------- travel_agent ----------
[FunctionExecutionResult(content='Transferred to flights_refunder, adopting the role of flights_refunder immediately.', 
call_id='call_ZQ2rGjq4Z29pd0yP2sNcuyd2')]
---------- travel_agent ----------
Transferred to flights_refunder, adopting the role of flights_refunder immediately.
---------- flights_refunder ----------
Could you please provide me with the flight reference number so I can process the refund for you?
[Prompt tokens: 191, Completion tokens: 20]
---------- flights_refunder ----------
[FunctionCall(id='call_1iRfzNpxTJhRTW2ww9aQJ8sK', arguments='{}', name='transfer_to_user')]
[Prompt tokens: 219, Completion tokens: 11]
---------- flights_refunder ----------
[FunctionExecutionResult(content='Transferred to user, adopting the role of user immediately.', call_id='call_1iRfzNpxTJ
hRTW2ww9aQJ8sK')]
---------- flights_refunder ----------
Transferred to user, adopting the role of user immediately.
---------- Summary ----------
Number of messages: 8
Finish reason: Handoff to user from flights_refunder detected.
Total prompt tokens: 529
Total completion tokens: 45
Duration: 2.05 seconds
---------- user ----------
Sure, it's 507811
---------- flights_refunder ----------
[FunctionCall(id='call_UKCsoEBdflkvpuT9Bi2xlvTd', arguments='{"flight_id":"507811"}', name='refund_flight')]
[Prompt tokens: 266, Completion tokens: 18]
---------- flights_refunder ----------
[FunctionExecutionResult(content='Flight 507811 refunded', call_id='call_UKCsoEBdflkvpuT9Bi2xlvTd')]
---------- flights_refunder ----------
Tool calls:
refund_flight({"flight_id":"507811"}) = Flight 507811 refunded
---------- flights_refunder ----------
[FunctionCall(id='call_MQ2CXR8UhVtjNc6jG3wSQp2W', arguments='{}', name='transfer_to_travel_agent')]
[Prompt tokens: 303, Completion tokens: 13]
---------- flights_refunder ----------
[FunctionExecutionResult(content='Transferred to travel_agent, adopting the role of travel_agent immediately.', call_id=
'call_MQ2CXR8UhVtjNc6jG3wSQp2W')]
---------- flights_refunder ----------
Transferred to travel_agent, adopting the role of travel_agent immediately.
---------- travel_agent ----------
Your flight with reference number 507811 has been successfully refunded. If you need anything else, feel free to let me 
know. Safe travels! TERMINATE
[Prompt tokens: 272, Completion tokens: 32]
---------- Summary ----------
Number of messages: 8
Finish reason: Text 'TERMINATE' mentioned
Total prompt tokens: 841
Total completion tokens: 63
Duration: 1.64 seconds

## Stock Research Example[#][77]

[Stock Research]

This system is designed to perform stock research tasks by leveraging four agents:
* **Planner**: The central coordinator that delegates specific tasks to specialized agents based on their expertise. The
  planner ensures that each agent is utilized efficiently and oversees the overall workflow.
* **Financial Analyst**: A specialized agent responsible for analyzing financial metrics and stock data using tools such
  as `get_stock_data`.
* **News Analyst**: An agent focused on gathering and summarizing recent news articles relevant to the stock, using
  tools such as `get_news`.
* **Writer**: An agent tasked with compiling the findings from the stock and news analysis into a cohesive final report.

### Workflow[#][78]
1. The **Planner** initiates the research process by delegating tasks to the appropriate agents in a step-by-step
   manner.
2. Each agent performs its task independently and appends their work to the shared **message thread/history**. Rather
   than directly returning results to the planner, all agents contribute to and read from this shared message history.
   When agents generate their work using the LLM, they have access to this shared message history, which provides
   context and helps track the overall progress of the task.
3. Once an agent completes its task, it hands off control back to the planner.
4. The process continues until the planner determines that all necessary tasks have been completed and decides to
   terminate the workflow.

### Tools[#][79]

async def get_stock_data(symbol: str) -> Dict[str, Any]:
    """Get stock market data for a given symbol"""
    return {"price": 180.25, "volume": 1000000, "pe_ratio": 65.4, "market_cap": "700B"}


async def get_news(query: str) -> List[Dict[str, str]]:
    """Get recent news articles about a company"""
    return [
        {
            "title": "Tesla Expands Cybertruck Production",
            "date": "2024-03-20",
            
"summary": "Tesla ramps up Cybertruck manufacturing capacity at Gigafactory Texas, aiming to meet strong demand.",
        },
        {
            "title": "Tesla FSD Beta Shows Promise",
            "date": "2024-03-19",
            
"summary": "Latest Full Self-Driving beta demonstrates significant improvements in urban navigation and safety features.
",
        },
        {
            "title": "Model Y Dominates Global EV Sales",
            "date": "2024-03-18",
            
"summary": "Tesla's Model Y becomes best-selling electric vehicle worldwide, capturing significant market share.",
        },
    ]

model_client = OpenAIChatCompletionClient(
    model="gpt-4o",
    # api_key="YOUR_API_KEY",
)

planner = AssistantAgent(
    "planner",
    model_client=model_client,
    handoffs=["financial_analyst", "news_analyst", "writer"],
    system_message="""You are a research planning coordinator.
    Coordinate market research by delegating to specialized agents:
    - Financial Analyst: For stock data analysis
    - News Analyst: For news gathering and analysis
    - Writer: For compiling final report
    Always send your plan first, then handoff to appropriate agent.
    Always handoff to a single agent at a time.
    Use TERMINATE when research is complete.""",
)

financial_analyst = AssistantAgent(
    "financial_analyst",
    model_client=model_client,
    handoffs=["planner"],
    tools=[get_stock_data],
    system_message="""You are a financial analyst.
    Analyze stock market data using the get_stock_data tool.
    Provide insights on financial metrics.
    Always handoff back to planner when analysis is complete.""",
)

news_analyst = AssistantAgent(
    "news_analyst",
    model_client=model_client,
    handoffs=["planner"],
    tools=[get_news],
    system_message="""You are a news analyst.
    Gather and analyze relevant news using the get_news tool.
    Summarize key market insights from news.
    Always handoff back to planner when analysis is complete.""",
)

writer = AssistantAgent(
    "writer",
    model_client=model_client,
    handoffs=["planner"],
    system_message="""You are a financial report writer.
    Compile research findings into clear, concise reports.
    Always handoff back to planner when writing is complete.""",
)

# Define termination condition
text_termination = TextMentionTermination("TERMINATE")
termination = text_termination

research_team = Swarm(
    participants=[planner, financial_analyst, news_analyst, writer], termination_condition=termination
)

task = "Conduct market research for TSLA stock"
await Console(research_team.run_stream(task=task))
await model_client.close()

---------- user ----------
Conduct market research for TSLA stock
---------- planner ----------
[FunctionCall(id='call_BX5QaRuhmB8CxTsBlqCUIXPb', arguments='{}', name='transfer_to_financial_analyst')]
[Prompt tokens: 169, Completion tokens: 166]
---------- planner ----------
[FunctionExecutionResult(content='Transferred to financial_analyst, adopting the role of financial_analyst immediately.'
, call_id='call_BX5QaRuhmB8CxTsBlqCUIXPb')]
---------- planner ----------
Transferred to financial_analyst, adopting the role of financial_analyst immediately.
---------- financial_analyst ----------
[FunctionCall(id='call_SAXy1ebtA9mnaZo4ztpD2xHA', arguments='{"symbol":"TSLA"}', name='get_stock_data')]
[Prompt tokens: 136, Completion tokens: 16]
---------- financial_analyst ----------
[FunctionExecutionResult(content="{'price': 180.25, 'volume': 1000000, 'pe_ratio': 65.4, 'market_cap': '700B'}", call_id
='call_SAXy1ebtA9mnaZo4ztpD2xHA')]
---------- financial_analyst ----------
Tool calls:
get_stock_data({"symbol":"TSLA"}) = {'price': 180.25, 'volume': 1000000, 'pe_ratio': 65.4, 'market_cap': '700B'}
---------- financial_analyst ----------
[FunctionCall(id='call_IsdcFUfBVmtcVzfSuwQpeAwl', arguments='{}', name='transfer_to_planner')]
[Prompt tokens: 199, Completion tokens: 337]
---------- financial_analyst ----------
[FunctionExecutionResult(content='Transferred to planner, adopting the role of planner immediately.', call_id='call_Isdc
FUfBVmtcVzfSuwQpeAwl')]
---------- financial_analyst ----------
Transferred to planner, adopting the role of planner immediately.
---------- planner ----------
[FunctionCall(id='call_tN5goNFahrdcSfKnQqT0RONN', arguments='{}', name='transfer_to_news_analyst')]
[Prompt tokens: 291, Completion tokens: 14]
---------- planner ----------
[FunctionExecutionResult(content='Transferred to news_analyst, adopting the role of news_analyst immediately.', call_id=
'call_tN5goNFahrdcSfKnQqT0RONN')]
---------- planner ----------
Transferred to news_analyst, adopting the role of news_analyst immediately.
---------- news_analyst ----------
[FunctionCall(id='call_Owjw6ZbiPdJgNWMHWxhCKgsp', arguments='{"query":"Tesla market news"}', name='get_news')]
[Prompt tokens: 235, Completion tokens: 16]
---------- news_analyst ----------
[FunctionExecutionResult(content='[{\'title\': \'Tesla Expands Cybertruck Production\', \'date\': \'2024-03-20\', \'summ
ary\': \'Tesla ramps up Cybertruck manufacturing capacity at Gigafactory Texas, aiming to meet strong demand.\'}, {\'tit
le\': \'Tesla FSD Beta Shows Promise\', \'date\': \'2024-03-19\', \'summary\': \'Latest Full Self-Driving beta demonstra
tes significant improvements in urban navigation and safety features.\'}, {\'title\': \'Model Y Dominates Global EV Sale
s\', \'date\': \'2024-03-18\', \'summary\': "Tesla\'s Model Y becomes best-selling electric vehicle worldwide, capturing
 significant market share."}]', call_id='call_Owjw6ZbiPdJgNWMHWxhCKgsp')]
---------- news_analyst ----------
Tool calls:
get_news({"query":"Tesla market news"}) = [{'title': 'Tesla Expands Cybertruck Production', 'date': '2024-03-20', 'summa
ry': 'Tesla ramps up Cybertruck manufacturing capacity at Gigafactory Texas, aiming to meet strong demand.'}, {'title': 
'Tesla FSD Beta Shows Promise', 'date': '2024-03-19', 'summary': 'Latest Full Self-Driving beta demonstrates significant
 improvements in urban navigation and safety features.'}, {'title': 'Model Y Dominates Global EV Sales', 'date': '2024-0
3-18', 'summary': "Tesla's Model Y becomes best-selling electric vehicle worldwide, capturing significant market share."
}]
---------- news_analyst ----------
Here are some of the key market insights regarding Tesla (TSLA):

1. **Expansion in Cybertruck Production**: Tesla has increased its Cybertruck production capacity at the Gigafactory in 
Texas to meet the high demand. This move might positively impact Tesla's revenues if the demand for the Cybertruck conti
nues to grow.

2. **Advancements in Full Self-Driving (FSD) Technology**: The recent beta release of Tesla's Full Self-Driving software
 shows significant advancements, particularly in urban navigation and safety. Progress in this area could enhance Tesla'
s competitive edge in the autonomous driving sector.

3. **Dominance of Model Y in EV Sales**: Tesla's Model Y has become the best-selling electric vehicle globally, capturin
g a substantial market share. Such strong sales performance reinforces Tesla's leadership in the electric vehicle market
.

These developments reflect Tesla's ongoing innovation and ability to capture market demand, which could positively influ
ence its stock performance and market position. 

I will now hand off back to the planner.
[Prompt tokens: 398, Completion tokens: 203]
---------- news_analyst ----------
[FunctionCall(id='call_pn7y6PKsBspWA17uOh3AKNMT', arguments='{}', name='transfer_to_planner')]
[Prompt tokens: 609, Completion tokens: 12]
---------- news_analyst ----------
[FunctionExecutionResult(content='Transferred to planner, adopting the role of planner immediately.', call_id='call_pn7y
6PKsBspWA17uOh3AKNMT')]
---------- news_analyst ----------
Transferred to planner, adopting the role of planner immediately.
---------- planner ----------
[FunctionCall(id='call_MmXyWuD2uJT64ZdVI5NfhYdX', arguments='{}', name='transfer_to_writer')]
[Prompt tokens: 722, Completion tokens: 11]
---------- planner ----------
[FunctionExecutionResult(content='Transferred to writer, adopting the role of writer immediately.', call_id='call_MmXyWu
D2uJT64ZdVI5NfhYdX')]
---------- planner ----------
Transferred to writer, adopting the role of writer immediately.
---------- writer ----------
[FunctionCall(id='call_Pdgu39O6GMYplBiB8jp3uyN3', arguments='{}', name='transfer_to_planner')]
[Prompt tokens: 599, Completion tokens: 323]
---------- writer ----------
[FunctionExecutionResult(content='Transferred to planner, adopting the role of planner immediately.', call_id='call_Pdgu
39O6GMYplBiB8jp3uyN3')]
---------- writer ----------
Transferred to planner, adopting the role of planner immediately.
---------- planner ----------
TERMINATE
[Prompt tokens: 772, Completion tokens: 4]
---------- Summary ----------
Number of messages: 27
Finish reason: Text 'TERMINATE' mentioned
Total prompt tokens: 4130
Total completion tokens: 1102
Duration: 17.74 seconds

TaskResult(messages=[TextMessage(source='user', models_usage=None, content='Conduct market research for TSLA stock', typ
e='TextMessage'), ToolCallRequestEvent(source='planner', models_usage=RequestUsage(prompt_tokens=169, completion_tokens=
166), content=[FunctionCall(id='call_BX5QaRuhmB8CxTsBlqCUIXPb', arguments='{}', name='transfer_to_financial_analyst')], 
type='ToolCallRequestEvent'), ToolCallExecutionEvent(source='planner', models_usage=None, content=[FunctionExecutionResu
lt(content='Transferred to financial_analyst, adopting the role of financial_analyst immediately.', call_id='call_BX5QaR
uhmB8CxTsBlqCUIXPb')], type='ToolCallExecutionEvent'), HandoffMessage(source='planner', models_usage=None, target='finan
cial_analyst', content='Transferred to financial_analyst, adopting the role of financial_analyst immediately.', type='Ha
ndoffMessage'), ToolCallRequestEvent(source='financial_analyst', models_usage=RequestUsage(prompt_tokens=136, completion
_tokens=16), content=[FunctionCall(id='call_SAXy1ebtA9mnaZo4ztpD2xHA', arguments='{"symbol":"TSLA"}', name='get_stock_da
ta')], type='ToolCallRequestEvent'), ToolCallExecutionEvent(source='financial_analyst', models_usage=None, content=[Func
tionExecutionResult(content="{'price': 180.25, 'volume': 1000000, 'pe_ratio': 65.4, 'market_cap': '700B'}", call_id='cal
l_SAXy1ebtA9mnaZo4ztpD2xHA')], type='ToolCallExecutionEvent'), TextMessage(source='financial_analyst', models_usage=None
, content='Tool calls:\nget_stock_data({"symbol":"TSLA"}) = {\'price\': 180.25, \'volume\': 1000000, \'pe_ratio\': 65.4,
 \'market_cap\': \'700B\'}', type='TextMessage'), ToolCallRequestEvent(source='financial_analyst', models_usage=RequestU
sage(prompt_tokens=199, completion_tokens=337), content=[FunctionCall(id='call_IsdcFUfBVmtcVzfSuwQpeAwl', arguments='{}'
, name='transfer_to_planner')], type='ToolCallRequestEvent'), ToolCallExecutionEvent(source='financial_analyst', models_
usage=None, content=[FunctionExecutionResult(content='Transferred to planner, adopting the role of planner immediately.'
, call_id='call_IsdcFUfBVmtcVzfSuwQpeAwl')], type='ToolCallExecutionEvent'), HandoffMessage(source='financial_analyst', 
models_usage=None, target='planner', content='Transferred to planner, adopting the role of planner immediately.', type='
HandoffMessage'), ToolCallRequestEvent(source='planner', models_usage=RequestUsage(prompt_tokens=291, completion_tokens=
14), content=[FunctionCall(id='call_tN5goNFahrdcSfKnQqT0RONN', arguments='{}', name='transfer_to_news_analyst')], type='
ToolCallRequestEvent'), ToolCallExecutionEvent(source='planner', models_usage=None, content=[FunctionExecutionResult(con
tent='Transferred to news_analyst, adopting the role of news_analyst immediately.', call_id='call_tN5goNFahrdcSfKnQqT0RO
NN')], type='ToolCallExecutionEvent'), HandoffMessage(source='planner', models_usage=None, target='news_analyst', conten
t='Transferred to news_analyst, adopting the role of news_analyst immediately.', type='HandoffMessage'), ToolCallRequest
Event(source='news_analyst', models_usage=RequestUsage(prompt_tokens=235, completion_tokens=16), content=[FunctionCall(i
d='call_Owjw6ZbiPdJgNWMHWxhCKgsp', arguments='{"query":"Tesla market news"}', name='get_news')], type='ToolCallRequestEv
ent'), ToolCallExecutionEvent(source='news_analyst', models_usage=None, content=[FunctionExecutionResult(content='[{\'ti
tle\': \'Tesla Expands Cybertruck Production\', \'date\': \'2024-03-20\', \'summary\': \'Tesla ramps up Cybertruck manuf
acturing capacity at Gigafactory Texas, aiming to meet strong demand.\'}, {\'title\': \'Tesla FSD Beta Shows Promise\', 
\'date\': \'2024-03-19\', \'summary\': \'Latest Full Self-Driving beta demonstrates significant improvements in urban na
vigation and safety features.\'}, {\'title\': \'Model Y Dominates Global EV Sales\', \'date\': \'2024-03-18\', \'summary
\': "Tesla\'s Model Y becomes best-selling electric vehicle worldwide, capturing significant market share."}]', call_id=
'call_Owjw6ZbiPdJgNWMHWxhCKgsp')], type='ToolCallExecutionEvent'), TextMessage(source='news_analyst', models_usage=None,
 content='Tool calls:\nget_news({"query":"Tesla market news"}) = [{\'title\': \'Tesla Expands Cybertruck Production\', \
'date\': \'2024-03-20\', \'summary\': \'Tesla ramps up Cybertruck manufacturing capacity at Gigafactory Texas, aiming to
 meet strong demand.\'}, {\'title\': \'Tesla FSD Beta Shows Promise\', \'date\': \'2024-03-19\', \'summary\': \'Latest F
ull Self-Driving beta demonstrates significant improvements in urban navigation and safety features.\'}, {\'title\': \'M
odel Y Dominates Global EV Sales\', \'date\': \'2024-03-18\', \'summary\': "Tesla\'s Model Y becomes best-selling electr
ic vehicle worldwide, capturing significant market share."}]', type='TextMessage'), TextMessage(source='news_analyst', m
odels_usage=RequestUsage(prompt_tokens=398, completion_tokens=203), content="Here are some of the key market insights re
garding Tesla (TSLA):\n\n1. **Expansion in Cybertruck Production**: Tesla has increased its Cybertruck production capaci
ty at the Gigafactory in Texas to meet the high demand. This move might positively impact Tesla's revenues if the demand
 for the Cybertruck continues to grow.\n\n2. **Advancements in Full Self-Driving (FSD) Technology**: The recent beta rel
ease of Tesla's Full Self-Driving software shows significant advancements, particularly in urban navigation and safety. 
Progress in this area could enhance Tesla's competitive edge in the autonomous driving sector.\n\n3. **Dominance of Mode
l Y in EV Sales**: Tesla's Model Y has become the best-selling electric vehicle globally, capturing a substantial market
 share. Such strong sales performance reinforces Tesla's leadership in the electric vehicle market.\n\nThese development
s reflect Tesla's ongoing innovation and ability to capture market demand, which could positively influence its stock pe
rformance and market position. \n\nI will now hand off back to the planner.", type='TextMessage'), ToolCallRequestEvent(
source='news_analyst', models_usage=RequestUsage(prompt_tokens=609, completion_tokens=12), content=[FunctionCall(id='cal
l_pn7y6PKsBspWA17uOh3AKNMT', arguments='{}', name='transfer_to_planner')], type='ToolCallRequestEvent'), ToolCallExecuti
onEvent(source='news_analyst', models_usage=None, content=[FunctionExecutionResult(content='Transferred to planner, adop
ting the role of planner immediately.', call_id='call_pn7y6PKsBspWA17uOh3AKNMT')], type='ToolCallExecutionEvent'), Hando
ffMessage(source='news_analyst', models_usage=None, target='planner', content='Transferred to planner, adopting the role
 of planner immediately.', type='HandoffMessage'), ToolCallRequestEvent(source='planner', models_usage=RequestUsage(prom
pt_tokens=722, completion_tokens=11), content=[FunctionCall(id='call_MmXyWuD2uJT64ZdVI5NfhYdX', arguments='{}', name='tr
ansfer_to_writer')], type='ToolCallRequestEvent'), ToolCallExecutionEvent(source='planner', models_usage=None, content=[
FunctionExecutionResult(content='Transferred to writer, adopting the role of writer immediately.', call_id='call_MmXyWuD
2uJT64ZdVI5NfhYdX')], type='ToolCallExecutionEvent'), HandoffMessage(source='planner', models_usage=None, target='writer
', content='Transferred to writer, adopting the role of writer immediately.', type='HandoffMessage'), ToolCallRequestEve
nt(source='writer', models_usage=RequestUsage(prompt_tokens=599, completion_tokens=323), content=[FunctionCall(id='call_
Pdgu39O6GMYplBiB8jp3uyN3', arguments='{}', name='transfer_to_planner')], type='ToolCallRequestEvent'), ToolCallExecution
Event(source='writer', models_usage=None, content=[FunctionExecutionResult(content='Transferred to planner, adopting the
 role of planner immediately.', call_id='call_Pdgu39O6GMYplBiB8jp3uyN3')], type='ToolCallExecutionEvent'), HandoffMessag
e(source='writer', models_usage=None, target='planner', content='Transferred to planner, adopting the role of planner im
mediately.', type='HandoffMessage'), TextMessage(source='planner', models_usage=RequestUsage(prompt_tokens=772, completi
on_tokens=4), content='TERMINATE', type='TextMessage')], stop_reason="Text 'TERMINATE' mentioned")

[

previous

Selector Group Chat

][80] [

next

Magentic-One

][81]
On this page
* [How Does It Work?][82]
* [Customer Support Example][83]
  * [Workflow][84]
  * [Tools][85]
  * [Agents][86]
* [Stock Research Example][87]
  * [Workflow][88]
  * [Tools][89]
[ Edit on GitHub ][90]
[ Show Source ][91]

© Copyright 2024, Microsoft.

[Privacy Policy][92] | [Consumer Health Privacy][93]

Built with the [PyData Sphinx Theme][94] 0.16.0.

[1]: #main-content
[2]: ../../index.html
[3]: index.html
[4]: ../core-user-guide/index.html
[5]: ../extensions-user-guide/index.html
[6]: ../autogenstudio-user-guide/index.html
[7]: ../../reference/index.html
[8]: https://microsoft.github.io/autogen/dotnet/
[9]: https://microsoft.github.io/autogen/0.2/
[10]: https://github.com/microsoft/autogen
[11]: https://aka.ms/autogen-discord
[12]: https://twitter.com/pyautogen
[13]: index.html
[14]: ../core-user-guide/index.html
[15]: ../extensions-user-guide/index.html
[16]: ../autogenstudio-user-guide/index.html
[17]: ../../reference/index.html
[18]: https://microsoft.github.io/autogen/dotnet/
[19]: https://microsoft.github.io/autogen/0.2/
[20]: https://github.com/microsoft/autogen
[21]: https://aka.ms/autogen-discord
[22]: https://twitter.com/pyautogen
[23]: installation.html
[24]: quickstart.html
[25]: migration-guide.html
[26]: tutorial/index.html
[27]: tutorial/models.html
[28]: tutorial/messages.html
[29]: tutorial/agents.html
[30]: tutorial/teams.html
[31]: tutorial/human-in-the-loop.html
[32]: tutorial/termination.html
[33]: tutorial/state.html
[34]: custom-agents.html
[35]: selector-group-chat.html
[36]: #
[37]: magentic-one.html
[38]: graph-flow.html
[39]: memory.html
[40]: logging.html
[41]: serialize-components.html
[42]: tracing.html
[43]: examples/index.html
[44]: examples/travel-planning.html
[45]: examples/company-research.html
[46]: examples/literature-review.html
[47]: ../../reference/python/autogen_agentchat.html
[48]: https://pypi.org/project/autogen-agentchat/
[49]: https://github.com/microsoft/autogen/tree/main/python/packages/autogen-agentchat
[50]: index.html
[51]: #swarm
[52]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.Swarm
[53]: https://github.com/openai/swarm
[54]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.SelectorGroupChat
[55]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.Swarm
[56]: ../core-user-guide/design-patterns/handoffs.html
[57]: #how-does-it-work
[58]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.Swarm
[59]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.SelectorGroupChat
[60]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.RoundRobinGroupChat
[61]: ../../reference/python/autogen_agentchat.messages.html#autogen_agentchat.messages.HandoffMessage
[62]: ../../reference/python/autogen_agentchat.messages.html#autogen_agentchat.messages.HandoffMessage
[63]: ../../reference/python/autogen_agentchat.agents.html#autogen_agentchat.agents.AssistantAgent
[64]: ../../reference/python/autogen_agentchat.base.html#autogen_agentchat.base.Handoff
[65]: ../../reference/python/autogen_agentchat.messages.html#autogen_agentchat.messages.HandoffMessage
[66]: ../../reference/python/autogen_agentchat.agents.html#autogen_agentchat.agents.AssistantAgent
[67]: ../../reference/python/autogen_agentchat.messages.html#autogen_agentchat.messages.HandoffMessage
[68]: ../../reference/python/autogen_agentchat.agents.html#autogen_agentchat.agents.AssistantAgent
[69]: ../../reference/python/autogen_ext.models.openai.html#autogen_ext.models.openai.OpenAIChatCompletionClient
[70]: ../../reference/python/autogen_ext.models.openai.html#autogen_ext.models.openai.AzureOpenAIChatCompletionClient
[71]: ../../reference/python/autogen_agentchat.teams.html#autogen_agentchat.teams.Swarm
[72]: #customer-support-example
[73]: #workflow
[74]: ../../reference/python/autogen_agentchat.messages.html#autogen_agentchat.messages.HandoffMessage
[75]: #tools
[76]: #agents
[77]: #stock-research-example
[78]: #id1
[79]: #id2
[80]: selector-group-chat.html
[81]: magentic-one.html
[82]: #how-does-it-work
[83]: #customer-support-example
[84]: #workflow
[85]: #tools
[86]: #agents
[87]: #stock-research-example
[88]: #id1
[89]: #id2
[90]: https://github.com/microsoft/autogen/edit/main/python/docs/src/user-guide/agentchat-user-guide/swarm.ipynb
[91]: ../../_sources/user-guide/agentchat-user-guide/swarm.ipynb.txt
[92]: https://go.microsoft.com/fwlink/?LinkId=521839
[93]: https://go.microsoft.com/fwlink/?linkid=2259814
[94]: https://pydata-sphinx-theme.readthedocs.io/en/stable/index.html
```
