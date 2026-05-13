# Pi vs ragent Feature Comparison & Implementation Plan

**Document Version:** 1.0  
**Date:** 2025-01-17  
**Status:** Analysis Complete, Implementation Plan Ready

---

## Executive Summary

This document compares the Pi terminal coding agent (https://pi.dev) with ragent, identifying key features present in Pi that are missing from ragent. The analysis focuses on **non-TypeScript-specific features** — functionality that can be implemented in Rust without requiring a JavaScript/TypeScript runtime.

**Key Finding:** Pi has a more mature session management system with tree-based branching, sophisticated context compaction, and better user experience features like prompt templates, customizable themes, and mid-session model switching.

---

## Part 1: Feature Comparison Matrix

### Legend
- ✅ **Implemented** — Feature exists in ragent
- ⚠️ **Partial** — Partially implemented or different approach
- ❌ **Missing** — Feature not present in ragent
- 🚫 **Out of Scope** — TypeScript-dependent feature

### 1.1 Core Session Management

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| Session persistence | ✅ JSONL tree | ✅ SQLite | Different formats |
| **Tree-based sessions** | ✅ Full tree | ❌ Linear | Pi stores sessions as trees with parent/child relationships |
| **Branching / Forking** | ✅ `/tree`, `/fork`, `/clone` | ❌ None | Pi can branch from any point in history |
| **Session naming** | ✅ `/name` | ❌ Auto-generated | Pi allows human-readable names |
| **Session labels** | ✅ Shift+L to label entries | ❌ None | Pi can bookmark specific messages |
| **Resume session picker** | ✅ Interactive with search | ⚠️ Basic list | Pi has rich picker with filters |
| **Export to HTML** | ✅ `/export` | ❌ None | Pi generates shareable HTML |
| **Share to Gist** | ✅ `/share` | ❌ None | Pi uploads to GitHub Gist |
| **Session statistics** | ✅ `/session` shows tokens/cost | ⚠️ Basic info | Pi tracks token usage and cost |
| **Tree navigation view** | ✅ Interactive tree UI | ❌ None | Pi has visual tree browser |

### 1.2 Context Management

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| **Auto-compaction** | ✅ Configurable thresholds | ❌ None | Pi auto-summarizes when approaching context limit |
| **Manual compaction** | ✅ `/compact [prompt]` | ❌ None | Pi can manually trigger with custom instructions |
| **Branch summarization** | ✅ On `/tree` navigation | ❌ None | Pi preserves context when switching branches |
| **Split turn handling** | ✅ Mid-turn cutting | ❌ N/A | Pi handles single turns exceeding token budget |
| **Iterative summaries** | ✅ Compounds summaries | ❌ N/A | Pi builds on previous compaction entries |
| **File tracking in summaries** | ✅ Cumulative file ops | ❌ N/A | Pi tracks all file operations in compaction |

### 1.3 User Experience Features

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| **Prompt templates** | ✅ Markdown files, `/name` expansion | ❌ None | Pi has reusable prompt snippets with arguments |
| **Skills (Agent Skills)** | ✅ SKILL.md standard | ⚠️ Different approach | Pi implements progressive disclosure; ragent has different skill system |
| **Custom themes** | ✅ JSON theme files | ❌ Fixed themes | Pi has 51-color customizable themes |
| **Custom keybindings** | ✅ Full keybindings.json | ❌ Hardcoded | Pi allows customizing all shortcuts |
| **Steering messages** | ✅ Enter during execution | ❌ None | Pi can submit messages while agent works |
| **Follow-up messages** | ✅ Alt+Enter | ❌ None | Pi queues messages to send after completion |
| **Double-escape action** | ✅ Configurable | ❌ None | Pi: tree/fork/none on double Esc |
| **Editor padding** | ✅ Configurable | ��� Fixed | Pi: editorPaddingX setting |
| **Autocomplete count** | ✅ Configurable | ❌ Fixed | Pi: autocompleteMaxVisible setting |

### 1.4 Provider & Model Management

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| Provider count | 15+ | 8 | Pi: DeepSeek, Groq, Cerebras, xAI, Mistral, etc. |
| **Mid-session model switch** | ✅ `/model` or Ctrl+L | ❌ Session restart | Pi can switch models without restarting |
| **Model cycling** | ✅ Ctrl+P favorites | ❌ None | Pi cycles through favorite models |
| **Subscription auth** | ✅ `/login` OAuth | ❌ API keys only | Pi supports Codex, Claude Pro/Max, Copilot OAuth |
| **Custom model entries** | ✅ models.json | ⚠️ Limited | Pi has full model definition files |
| **Thinking budgets** | ✅ Configurable per level | ⚠️ Basic | Pi: custom token budgets per thinking level |
| **Scoped model selector** | ✅ Ctrl+M | ❌ None | Pi: quick model switcher with categories |

### 1.5 Input/Output Modes

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| Interactive TUI | ✅ Full TUI | ✅ Full TUI | Both have ratatui-based interfaces |
| **Print mode** | ✅ `pi -p` | ⚠️ `ragent run` | Pi has better non-interactive support |
| **JSON event stream** | ✅ `--mode json` | ❌ SSE only | Pi outputs structured events |
| **RPC mode** | ✅ `--mode rpc` | ❌ None | Pi has JSONL protocol over stdin/stdout |
| **SDK** | ✅ Node.js SDK | ❌ HTTP API only | Pi has embeddable SDK (TypeScript) |

### 1.6 Security & Permissions

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| Built-in safe commands | ✅ 51 commands | ✅ 51+ commands | Similar approaches |
| Permission system | 🚫 Extensions | ✅ Built-in | Pi delegates to extensions; ragent has built-in |
| Path protection | 🚫 Extensions | ✅ Built-in | Similar capability, different implementation |

### 1.7 Extension System (TypeScript-Dependent - Out of Scope)

| Feature | Pi | ragent | Notes |
|---------|-----|--------|-------|
| TypeScript extensions | ✅ Full system | ❌ N/A | **Out of scope** - requires TS runtime |
| Custom tools via extensions | ✅ Yes | ❌ N/A | **Out of scope** |
| Custom commands | ✅ Yes | ❌ N/A | **Out of scope** |
| Custom UI components | ✅ Yes | ❌ N/A | **Out of scope** |

---

## Part 2: Missing Features - Detailed Analysis

### Priority 1: High Impact, User-Facing

#### 2.1 Session Tree/Branching System
**Current State:** ragent has linear session history in SQLite
**Pi Advantage:** Tree structure allows exploring alternative paths without losing history
**User Value:** Can try approach A, then go back and try approach B from same point
**Implementation Complexity:** High — requires schema changes and new TUI components

#### 2.2 Context Compaction
**Current State:** No automatic context management
**Pi Advantage:** Auto-summarizes old messages when approaching token limits
**User Value:** Can have very long sessions without hitting context limits
**Implementation Complexity:** Medium — requires LLM call to generate summaries

#### 2.3 Prompt Templates
**Current State:** ragent has `/opt` for prompt optimization, but no user-defined templates
**Pi Advantage:** Users can create reusable prompt templates with argument substitution
**User Value:** Quickly invoke common prompts with `/template-name args...`
**Implementation Complexity:** Low — file loading + simple substitution

#### 2.4 Mid-Session Model Switching
**Current State:** Model fixed per session
**Pi Advantage:** Can switch models mid-conversation with `/model` or Ctrl+L
**User Value:** Use cheap model for simple tasks, expensive model for complex ones
**Implementation Complexity:** Low — already have multi-provider support

#### 2.5 Custom Themes
**Current State:** Fixed dark theme
**Pi Advantage:** JSON theme files with 51 customizable color tokens
**User Value:** Personalize appearance, accessibility
**Implementation Complexity:** Medium — requires theme system refactor

### Priority 2: Medium Impact, Nice to Have

#### 2.6 Session Export/Share
**Pi Features:** `/export` to HTML, `/share` to GitHub Gist
**User Value:** Share conversations, documentation
**Implementation Complexity:** Medium — HTML generation, GitHub API integration

#### 2.7 Custom Keybindings
**Current State:** Hardcoded keybindings in TUI
**Pi Advantage:** Full customization via keybindings.json
**User Value:** Emacs/vim-style configurations
**Implementation Complexity:** Medium — requires keybinding config system

#### 2.8 Additional Providers
**Missing Providers:** DeepSeek, Groq, Cerebras, xAI, Mistral, Azure OpenAI, Bedrock, etc.
**User Value:** More model options, price points
**Implementation Complexity:** Low-Medium per provider

#### 2.9 Steering Messages
**Current State:** Must wait for agent to finish
**Pi Advantage:** Can send messages while agent is working (Enter vs Alt+Enter)
**User Value:** Course-correct agent in real-time
**Implementation Complexity:** Medium — requires execution loop changes

#### 2.10 Agent Skills (Standard)
**Current State:** ragent has different skill system
**Pi Advantage:** Implements agentskills.io standard with progressive disclosure
**User Value:** Interoperability, ecosystem compatibility
**Implementation Complexity:** Medium — new loading mechanism

### Priority 3: Lower Impact, Advanced Features

#### 2.11 Session Labels/Bookmarks
**Pi Feature:** Can label specific messages in history
**User Value:** Bookmark important points
**Implementation Complexity:** Low

#### 2.12 Branch Summarization
**Pi Feature:** Summarize context when switching branches via `/tree`
**User Value:** Maintain context across branches
**Implementation Complexity:** Medium — requires summary generation

#### 2.13 Print/JSON/RPC Modes
**Pi Feature:** Multiple non-interactive output modes
**User Value:** Scripting, integration
**Implementation Complexity:** Medium

#### 2.14 Subscription Authentication
**Pi Feature:** OAuth for Codex, Claude Pro/Max, Copilot
**User Value:** Use subscription models
**Implementation Complexity:** High — requires OAuth flows

---

## Part 3: Implementation Milestones

### Milestone 1: Session Tree System (Core Infrastructure)
**Duration:** 3-4 weeks  
**Goal:** Implement tree-based session storage and navigation

#### Tasks
1. **Database Schema Migration**
   - Add `parent_id`, `branch_id` columns to messages table
   - Create tree index for efficient traversal
   - Migration script for existing linear sessions

2. **Session Tree Data Model**
   - Define TreeNode, Branch, SessionTree structs
   - Implement tree traversal algorithms
   - Add tree serialization/deserialization

3. **Tree Navigation TUI Component**
   - Create tree view overlay (similar to Pi's `/tree`)
   - Implement folding/unfolding of branches
   - Add visual indicators for active branch

4. **Fork/Clone Commands**
   - `/fork` command to branch from a message
   - `/clone` command to duplicate current branch
   - Branch naming and management

5. **Session State Management Updates**
   - Update session processor to handle tree navigation
   - Implement branch switching logic
   - Persist branch metadata

**Success Criteria:**
- Can create branches from any message
- Can switch between branches
- Tree visualization works
- Existing sessions migrate successfully

---

### Milestone 2: Context Compaction (Auto & Manual)
**Duration:** 2-3 weeks  
**Goal:** Implement automatic and manual context compaction

#### Tasks
1. **Compaction Trigger System**
   - Monitor token count approaching context window
   - Configurable `reserveTokens` (default 16k)
   - Configurable `keepRecentTokens` (default 20k)

2. **Summary Generation**
   - LLM call to summarize message batch
   - Structured summary format (conversations, files, decisions)
   - Iterative summary building (compound summaries)

3. **Compaction Entry Storage**
   - New entry type: `CompactionEntry`
   - Store `firstKeptEntryId` reference
   - Track cumulative file operations

4. **Manual Compaction Command**
   - `/compact [instructions]` slash command
   - Custom instructions for focused summarization
   - Immediate compaction trigger

5. **Split Turn Handling**
   - Handle single turns exceeding token budget
   - Cut at assistant message boundaries
   - Preserve partial context

**Success Criteria:**
- Auto-compaction triggers at configured threshold
- Summaries preserve context effectively
- Manual `/compact` works with custom instructions
- Long sessions stay within context limits

---

### Milestone 3: Prompt Templates
**Duration:** 1-2 weeks  
**Goal:** Implement user-defined prompt templates with arguments

#### Tasks
1. **Template Discovery**
   - Load from `~/.ragent/prompts/*.md`
   - Load from `.ragent/prompts/*.md` (project-local)
   - CLI flag `--prompt-template <path>`

2. **Template Format**
   - Frontmatter: `description`, `argument-hint`
   - Body: template content with `$1`, `$2`, `$@` substitution
   - Support `${@:N}` and `${@:N:L}` slicing

3. **Slash Command Integration**
   - `/template-name` expands to template content
   - Autocomplete with descriptions
   - Argument hints in autocomplete

4. **Argument Substitution Engine**
   - Parse and substitute positional arguments
   - Handle quoted arguments
   - Escape special characters

**Success Criteria:**
- Templates load from global and project directories
- `/template-name` expands correctly
- Arguments substitute properly
- Autocomplete shows available templates

---

### Milestone 4: Mid-Session Model Switching
**Duration:** 1 week  
**Goal:** Allow switching models without restarting session

#### Tasks
1. **Model Registry Updates**
   - Enable model switching without session reset
   - Update conversation context for new model
   - Preserve conversation history across switches

2. **Slash Commands**
   - `/model` — open model picker
   - `/model <provider/model>` — direct switch

3. **Keybinding**
   - Ctrl+L — quick model switcher
   - Ctrl+P — cycle through favorites

4. **Scoped Model Selector UI**
   - Quick picker with provider categories
   - Favorite models section
   - Recent models section

**Success Criteria:**
- Can switch models mid-conversation
- Conversation context preserved
- Model picker UI works
- Favorites and cycling work

---

### Milestone 5: Custom Themes
**Duration:** 2-3 weeks  
**Goal:** Implement customizable theme system

#### Tasks
1. **Theme Schema Definition**
   - 51 color tokens (match Pi specification)
   - Vars section for reusable colors
   - JSON schema for validation

2. **Theme Loading**
   - Built-in: dark, light
   - Global: `~/.ragent/themes/*.json`
   - Project: `.ragent/themes/*.json`
   - CLI: `--theme <path>`

3. **Theme Application**
   - Refactor TUI to use theme tokens
   - Hot-reload on theme file change
   - Fallback for missing tokens

4. **Theme Tokens (51 total)**
   - Core UI: accent, border, success, error, warning, muted, dim, text, etc.
   - Backgrounds: selectedBg, userMessageBg, toolSuccessBg, etc.
   - Markdown: mdHeading, mdLink, mdCode, mdQuote, etc.
   - Syntax highlighting: syntaxKeyword, syntaxFunction, etc.
   - Thinking levels: thinkingOff, thinkingLow, etc.

**Success Criteria:**
- Custom themes load and apply
- All 51 tokens configurable
- Hot-reload works
- Default dark/light themes included

---

### Milestone 6: Additional Providers
**Duration:** 2-3 weeks (can parallelize)
**Goal:** Add 7+ new LLM providers

#### Tasks
1. **DeepSeek Provider**
   - API key auth
   - Model endpoints
   - Streaming support

2. **Groq Provider**
   - API key auth
   - High-speed inference
   - Model list

3. **Cerebras Provider**
   - API key auth
   - Cerebras-specific endpoints

4. **xAI (Grok) Provider**
   - API key auth
   - Grok model support

5. **Mistral Provider**
   - API key auth
   - Mistral model family

6. **Azure OpenAI Provider**
   - Azure-specific auth (endpoint + key)
   - Deployment-based models

7. **Amazon Bedrock Provider**
   - AWS credential chain
   - Bedrock model access

**Success Criteria:**
- All providers authenticate successfully
- Streaming works
- Model listing works
- Error handling implemented

---

### Milestone 7: Session Export & Share
**Duration:** 1-2 weeks  
**Goal:** Export sessions to HTML and share to GitHub Gist

#### Tasks
1. **HTML Export**
   - Generate styled HTML from session
   - Include syntax highlighting
   - Include tool outputs with formatting
   - `/export [file]` command

2. **GitHub Gist Integration**
   - GitHub API client
   - Create private gists
   - Generate shareable URLs
   - `/share` command

3. **Session Naming**
   - `/name <name>` command
   - Named sessions in picker
   - Display name in UI

4. **Session Statistics**
   - Token count tracking
   - Cost estimation
   - `/session` command output

**Success Criteria:**
- HTML export generates valid files
- Gist sharing creates accessible URLs
- Session naming works
- Statistics display correctly

---

### Milestone 8: Custom Keybindings
**Duration:** 2 weeks  
**Goal:** Allow full customization of keyboard shortcuts

#### Tasks
1. **Keybinding Schema**
   - Namespaced action IDs (e.g., `tui.editor.cursorUp`)
   - Modifier format: `ctrl+shift+x`
   - Key list definition

2. **Keybinding Loading**
   - Global: `~/.ragent/keybindings.json`
   - Project: `.ragent/keybindings.json`
   - Migration from old format

3. **Keybinding Application**
   - Replace hardcoded bindings with configurable
   - Default configs for Emacs/Vim presets
   - Conflict detection

4. **Keybinding Actions (40+)**
   - Editor: cursor movement, deletion, clipboard
   - Application: quit, reload, cancel
   - Sessions: new, resume, fork, clone
   - Models: switch, cycle
   - Display: expand/collapse, scroll
   - Tree: navigation, fold/unfold

**Success Criteria:**
- Keybindings load from JSON
- All actions configurable
- Emacs/Vim presets work
- No binding conflicts

---

### Milestone 9: Steering Messages & Advanced Input
**Duration:** 2 weeks  
**Goal:** Allow messages during agent execution

#### Tasks
1. **Execution Loop Refactoring**
   - Make execution interruptible
   - Queue steering messages
   - Handle follow-up messages

2. **Steering Message System**
   - `Enter` — send steering message (interrupts after current tool)
   - `Alt+Enter` — queue follow-up (waits for completion)
   - UI indication of queued messages

3. **Input State Management**
   - Distinguish between idle and executing states
   - Show appropriate UI cues
   - Handle message queuing

4. **Double-Escape Action**
   - Configurable: tree/fork/none
   - `doubleEscapeAction` setting
   - Implementation for each action

**Success Criteria:**
- Can send messages during execution
- Steering interrupts appropriately
- Follow-up queues correctly
- Double-escape works as configured

---

### Milestone 10: Agent Skills (Standard)
**Duration:** 2-3 weeks  
**Goal:** Implement agentskills.io standard

#### Tasks
1. **SKILL.md Parser**
   - Frontmatter extraction (name, description, compatibility, etc.)
   - Validation against spec
   - Error reporting

2. **Skill Discovery**
   - Global: `~/.ragent/skills/`
   - Project: `.ragent/skills/`
   - Progressive disclosure (descriptions in context, full content on-demand)

3. **Skill Commands**
   - `/skill:name` to load and execute
   - Arguments appended as user message
   - Autocomplete integration

4. **Skill Execution**
   - Load full SKILL.md on demand
   - Execute setup instructions
   - Run usage instructions

5. **Compatibility with Other Harnesses**
   - Load Claude Code skills
   - Load OpenAI Codex skills
   - Standard format support

**Success Criteria:**
- Skills load from standard locations
- `/skill:name` executes correctly
- Progressive disclosure works
- Compatible with other harness skills

---

### Milestone 11: Session Labels & Branch Summarization
**Duration:** 1-2 weeks  
**Goal:** Add message labeling and branch context preservation

#### Tasks
1. **Message Labeling**
   - Shift+L to set/clear label on entry
   - Shift+T to toggle timestamps
   - Store labels in session

2. **Branch Summarization**
   - Generate summary when switching branches
   - Include in session context
   - BranchSummaryEntry type

3. **Tree Filter Modes**
   - Default, no-tools, user-only, labeled-only, all
   - `treeFilterMode` setting
   - Ctrl+O to cycle filters

**Success Criteria:**
- Labels can be set/cleared
- Branch switching preserves context via summaries
- Filter modes work in tree view

---

### Milestone 12: Non-Interactive Modes (Print/JSON)
**Duration:** 2 weeks  
**Goal:** Add print and JSON event stream modes

#### Tasks
1. **Print Mode (`-p`)**
   - Non-interactive execution
   - Plain text output
   - Script-friendly

2. **JSON Event Stream (`--mode json`)**
   - Structured events to stdout
   - Event types: message, tool_call, tool_result, error, done
   - JSONL format

3. **RPC Mode (`--mode rpc`)**
   - JSONL protocol over stdin/stdout
   - Request/response format
   - Method handlers

4. **Mode-Specific Output Handlers**
   - Different output formatting per mode
   - Event serialization
   - Error handling

**Success Criteria:**
- Print mode works non-interactively
- JSON events stream correctly
- RPC mode accepts commands
- All modes handle errors gracefully

---

### Milestone 13: Subscription Authentication (OAuth)
**Duration:** 3-4 weeks  
**Goal:** Support OAuth for subscription providers

#### Tasks
1. **OAuth Flow Infrastructure**
   - Local redirect server
   - PKCE support
   - Token storage (secure)

2. **OpenAI Codex OAuth**
   - ChatGPT Plus/Pro subscription auth
   - Token refresh
   - Scope handling

3. **Claude Pro/Max OAuth**
   - Anthropic subscription auth
   - Extra usage tracking
   - Billing considerations

4. **GitHub Copilot OAuth**
   - GitHub device flow
   - Enterprise Server support
   - Model enablement checks

5. **Auth Management**
   - `/login` command
   - `/logout` command
   - `~/.ragent/auth.json` storage

**Success Criteria:**
- OAuth flows complete successfully
- Tokens stored securely
- Auto-refresh works
- Auth status visible

---

## Part 4: Implementation Order & Dependencies

### Phase 1: Foundation (Weeks 1-6)
1. **Milestone 1:** Session Tree System
2. **Milestone 2:** Context Compaction
3. **Milestone 4:** Mid-Session Model Switching

### Phase 2: User Experience (Weeks 7-12)
4. **Milestone 3:** Prompt Templates
5. **Milestone 5:** Custom Themes
6. **Milestone 7:** Session Export & Share

### Phase 3: Advanced Features (Weeks 13-18)
7. **Milestone 6:** Additional Providers
8. **Milestone 8:** Custom Keybindings
9. **Milestone 9:** Steering Messages

### Phase 4: Ecosystem (Weeks 19-24)
10. **Milestone 10:** Agent Skills (Standard)
11. **Milestone 11:** Session Labels & Branch Summarization
12. **Milestone 12:** Non-Interactive Modes
13. **Milestone 13:** Subscription Authentication

---

## Part 5: Technical Considerations

### 5.1 Database Schema Changes

The session tree requires significant schema updates:

```sql
-- Current (linear)
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    role TEXT,
    content TEXT,
    created_at TIMESTAMP
);

-- New (tree-based)
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    parent_id TEXT,          -- NEW: tree parent
    branch_id TEXT,          -- NEW: branch identifier
    entry_type TEXT,         -- NEW: user, assistant, tool, compaction, summary, etc.
    role TEXT,
    content TEXT,
    metadata JSON,           -- NEW: flexible metadata
    created_at TIMESTAMP
);

-- New table for branches
CREATE TABLE branches (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    name TEXT,
    root_message_id TEXT,    -- First message in branch
    is_active BOOLEAN
);

-- New table for compactions
CREATE TABLE compactions (
    id TEXT PRIMARY KEY,
    session_id TEXT,
    branch_id TEXT,
    summary TEXT,
    first_kept_message_id TEXT,
    tokens_before INTEGER,
    files_modified JSON       -- Cumulative file tracking
);
```

### 5.2 Configuration Extensions

New settings to add to `ragent.json`:

```jsonc
{
  "compaction": {
    "enabled": true,
    "reserveTokens": 16384,
    "keepRecentTokens": 20480,
    "triggerThreshold": 0.8
  },
  "theme": "dark",
  "doubleEscapeAction": "tree",
  "treeFilterMode": "default",
  "editorPaddingX": 0,
  "autocompleteMaxVisible": 5,
  "thinkingBudgets": {
    "minimal": 1024,
    "low": 4096,
    "medium": 10240,
    "high": 32768
  },
  "skills": ["~/.claude/skills", "../.codex/skills"],
  "prompts": ["~/.ragent/prompts"],
  "models": "~/.ragent/models.json"
}
```

### 5.3 File Structure Additions

New directories to support:

```
~/.ragent/
├── agent/
│   ├── sessions/       # Tree-based session files (JSONL)
│   ├── themes/         # Custom theme JSON files
│   ├── prompts/        # Prompt template Markdown files
│   ├── skills/         # Agent Skills (SKILL.md)
│   └── auth.json       # OAuth tokens
├── keybindings.json    # Custom keybindings
└── models.json         # Custom model definitions

./.ragent/
├── settings.json       # Project settings
├── themes/             # Project-specific themes
├── prompts/            # Project-specific templates
└── skills/             # Project-specific skills
```

---

## Part 6: Conclusion

### Summary of Missing Features

**High Priority (User-Facing, High Impact):**
1. Session tree/branching system
2. Context compaction (auto & manual)
3. Prompt templates
4. Mid-session model switching
5. Custom themes

**Medium Priority (Enhanced UX):**
6. Session export/share
7. Custom keybindings
8. Additional providers
9. Steering messages
10. Agent Skills (standard)

**Lower Priority (Advanced/Nice-to-Have):**
11. Session labels
12. Branch summarization
13. Non-interactive modes
14. Subscription authentication

### Competitive Position

After implementing these milestones, ragent would have **feature parity** with Pi on all non-TypeScript features, while maintaining its advantages:

- **Single binary** distribution (no Node.js runtime)
- **Better performance** (Rust vs TypeScript)
- **Built-in security** (not delegated to extensions)
- **Native code indexing** (tree-sitter compiled in)

### Next Steps

1. Review and prioritize milestones based on user feedback
2. Create detailed design documents for Milestone 1 (Session Tree)
3. Begin implementation with core infrastructure
4. Maintain backward compatibility during migration

---

## Appendix A: Pi Feature Reference

### Pi Commands Reference

| Command | Description | Status in ragent |
|---------|-------------|------------------|
| `/tree` | Navigate session tree | ❌ Missing |
| `/fork` | Create branch from message | ❌ Missing |
| `/clone` | Duplicate current branch | ❌ Missing |
| `/compact [prompt]` | Summarize context | ❌ Missing |
| `/name <name>` | Name session | ❌ Missing |
| `/export [file]` | Export to HTML | ❌ Missing |
| `/share` | Share to GitHub Gist | ❌ Missing |
| `/model` | Switch model | ❌ Missing |
| `/resume` | Resume session picker | ⚠️ Basic version |
| `/new` | New session | ✅ Implemented |
| `/skill:name` | Execute skill | ⚠️ Different system |
| `/reload` | Reload extensions | 🚫 N/A (TypeScript) |
| `/login` | OAuth login | ❌ Missing |
| `/logout` | Clear auth | ❌ Missing |
| `/settings` | Settings editor | ⚠️ Config file only |

### Pi Keybindings Reference

| Key | Action | Status in ragent |
|-----|--------|------------------|
| Ctrl+L | Model switcher | ❌ Missing |
| Ctrl+P | Cycle models | ❌ Missing |
| Ctrl+M | Scoped model selector | ❌ Missing |
| Double Esc | Configurable action | ❌ Missing |
| Shift+L | Label entry | ❌ Missing |
| Shift+T | Toggle timestamps | ❌ Missing |
| Ctrl+O | Cycle tree filters | ❌ Missing |

---

*End of Document*
