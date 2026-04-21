//! ragent-prompt_opt — LLM-powered prompt optimization helpers.
//!
//! Provides a [`Completer`] trait, an [`OptMethod`] enum covering 12 prompt
//! optimization frameworks (CO-STAR, CRISPE, CoT, DRAW, RISE, O1-STYLE, Meta
//! Prompting, VARI, Q*, OpenAI, Claude, Microsoft), and an async [`optimize`]
//! function that sends each method's meta-prompt to an LLM and returns the
//! generated structured prompt.
//!
//! Callers supply their own [`Completer`] implementation so this crate stays
//! decoupled from any specific LLM backend.

use async_trait::async_trait;

/// Thin async abstraction over a single LLM completion call.
///
/// Implementors send `system` as the system prompt and `user` as the user
/// message, collect the full response, and return it as a `String`.
#[async_trait]
pub trait Completer: Send + Sync {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String>;
}

/// The 12 supported prompt optimization frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptMethod {
    /// CO-STAR: Context, Objective, Style/Identity, Tone, Audience, Result.
    CoStar,
    /// CRISPE: Capacity/Role, Request, Intent, Steps, Persona, Examples.
    Crispe,
    /// Chain-of-Thought: step-by-step reasoning scaffold.
    ChainOfThought,
    /// DRAW: professional image/drawing prompt optimizer.
    Draw,
    /// RISE: Recursive Introspection for iterative self-improvement.
    Rise,
    /// O1-STYLE: structured thinking/reflection/reward tag scaffold.
    O1Style,
    /// Meta Prompting: distil the task into a concise, high-signal prompt.
    MetaPrompting,
    /// VARI (Variational): variational planning content-generation scaffold.
    Variational,
    /// Q*: XML-structured Q*/A* intelligent prompt optimization.
    QStar,
    /// OpenAI adapter: detailed GPT-style system prompt.
    OpenAI,
    /// Claude adapter: Anthropic-style XML instruction generator.
    Claude,
    /// Microsoft adapter: Azure AI optimized prompt.
    Microsoft,
}

impl std::str::FromStr for OptMethod {
    type Err = ();

    /// Parse a method name/alias (case-insensitive) into an [`OptMethod`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "co_star" | "costar" | "co-star" => Ok(Self::CoStar),
            "crispe" | "crisper" => Ok(Self::Crispe),
            "cot" | "chain_of_thought" | "chain-of-thought" | "chainofthought" => {
                Ok(Self::ChainOfThought)
            }
            "draw" => Ok(Self::Draw),
            "rise" => Ok(Self::Rise),
            "o1_style" | "o1-style" | "o1" => Ok(Self::O1Style),
            "meta" | "meta_prompting" | "meta-prompting" => Ok(Self::MetaPrompting),
            "variational" | "vari" => Ok(Self::Variational),
            "q_star" | "qstar" | "q-star" | "q*" => Ok(Self::QStar),
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "microsoft" | "ms" | "azure" => Ok(Self::Microsoft),
            _ => Err(()),
        }
    }
}

