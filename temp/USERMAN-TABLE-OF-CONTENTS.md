# USERMAN.docx - Table of Contents and Structure

## Overview
This document defines the structure and comprehensive table of contents for the ragent User Manual (USERMAN.docx).

---

## Part 1: Getting Started

### Chapter 1: Introduction
1.1 What is ragent?
1.2 Key Features
    - Multi-provider LLM support
    - Comprehensive tool system
    - Terminal UI
    - HTTP server
    - Custom agents
    - Teams and collaboration
1.3 Target Audience
1.4 Getting Help and Support
1.5 Document Conventions

### Chapter 2: Installation
2.1 Prerequisites
    - Rust 1.85+ requirements
    - LLM provider setup (Anthropic, OpenAI, Copilot, Ollama)
2.2 Building from Source
2.3 Installing to PATH
2.4 Verification and First Launch

### Chapter 3: Quick Start
3.1 First Run Experience
3.2 Configuring Your First Provider
    - Interactive TUI setup
    - Environment variable setup
3.3 Running Your First Task
    - Interactive mode
    - One-shot mode
3.3 Common Initial Workflows
    - Code exploration
    - Simple edits
    - File analysis

---

## Part 2: Core Concepts

### Chapter 4: Understanding ragent
4.1 Agent Presets
    - Built-in agents (coder, task, architect, ask, debug, code-review)
    - Custom agent overview
4.2 Sessions and Context
    - Session lifecycle
    - Context window management
4.3 Providers and Models
    - Provider selection
    - Model compatibility
4.4 Tool System Overview
    - Tool categories
    - Tool execution flow

### Chapter 5: Configuration
5.1 Configuration File (ragent.json)
    - File locations and priority
    - JSON structure
    - Example configurations
5.2 Environment Variables
    - Provider-specific variables
    - Runtime configuration
5.3 Agent Settings
    - Default agent selection
    - Model preferences
5.4 Permissions System
    - Permission rules
    - File write protection
    - Shell command restrictions

### Chapter 6: The Terminal User Interface
6.1 Interface Overview
    - Panel layout
    - Navigation flow
6.2 Main Panels
    - Chat panel
    - Log panel
    - Team panel
6.3 Input Methods
    - Text input
    - Slash commands
    - Agent switching
6.4 Keyboard Shortcuts
    - Navigation
    - Input panel
    - Panel switching

---

## Part 3: Basic Usage

### Chapter 7: Working with Providers
7.1 Anthropic (Claude)
    - Setup and configuration
    - Model selection
7.2 OpenAI (GPT)
    - Setup and configuration
    - Model selection
7.3 GitHub Copilot
    - Auto-discovery
    - Manual token setup
7.4 Ollama (Local)
    - Local server setup
    - Model management
7.5 Generic OpenAI-compatible
    - Custom endpoint configuration
    - Port configuration

### Chapter 8: Using Built-in Tools
8.1 File Operations
    - File reading (read)
    - File writing (write)
    - File creation (create)
    - File editing (edit)
8.2 System Interaction
    - Bash execution (bash)
    - Directory listing (list)
    - File searching (grep, glob)
8.3 Interactive Tools
    - Question asking (question)
    - Interactive dialogs

### Chapter 9: Using Extended Tools
9.1 Advanced Editing
    - Multi-file editing (multiedit)
    - Unified diff patches (patch)
9.2 Web Integration
    - Web fetching (webfetch)
    - Web search (websearch)
9.3 Planning and Management
    - Plan delegation (plan_enter/plan_exit)
    - Todo management (todo_write/todo_read)
9.4 Document Processing
    - Office documents (office_read, office_write, office_info)
    - PDF handling (pdf_read, pdf_write)
9.5 File Management
    - File deletion (rm)
    - File snapshots

### Chapter 10: Sub-Agent Tools
10.1 Spawning Tasks
    - Background agents (new_task)
    - Synchronous vs asynchronous
10.2 Task Management
    - Listing tasks (list_tasks)
    - Canceling tasks (cancel_task)
10.3 Task Result Handling

### Chapter 11: Custom Agents
11.1 Agent Profile Format (.md)
    - Markdown structure
    - Frontmatter configuration
    - System prompt definition
11.2 OASF Agent Format (.json)
    - JSON schema
    - Complete structure
