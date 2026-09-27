# Web source

- URL: https://forum.gitlab.com/t/recently-evaluating-gitlab-ci-is-my-setup-ideal/3863
- Title: Recently evaluating Gitlab-CI...is my setup ideal?
- Author(s): —
- Language: English
- Published (UTC): 2016-08-06T16:59:45.675+00:00
- Captured (UTC): 2026-09-24T14:13:53.614056612+00:00
- Relevance: Medium — partial query match


```text
In an August 7, 2016 GitLab forum thread, a user evaluating GitLab CI for mostly Node projects asked whether running multiple GitLab Runners as Docker containers on one Amazon EC2 t2.medium—using a privileged Docker executor and mounting `/var/run/docker.sock` to build Docker images (possibly via DinD)—was sound, how to add another runner, and why build containers stopped but were not deleted. Respondent axil said `gitlab-ci-multi-runner` is deprecated and now symlinks to `gitlab-runner`, so they are the same; the same runner container can register multiple setups by changing the executor and registration token, without reinstalling; the Docker-container setup is acceptable, with a deb/rpm runner plus Docker executor as an alternative if desired. The original poster confirmed containers were stopped after builds but not deleted.
```
