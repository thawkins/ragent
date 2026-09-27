# Web source

- URL: https://forum.gitlab.com/t/how-does-a-shared-runner-work/103603
- Title: How does a shared runner work?
- Author(s): —
- Language: English
- Published (UTC): 2024-04-27T06:07:20.502+00:00
- Captured (UTC): 2026-09-24T14:11:51.727751118+00:00
- Relevance: Medium — multiple title terms match query


```text
In this GitLab forum thread, users explain that shared runners can serve multiple projects, so the “Enable for this project” button lets you enable an already registered shared/group runner instead of creating a new runner per project; it runs jobs the same way but is easier to manage. Commenters warn that sharing runners can cause conflicts if jobs permanently set variables or install software, recommend using tags (or enabling untagged jobs if not using tags), and note that per-project runners are not wrong but excessive—e.g., 20 Python projects may need only one or two runners, while updating many runners is burdensome. They add that Docker-based runners reset filesystem/packages/logins per job, whereas shell-based runners retain SDKs and logins and may interfere across projects or parallel jobs; different OS/hardware requires different runners, and a custom Docker image stored in the local GitLab registry can preserve needed packages/logins.
```