11.3 Template Variables
    - WORKING_DIR
    - FILE_TREE
    - AGENTS.md
    - DATE
11.4 Permission Rules for Agents
11.5 Agent Discovery Paths

### Chapter 12: Sessions
12.1 Session Lifecycle
12.2 Managing Sessions
    - Listing sessions
    - Resuming sessions
    - Deleting sessions
12.3 Session Export/Import
    - JSON format
    - Portability

---

## Part 4: Advanced Features

### Chapter 13: Teams
13.1 Team Concepts and Architecture
    - Team lead vs teammates
    - Shared task lists
    - Mailbox communication
13.2 Creating a Team
    - Blueprint-based creation
    - Manual team setup
13.3 Teammate Roles and Setup
13.4 Managing Shared Tasks
    - Task creation
    - Task claiming
    - Task completion
13.5 Team Communication
    - Direct messaging
    - Broadcast messages
13.6 Team Blueprints
    - Creating blueprints
    - Reusable templates
13.7 Best Practices
    - Team lifecycle management
    - Cleanup procedures

### Chapter 14: Background Agents
14.1 Spawning Background Tasks
14.2 Monitoring Background Tasks
    - Task status
    - Progress tracking
14.3 Task Result Handling
14.4 Task Configuration

### Chapter 15: HTTP Server
15.1 Starting the Server
    - Basic startup
    - Port configuration
15.2 Authentication
    - Bearer token setup
15.3 API Endpoints
    - REST endpoints
    - SSE streaming
15.4 Server Configuration

### Chapter 16: Prompt Optimization
16.1 Structured Frameworks
    - CO-STAR
    - CRISPE
    - Chain-of-Thought (CoT)
    - DRAW
    - RISE
    - VARI
    - Q*
    - O1-STYLE
    - Meta Prompting
16.2 Platform Adapters
    - OpenAI/GPT
    - Anthropic/Claude
    - Microsoft/Azure
16.3 Using the /opt Command
16.4 Creating Optimized Prompts
16.5 Prompt Examples

---

## Part 5: Reference

### Chapter 17: Provider Reference
17.1 Complete Provider List
17.2 Provider Configuration Reference
17.3 Model Compatibility Tables
17.4 Troubleshooting Providers

### Chapter 18: Tool Reference
18.1 Complete Tool List
18.2 Tool Arguments and Options
18.3 Tool Output Formats
18.4 Error Handling Reference

### Chapter 19: Configuration Reference
19.1 Full Config Schema
19.2 Permission Rules Reference
19.3 Agent Definition Reference
19.4 Environment Variable Reference

---

## Part 6: Troubleshooting

### Chapter 20: Common Issues
20.1 Provider Connection Problems
20.2 Model Selection Issues
20.3 Tool Execution Errors
20.4 Configuration Problems
20.5 Permission Denied Errors

### Chapter 21: Debugging
21.1 Logging Levels
21.2 Diagnostic Commands
21.3 Troubleshooting Steps
21.4 Common Error Messages

---

## Part 7: Appendix

### Appendix A: Keyboard Shortcuts
A.1 TUI Shortcuts
A.2 Input Panel Shortcuts
A.3 Navigation Shortcuts

### Appendix B: File Formats
B.1 Config File Format (ragent.json)
B.2 Session Export Format
B.3 Agent Definition Formats
B.4 Team Blueprints Format

### Appendix C: Glossary

### Appendix D: Changelog

---

## Document Structure Notes

### Logical Flow
The manual follows a progressive learning path:
1. **Getting Started** - Install and run immediately
2. **Core Concepts** - Understand how ragent works
3. **Basic Usage** - Everyday tasks and tools
4. **Advanced Features** - Teams, background agents, server mode
5. **Reference** - Detailed reference material
6. **Troubleshooting** - Problem-solving guide
7. **Appendix** - Quick lookup and reference

### Audience Considerations
- **Beginners**: Start with Part 1 and Part 2
- **Intermediate**: Focus on Part 3 and Part 4
- **Advanced**: Reference Part 5 and Part 6
- **Developers**: Appendix D and reference sections

### Cross-References
- Each chapter includes links to related sections
- "See Also" sections for related topics
- Code examples throughout with copy-paste ready snippets