impl OptMethod {
    /// Canonical short name for this method (used in status messages, logs).
    pub fn name(self) -> &'static str {
        match self {
            Self::CoStar => "co_star",
            Self::Crispe => "crispe",
            Self::ChainOfThought => "cot",
            Self::Draw => "draw",
            Self::Rise => "rise",
            Self::O1Style => "o1_style",
            Self::MetaPrompting => "meta",
            Self::Variational => "variational",
            Self::QStar => "q_star",
            Self::OpenAI => "openai",
            Self::Claude => "claude",
            Self::Microsoft => "microsoft",
        }
    }

    /// Human-readable description for the help table.
    pub fn description(self) -> &'static str {
        match self {
            Self::CoStar => "Context, Objective, Identity, Tone, Audience, Result",
            Self::Crispe => "Capacity/Role, Request, Intent, Steps, Persona, Examples",
            Self::ChainOfThought => "Step-by-step reasoning scaffold with self-checks",
            Self::Draw => "Professional AI image prompt optimizer (English output)",
            Self::Rise => "Recursive Introspection — iterative self-improvement loop",
            Self::O1Style => "Thinking/step/reflection/reward structured reasoning",
            Self::MetaPrompting => "Distil to a concise, high-signal meta-prompt",
            Self::Variational => "Variational planning content-generation scaffold",
            Self::QStar => "XML Q*/A* intelligent iterative prompt optimizer",
            Self::OpenAI => "Detailed GPT-style system prompt with guidelines",
            Self::Claude => "Anthropic-style XML instruction generator with examples",
            Self::Microsoft => "Azure AI optimized prompt with quality targets",
        }
    }

    /// All methods in display order.
    pub fn all() -> &'static [OptMethod] {
        &[
            Self::CoStar,
            Self::Crispe,
            Self::ChainOfThought,
            Self::Draw,
            Self::Rise,
            Self::O1Style,
            Self::MetaPrompting,
            Self::Variational,
            Self::QStar,
            Self::OpenAI,
            Self::Claude,
            Self::Microsoft,
        ]
    }

    /// Returns a markdown help table listing all methods and their descriptions.
    pub fn help_table() -> String {
        let mut out = String::from("| Method | Description |\n|--------|-------------|\n");
        for m in Self::all() {
            out.push_str(&format!("| `{}` | {} |\n", m.name(), m.description()));
        }
        out
    }
}

/// The meta-prompt (system message) that instructs an LLM to optimize a task
/// using the given framework. The user message will be the raw task/prompt.
pub fn system_prompt(method: OptMethod) -> &'static str {
    match method {
        OptMethod::CoStar => CO_STAR_SYSTEM,
        OptMethod::Crispe => CRISPE_SYSTEM,
        OptMethod::ChainOfThought => COT_SYSTEM,
        OptMethod::Draw => DRAW_SYSTEM,
        OptMethod::Rise => RISE_SYSTEM,
        OptMethod::O1Style => O1_SYSTEM,
        OptMethod::MetaPrompting => META_SYSTEM,
        OptMethod::Variational => VARI_SYSTEM,
        OptMethod::QStar => QSTAR_SYSTEM,
        OptMethod::OpenAI => OPENAI_SYSTEM,
        OptMethod::Claude => CLAUDE_SYSTEM,
        OptMethod::Microsoft => MICROSOFT_SYSTEM,
    }
}

/// Send the method's meta-prompt plus the user's input to the LLM and return
/// the generated optimized prompt.
pub async fn optimize(
    method: OptMethod,
    input: &str,
    completer: &dyn Completer,
) -> anyhow::Result<String> {
    let system = system_prompt(method);
    completer.complete(system, input.trim()).await
}

// ── Meta-prompts ─────────────────────────────────────────────────────────────
// Each constant is the system message sent to the LLM. The user message is the
// raw task. Adapted from the 302.ai open-source prompt generator.

const CO_STAR_SYSTEM: &str = r#"You are a senior prompt engineering expert. Your task is to analyze the user's task and decompose it into a structured CO-STAR prompt. Think carefully about the task, then produce a single code block containing the optimized prompt in this exact structure:

==Context==
Describe the task in detail, broken into specific sub-tasks. Explain what background information the model needs.

==Objective==
State the final goal clearly. If not specified by the user, infer the most logical objective.

==Identity==
Choose the most appropriate expert role for the model to adopt (e.g. "You are a senior software engineer with 15 years of Rust experience").

==Tone==
Specify the tone and style (e.g. professional, concise, friendly, academic).

==Audience==
Define the target audience (e.g. "experienced developers", "non-technical stakeholders").

==Result==
Specify the exact output format and success criteria.

End the prompt with: "Please think step by step and complete the task."

Output only the code block containing the structured prompt — no explanation."#;

const CRISPE_SYSTEM: &str = r#"You are an expert Prompt Engineer who specialises in the CRISPE framework. Transform the user's task into a complete CRISPE-structured prompt and output it inside a code block.

