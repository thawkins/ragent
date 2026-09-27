# Web source

- URL: https://forum.gitlab.com/t/where-should-you-install-gitlab-runner/99147
- Title: Where should you install gitlab-runner?
- Author(s): —
- Language: English
- Published (UTC): 2024-01-31T03:12:40.037+00:00
- Captured (UTC): 2026-09-24T14:13:58.137873272+00:00
- Relevance: Medium — multiple title terms match query


```text
In a Jan 31, 2024 GitLab forum thread, user meemaw asked whether GitLab Runner should be installed on each environment server (e.g., vm1-test, vm2-prod) or on a separate machine. Advice given: for performance, GitLab recommends a dedicated VM separate from GitLab and application environments; one runner can serve multiple environments, though shell runners are limiting and generally not recommended. Runners communicate with GitLab via its HTTP API, so proper SSL and standard security practices matter; docker executors are preferred for flexibility (e.g., SSH/SCP deploy jobs), while shell runners may be used when Docker cannot run, such as Windows Server builds.
```
