# Introduction and Overview

## What is ragent?

ragent is an AI coding agent designed for the terminal, built entirely in Rust. Inspired by [OpenCode](https://github.com/anomalyco/opencode), ragent provides developers with a powerful tool for automating coding tasks through natural language interactions. It combines multi-provider LLM orchestration, a comprehensive tool system, and a terminal-based user interface to deliver a seamless coding experience.

Unlike traditional development tools, ragent operates as an intelligent assistant that understands your codebase context and can perform complex tasks ranging from code explanation and refactoring to bug detection and feature implementation.

## Main Features

ragent offers a rich set of features designed to enhance developer productivity:

### Multi-Provider LLM Support
- Native integration with Anthropic, OpenAI, GitHub Copilot, and Ollama
- Extensible provider trait system for adding custom LLM services
- Automatic model compatibility handling and switching

### Comprehensive Tool System
- **Built-in Tools (8)**: File operations (read/write/create/edit), bash execution, grep, glob, directory listing, and interactive questions
- **Extended Tools (15)**: Advanced editing (multiedit, patch), web integration (webfetch, websearch), planning (plan delegation), task management (todo), document processing (Office, PDF), and file deletion
- **Sub-Agent Tools (3)**: Background task management with new_task, cancel_task, and list_tasks

### Terminal User Interface
- Full-screen ratatui interface with intuitive navigation
- Provider setup dialogs and slash-command autocomplete
- Streaming chat with step-numbered tool calls and JSON visualization
- Agent cycling for different operational modes

### HTTP Server Architecture
- REST + SSE API for external frontend integration
- Persistent session management with SQLite storage
- Configurable permission system for secure operation

### Advanced Capabilities
- Customizable agent presets (coder, task, architect, ask, debug, code-review)
- Automatic loading of project guidelines from `AGENTS.md`
- Model Context Protocol (MCP) client support (in development)
- File snapshotting and rollback functionality
- Real-time event bus for UI updates
- Background agent spawning for parallel task execution
- Prompt optimization with structured frameworks

## Target Audience

ragent is designed for:

- **Professional Developers**: Seeking to automate repetitive coding tasks and accelerate development workflows
- **Technical Leads**: Who need tools to review codebases, assess architectural decisions, and coordinate team efforts
- **DevOps Engineers**: Looking to streamline deployment processes and infrastructure management through AI assistance
- **Students and Educators**: Interested in learning modern coding practices and exploring AI-assisted development
- **Open Source Contributors**: Who want to contribute to projects more efficiently with AI-powered insights

Whether you're building web applications, system software, or data processing pipelines, ragent adapts to your specific domain and workflow preferences.

## Key Benefits

### Enhanced Productivity
ragent eliminates the manual effort required for routine coding tasks, allowing developers to focus on high-value creative and strategic work. Its ability to understand complex codebases means less time spent on context switching and more time building features.

### Improved Code Quality
With built-in code review capabilities and architectural analysis tools, ragent helps identify potential issues before they become problems. Its systematic approach to code examination ensures consistent quality standards.

### Accelerated Learning
For newcomers to a codebase or technology stack, ragent serves as an intelligent tutor that can explain concepts, demonstrate best practices, and provide guided exploration of complex systems.

### Reduced Cognitive Load
By handling the mechanical aspects of coding, ragent frees mental resources for problem-solving and innovation. Developers can think at a higher level while the agent manages implementation details.

### Flexible Integration
Whether used interactively through the terminal UI or programmatically via the HTTP API, ragent integrates seamlessly into existing development workflows without imposing rigid constraints.

## Document Structure and Navigation

This documentation is organized to support both quick reference and comprehensive learning:

1. **Getting Started** - Installation, configuration, and basic usage
2. **Core Concepts** - Understanding agents, sessions, tools, and permissions
3. **Advanced Features** - Teams, background agents, prompt optimization
4. **API Reference** - Detailed documentation for HTTP endpoints and tool usage
5. **Guides and Tutorials** - Step-by-step examples for common scenarios
6. **Troubleshooting** - Solutions to common issues and error messages
7. **Architecture** - Deep dive into ragent's internal design and components

Each section builds upon previous concepts while remaining accessible as standalone reference material. Code examples throughout illustrate practical usage, and cross-references help you discover related topics efficiently.

For immediate assistance, the integrated `/help` command within ragent provides context-sensitive guidance tailored to your current session and configuration.