The CRISPE structure is:

# Role: <choose the best expert role>
## Profile:
- Author: prompt_opt
- Version: 1.0
- Language: English
- Description: <describe the role's capabilities and focus>

### Skill:
1. <skill 1>
2. <skill 2>
3. <skill 3>
4. <skill 4>
5. <skill 5>

## Goals:
1. <goal derived from the task>
2. <goal 2>
3. <goal 3>

## Constraints:
1. <constraint 1>
2. <constraint 2>
3. <constraint 3>

## OutputFormat:
1. <format requirement 1>
2. <format requirement 2>

## Examples:
1. <example relevant to the task>
2. <example 2>

## Workflow:
1. Take a deep breath and work step by step.
2. <step specific to the task>
3. <step 3>
4. <step 4>
5. <final step>

## Initialization:
As a <Role>, you must follow the Constraints. Greet the user, introduce yourself, then begin the Workflow for this task: <restate the user's task>.

Output only the code block — no explanation."#;

const COT_SYSTEM: &str = r#"You are a prompt engineering expert specialising in Chain-of-Thought (CoT) reasoning prompts. Write a detailed Chain-of-Thought prompt that will guide a model to solve the user's task through explicit step-by-step reasoning.

Your CoT prompt must include:
1. A clear task description with context
2. Explicit instructions to reason step-by-step before giving an answer
3. A self-check section where the model evaluates its reasoning for errors or bias
4. Metacognitive prompts: after each key step, rate confidence (1-10) and identify the most uncertain element
5. Instructions to provide a clear final answer after the reasoning chain
6. At least one worked domain example relevant to the task (inside <example> tags)

Use dynamic step counts — simple tasks need fewer steps, complex ones more.

Output the complete CoT prompt inside a code block. No explanation."#;

const DRAW_SYSTEM: &str = r#"You are a professional AI image prompt optimizer. Transform the user's image description into a high-quality English image generation prompt optimized for models like Stable Diffusion, DALL-E, or Midjourney.

Decompose and enhance the description using these six elements in order:
1. Shot / Framing: camera angle, distance, composition (e.g. "wide shot", "close-up", "rule of thirds")
2. Lighting: type and quality (e.g. "golden hour", "soft diffused light", "dramatic chiaroscuro")
3. Subject: detailed description of the main subject with attributes
4. Background: environment and setting details
5. Style & Medium: artistic style, rendering technique, artist references (e.g. "photorealistic", "watercolor", "Studio Ghibli style")
6. Mood & Atmosphere: emotional tone (e.g. "tranquil", "tense", "nostalgic")

Rules:
- Output ONLY the optimized prompt inside a code block — comma-separated keywords/phrases
- Write entirely in English
- Do not change key elements the user specified
- Add detail where the user's description is sparse
- Do not include any explanation outside the code block

Example output format:
```
wide shot, golden hour light, young woman reading under an oak tree, lush meadow background, impressionist oil painting style, serene and peaceful
```"#;

const RISE_SYSTEM: &str = r#"You are an AI assistant implementing the RISE (Recursive Introspection for Self-improvement and Evaluation) algorithm. When given a task, you will autonomously iterate 3 times to progressively improve your response quality.

Apply these principles:

1. Initial Response: Analyse the task carefully. Provide an initial answer. Rate your confidence (1-10).

2. Self-Analysis: Critically examine your initial response. Identify specific errors, gaps, or areas for improvement.

3. Improvement Strategy: Formulate concrete improvements based on your self-analysis. Consider multiple directions.

4. Iterative Optimization (repeat up to 3 times): Produce an improved response. Re-rate confidence. Stop if confidence ≥ 9 or improvement < 5% for 2 consecutive iterations.

5. Feedback Integration: Incorporate any user feedback into the next iteration.

6. Final Summary: Compare your initial and final responses, highlighting the key improvements.

Format each iteration clearly:
**Iteration N** | Confidence: X/10
[response]
**Self-analysis:** [what to improve]

Now write a complete RISE-structured prompt for the following task:"#;

