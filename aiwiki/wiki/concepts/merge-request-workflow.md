---
title: "Merge Request Workflow"
type: concept
generated: "2026-04-19T18:02:44.915818885+00:00"
---

# Merge Request Workflow

### From: gitlab_mrs

A merge request (MR) workflow is a structured process for proposing, reviewing, and integrating code changes in Git-based version control systems, serving as a cornerstone of modern collaborative software development. The workflow begins when a developer creates a feature branch from the main codebase, implements changes, and pushes the branch to the remote repository. They then open a merge request proposing to integrate their changes into a target branch, typically main or master. This creation step includes providing a title, description, and potentially linking to issues or other MRs. The GitlabCreateMrTool in this implementation automates this step, with special handling for draft MRs (marked with "Draft:" prefix) that indicate work-in-progress status.

Once created, the MR enters a review phase where team members examine the proposed changes, leave comments on specific lines of code, and participate in discussion threads. The GitlabGetMrTool retrieves comprehensive MR information including these review notes, filtering out system-generated messages to focus on human feedback. Modern MR workflows integrate automated checks through CI/CD pipelines that run tests, linting, security scans, and build verification before human approval. The GitlabApproveMrTool implements the explicit approval step, which in many organizations is a required gate before merging, often configured with approval rules requiring specific reviewers or maintainer sign-offs.

The final stage occurs when all conditions are satisfied—required approvals obtained, all discussions resolved, and CI/CD pipelines passing. The GitlabMergeMrTool handles this culmination, supporting options like squash merging (combining all commits into one) and automatic source branch deletion. Throughout this workflow, the GitlabListMrsTool provides visibility into the pipeline of pending changes. This structured approach transforms raw git operations into a collaborative, quality-controlled process that scales from small teams to enterprise development with thousands of contributors. The workflow enforces code review practices, maintains audit trails of decisions, and provides rollback capabilities through the preserved branch history.

## Diagram

```mermaid
flowchart TD
    subgraph MR_Lifecycle["Merge Request Lifecycle"]
        direction TB
        start([Developer pushes branch])
        create["gitlab_create_mr
Title, description, draft status"]
        ci[CI/CD Pipeline
Tests & Security Scans]
        review[Code Review
Comments & Discussions]
        approve["gitlab_approve_mr
Required approvals"]
        ready{All checks pass?}
        merge["gitlab_merge_mr
Squash, delete branch"]
        end([Merged to target])
        
        start --> create
        create --> ci
        ci --> review
        review --> approve
        approve --> ready
        ready -->|Yes| merge
        ready -->|No| review
        merge --> end
    end
    
    subgraph Visibility["Ongoing Visibility"]
        list["gitlab_list_mrs
Filter by state, branch"]
        get["gitlab_get_mr
Details & notes"]
    end
    
    list -.->|Monitor| MR_Lifecycle
    get -.->|Inspect| MR_Lifecycle
```

## External Resources

- [GitLab merge request workflow documentation](https://docs.gitlab.com/ee/user/project/merge_requests/) - GitLab merge request workflow documentation
- [Martin Fowler's patterns for managing source code branches](https://martinfowler.com/articles/branching-patterns.html) - Martin Fowler's patterns for managing source code branches

## Sources

- [gitlab_mrs](../sources/gitlab-mrs.md)