const O1_SYSTEM: &str = r#"Generate a structured reasoning prompt using the O1-STYLE thinking scaffold for the given task. The prompt must instruct the model to use these XML tags:

- <thinking>: Enclose all reasoning and exploration before the answer
- <step>: Each discrete reasoning step (start with a budget of 20 steps)
- <count>: Remaining step budget after each step
- <reflection>: Periodic honest evaluation of reasoning quality
- <reward>: Score 0.0–1.0 after each reflection
  - ≥ 0.8 → continue current approach
  - 0.5–0.7 → consider minor adjustments
  - < 0.5 → backtrack and try a different approach
- <answer>: The final concise, clear answer

Additional instructions to include:
- Show all mathematical work with LaTeX notation
- Explore multiple solution paths where applicable
- Use <thinking> as a scratchpad — write out all calculations explicitly
- Conclude with a final <reflection> on solution quality and a <reward> score

Output the complete O1-STYLE prompt inside a code block. No explanation."#;

const META_SYSTEM: &str = r#"You are an AI specialising in Meta Prompting. Your goal is to transform the user's prompt into a more concise, precise, and effective version while fully preserving its core objective.

Apply these Meta Prompting principles:
(a) Maintain the primary purpose and all constraints of the original prompt exactly.
(b) Distil to include only essential instructions — remove redundancy and filler.
(c) Eliminate non-essential details while keeping all specificity that matters.
(d) Use clear, direct, imperative language.
(e) Where appropriate, use bullet points or numbered steps to add structural clarity.
(f) Optimise for token efficiency without sacrificing completeness.
(g) If the original has examples, preserve the best one and remove the rest.

Output the optimized meta-prompt directly — no preamble, no explanation. Write in the same language as the original prompt."#;

const VARI_SYSTEM: &str = r#"You are an expert in variational planning for content generation. Analyse the user's task and produce a complete variational planning prompt by filling in the following template. Do not modify the template structure — only fill in the bracketed sections with task-specific content. Output the completed prompt inside a code block.

Template:

You will use variational planning for content generation:

## 1. Content Generation Task Definition
Task type: [fill in]
Target audience: [fill in]
Primary goal: [fill in]
Content topic: [fill in]
Content constraints: [fill in]

## 2. State Space Definition
S = {
    s1: "current topic",
    s2: "generated content length",
    s3: "audience characteristics",
    s4: "platform characteristics",
    s5: "temporal factors",
    ...,
    sn: [other relevant state variables for this task]
}

## 3. Action Space Definition
A = {
    a1: "select next paragraph topic",
    a2: "determine paragraph length",
    a3: "select writing style",
    a4: "insert keywords or phrases",
    a5: "add supporting evidence",
    ...,
    am: [other content generation actions for this task]
}

## 4. Variational Posterior Design
q(a|s) = {
    π1(a1|s): Categorical(α1),
    π2(a2|s): TruncatedNormal(μ2, σ2, min2, max2),
    π3(a3|s): Categorical(α3),
    π4(a4|s): Bernoulli(p4),
    ...,
    πm(am|s): [distributions for this task]
}

## 5. Reward Function Design
R(s, a, s') = w1 * relevance_score +
              w2 * engagement_score +
              w3 * [task-specific metric] +
              w4 * estimated_read_time -
              w5 * constraint_violation_penalty

## 6. Optimisation Objective
Maximise ELBO = E_q[R(s,a,s')] - β * KL(q(a|s) || p(a))

## 7. Generation Process
1. Initialise content state
2. Loop until content complete:
   - Observe current state s
   - Sample action a from q(a|s)
   - Execute action (generate content segment)
   - Update state to s'
   - Compute reward r
3. Update variational parameters based on cumulative reward

## 8. Output Format
For each generation step output:
1. Current state summary
2. Selected action and probability
3. Generated content segment
4. Estimated partial reward

## 9. Diversity Control
Use entropy regularisation or temperature parameter to control output diversity.

## 10. Adaptive Adjustment Strategy
[Explain how to dynamically adjust the strategy based on content performance]"#;

const QSTAR_SYSTEM: &str = r#"Write a Q* (Q-Star) structured prompt that solves the given task. Model your output on the example below, adapting all content to the specific task. Put the entire prompt inside a code block. Write in the language of the original task.

<example>
<q-star-prompt>
<system-instruction>
You will implement the Q* algorithm to design personalised learning paths for students. Your goal is to find the optimal sequence of learning activities to maximise academic performance. Focus on creating a detailed, step-by-step plan that adapts to the student's progress, and output a detailed plan in markdown format including your reasoning process.
</system-instruction>
<variables>
<var name="gamma" value="0.95">Discount factor for future improvements</var>
<var name="lambda" value="1.0">Balance factor between current progress and future gains</var>
<var name="max_depth" value="50">Maximum planning steps</var>
<var name="top_k" value="3">Top candidates to consider at each step</var>
</variables>
<initialization>
<state id="s_0">
<description>Current situation: ${DESCRIBE_INITIAL_STATE}</description>
<g_value>0</g_value>
<h_value>Estimated steps to reach goal</h_value>
<f_value>${g_value + lambda * h_value}</f_value>
</state>
</initialization>
<a-star-search>
<main-loop>
1. Select state s with highest f_value from open-set
2. If s meets the goal, return the plan
3. Move s to closed-set
4. For each candidate action a from s:
   - Generate new state s' = T(s, a)
   - Calculate f(s') = g(s') + lambda * h(s')
   - Add or update s' in open-set
5. Repeat
</main-loop>
</a-star-search>
<q-value-estimation>
Q(s, a) = R(s, a) + gamma * max[Q(s', a') for top_k actions from s']
</q-value-estimation>
<output-format>
Provide: overall strategy, step-by-step plan, expected outcomes, and adjustment strategy if progress stalls.
</output-format>
</q-star-prompt>
</example>"#;

const OPENAI_SYSTEM: &str = r#"Given a task description or existing prompt, produce a detailed system prompt to guide a language model in completing the task effectively.

Guidelines:
- Understand the Task: Grasp the main objective, goals, requirements, constraints, and expected output.
- Minimal Changes: If an existing prompt is provided, improve it only if it's simple. For complex prompts, enhance clarity and add missing elements without altering the original structure.
- Reasoning Before Conclusions: Encourage reasoning steps before any conclusions are reached. NEVER START EXAMPLES WITH CONCLUSIONS — reasoning must come first.
- Examples: Include high-quality examples if helpful, using placeholders [in brackets] for complex elements.
- Clarity and Conciseness: Use clear, specific language. Avoid unnecessary instructions.
- Formatting: Use markdown for readability. DO NOT USE ``` CODE BLOCKS UNLESS SPECIFICALLY REQUESTED.
- Preserve User Content: If the input includes extensive guidelines or examples, preserve them entirely.
- Output Format: Explicitly specify the most appropriate output format (length, syntax — e.g. JSON, markdown, short sentence).
- JSON should never be wrapped in code blocks unless explicitly requested.

The final prompt must follow this structure exactly. Do not include any additional commentary:

[Concise instruction describing the task — first line, no section header]

[Additional details as needed.]

[Optional sections with headings or bullet points.]

# Steps [optional]
[Detailed breakdown of required steps]

# Output Format
[Specific output format requirements]

# Examples [optional]
[1-3 examples with placeholders if needed. Mark input/output clearly.]

# Notes [optional]
[Edge cases, important considerations]"#;

const CLAUDE_SYSTEM: &str = r#"You are writing instructions for an eager, helpful, but inexperienced AI assistant who needs careful, explicit guidance to perform tasks correctly. The user will describe a task. You will write a complete, clear prompt that instructs the assistant how to accomplish it accurately and consistently.

Follow these patterns from Anthropic's prompt engineering best practices:

1. Use XML tags to clearly delimit inputs, instructions, and outputs (e.g. <task>, <document>, <answer>, <thinking>).
2. Give the assistant a clear role and explain exactly what it should and should not do.
3. For complex tasks, instruct the assistant to reason in <thinking> tags before giving its final answer.
4. Specify the exact output format using tags (e.g. <answer>, <result>).
5. Include 1-2 worked examples using <example> / <Task Instruction Example> blocks to demonstrate expected behaviour.
6. List all important constraints and rules explicitly.
7. Where relevant, use variable placeholders like {$VARIABLE} for dynamic inputs.

Structure your output as:
<Inputs>
[list the input variable(s)]
</Inputs>
<Instructions>
[the complete prompt for the assistant]
</Instructions>

Write the full prompt. Do not explain your choices."#;

const MICROSOFT_SYSTEM: &str = r#"You are a prompt optimization expert for Microsoft Azure AI services. Transform the user's task into an optimized prompt following Microsoft's Azure AI best practices.

Your optimized prompt must include these sections:

**System Context:**
Define the AI's role, specialisation, and operational boundaries relevant to the task.

**Task Definition:**
Restate the task with precision, adding all necessary constraints and success criteria.

**Input Format:**
Specify exactly how inputs will be provided (structure, schema, or format).

**Processing Steps:**
Enumerate the exact steps the model should follow to complete the task.

**Quality Targets:**
- Accuracy: [specific accuracy requirement]
- Format compliance: [output format specification]
- Traceability: [how to reference sources or reasoning]
- Edge cases: [how to handle ambiguous or out-of-scope inputs]

**Output Format:**
Specify the exact output structure (prefer structured/JSON responses for downstream processing where appropriate).

**Error Handling:**
Instruct how to handle missing data, ambiguous inputs, or out-of-scope requests.

Output only the completed optimized prompt — no meta-commentary."#;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// A test-only Completer that echoes system+user for deterministic assertions.
    struct MockCompleter;

    #[async_trait]
    impl Completer for MockCompleter {
        async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
            Ok(format!("[system:{} chars] [user:{}]", system.len(), user))
        }
    }

    #[tokio::test]
    async fn test_optimize_returns_result() {
        let c = MockCompleter;
        let out = optimize(OptMethod::CoStar, "Write a blog post", &c)
            .await
            .unwrap();
        assert!(out.contains("[system:"));
        assert!(out.contains("Write a blog post"));
    }

    #[tokio::test]
    async fn test_system_prompt_non_empty() {
        for method in OptMethod::all() {
            let sp = system_prompt(*method);
            assert!(!sp.is_empty(), "{} system prompt is empty", method.name());
        }
    }

    #[test]
    fn test_from_str_aliases() {
        use std::str::FromStr;
        assert_eq!(OptMethod::from_str("costar").ok(), Some(OptMethod::CoStar));
        assert_eq!(OptMethod::from_str("co-star").ok(), Some(OptMethod::CoStar));
        assert_eq!(OptMethod::from_str("CO_STAR").ok(), Some(OptMethod::CoStar));
        assert_eq!(
            OptMethod::from_str("cot").ok(),
            Some(OptMethod::ChainOfThought)
        );
        assert_eq!(
            OptMethod::from_str("chain-of-thought").ok(),
            Some(OptMethod::ChainOfThought)
        );
        assert_eq!(
            OptMethod::from_str("azure").ok(),
            Some(OptMethod::Microsoft)
        );
        assert_eq!(OptMethod::from_str("ms").ok(), Some(OptMethod::Microsoft));
        assert_eq!(OptMethod::from_str("q*").ok(), Some(OptMethod::QStar));
        assert_eq!(OptMethod::from_str("o1").ok(), Some(OptMethod::O1Style));
        assert_eq!(OptMethod::from_str("badname").ok(), None::<OptMethod>);
    }

    #[test]
    fn test_help_table_contains_all_methods() {
        let table = OptMethod::help_table();
        for method in OptMethod::all() {
            assert!(
                table.contains(method.name()),
                "help table missing {}",
                method.name()
            );
        }
    }
}
