# Corpus Analysis Companion (CORPA.md)

Quality-assurance companion document for `Across the captured sources, support for Windows on GitLab's latest shared…`. Generated together with `RESEARCH.md`; the `[#N]` source indices reference the Sources Reference table at the bottom of this file.

## Contradiction Graph

_(no contradictions detected among the gathered sources)_

## Loci Analysis

| Locus | Sources | Mentions | Representative Snippets |
|-------|---------|----------|-------------------------|
| Cost | #12, #28, #38 | 3 | r FreeBSD and can reduce infrastructure cost; SSH is least supported and runs builds; able behavior, job failures, and h |
| Performance | #19, #38, #48 | 3 | dvanced features including autoscaling, performance optimization, fleet management, and Pro; ; fixed Kubernetes offers b |
| Benefit | #13 | 1 | rs mode, and wants clarification on the benefits of `docker-windows`, possibly by openi |
| Reliability | #27 | 1 | mproving Command Palette navigation and reliability, Window Hopper functionality for switch |
| Risk | #35 | 1 | ia the parent repo’s reflog. Executor risks include Shell (high-risk, runs as Runn |

## Depth Investigation

| Locus | Depth | Sources | Note |
|-------|-------|---------|------|
| Cost | moderate | #12, #28, #38 | Detected in 3 sources (depth: moderate). |
| Performance | moderate | #19, #38, #48 | Detected in 3 sources (depth: moderate). |
| Benefit | surface | #13 | Detected in 1 source (depth: surface). |
| Reliability | surface | #27 | Detected in 1 source (depth: surface). |
| Risk | surface | #35 | Detected in 1 source (depth: surface). |

## Source Tensions

| Kind | Label | Sources | Note |
|------|-------|---------|------|
| shallow evidence | Benefit | #13 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Cost | #12, #28, #38 | moderate evidence: only 3 source(s) mention this dimension. |
| shallow evidence | Performance | #19, #38, #48 | moderate evidence: only 3 source(s) mention this dimension. |
| shallow evidence | Reliability | #27 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Risk | #35 | surface evidence: only 1 source(s) mention this dimension. |
| isolated source | Benefit | #13 | Source #13 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Cost | #12 | Source #12 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Cost | #28 | Source #28 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Performance | #19 | Source #19 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Performance | #48 | Source #48 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Reliability | #27 | Source #27 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Risk | #35 | Source #35 only supports one dimension and may represent an outlier or niche view. |

## Synthesis Audit

**Overall score:** 95/100

**Recommendation:** Proceed — the synthesis passes the deterministic 4-critic audit.

Synthesis audit for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners' scored 95/100 across critics [coverage=80 logic=100 evidence=100 readability=100]; 32/49 sources cited.

| Critic | Score | Status | Issue / Gap Summary |
|--------|-------|--------|---------------------|
| coverage | 80 | pass | Dimension 'Benefit' is not addressed in the synthesis findings or implications |
| logic | 100 | pass | No contradictions detected; no logic conflicts to resolve. |
| evidence | 100 | pass | none |
| readability | 100 | pass | Finding 1 contains a paragraph longer than 1200 characters |

## Corpus Critic

**Overall score:** 58/100 (review)

**Subscores:** coverage 50 | evidence 24 | balance 89 | tension 100

**Issues:**
- Dimension 'Cost' has only moderate support (3 source(s))
- Dimension 'Performance' has only moderate support (3 source(s))
- Dimension 'Benefit' has only surface-level support (1 source(s))
- Dimension 'Reliability' has only surface-level support (1 source(s))
- Dimension 'Risk' has only surface-level support (1 source(s))
- 7 source(s) only support a single dimension and may be outliers

**Evidence gaps:**
- Find additional evidence on 'Cost' for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners'
- Find additional evidence on 'Performance' for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners'
- Find additional evidence on 'Benefit' for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners'
- Find additional evidence on 'Reliability' for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners'
- Find additional evidence on 'Risk' for 'research all the stratergies for supporting windows shared runners on Gitlab latest. environment need to support build environs that differ between teams, review all literature that describes how to support windows native containers on shared CI/CD gitlab runners'

**Recommendations:**
- Broaden the width sweep to capture more sources for each dimension.

**Shallow dimensions:** Cost, Performance, Benefit, Reliability, Risk

**Isolated sources:** #28, #13, #35, #27, #12, #19, #48

## Sources Reference

| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|
| 1 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Wine_(software)` | Wine (software) | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:44.960131012+00:00 |
| 2 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/development/add-windows-version` | Add Docker executor support for a Windows version \| GitLab Docs | — | — | High — title matches query | mf_search | serper | 2026-09-24T14:10:25.586484786+00:00 |
| 3 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Google_Chrome` | Google Chrome | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:46.667421192+00:00 |
| 4 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/List_of_TCP_and_UDP_port_numbers` | List of TCP and UDP port numbers | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:50.553446688+00:00 |
| 5 | web | page | English | `hxxps://docs[.]gitlab[.]com/ci/runners/hosted_runners/windows` | Hosted runners on Windows \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:10:14.460450597+00:00 |
| 6 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Adobe_Flash` | Adobe Flash | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:54.281127010+00:00 |
| 7 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/List_of_unit_testing_frameworks` | List of unit testing frameworks | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:56.367799254+00:00 |
| 8 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Vertica` | Vertica | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:09:58.720645138+00:00 |
| 9 | web | page | English | `hxxps://aixxe[.]net/2024/04/windows-ci-docker` | Windows containers with GitLab CI | [aixxe] | — | High — title matches query | mf_search | serper | 2026-09-24T14:10:38.764099433+00:00 |
| 10 | web | page | English | `hxxps://notes[.]runtimeterror[.]dev/CICD/Set-up-Gitlab-Runner-for-Windows-containers[.]html` | Set up GitLab Runner for Windows containers | — | — | High — title matches query | mf_search | serper | 2026-09-24T14:10:01.715873904+00:00 |
| 11 | web | page | English | `hxxps://archives[.]docs[.]gitlab[.]com/17[.]2/runner/install/windows[.]html` | Install GitLab Runner on Windows \| GitLab | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:10:32.323560357+00:00 |
| 12 | web | page | English | `hxxps://github[.]com/ayufan/gitlab-ci-multi-runner/tree/master/docs/executors` | gitlab-ci-multi-runner/docs/executors at master · ayufan/gitlab-ci-multi-runner | — | — | High — title + snippet match query | mf_search | langsearch | 2026-09-24T14:10:18.132368344+00:00 |
| 13 | web | page | English | `hxxps://forum[.]gitlab[.]com/t/gitlab-runner-on-windows-with-windows-containers-docker-vs-docker-windows-executor/115489` | GitLab Runner on Windows with Windows containers: docker vs docker-windows executor | — | — | High — title + snippet match query | mf_search | serper | 2026-09-24T14:10:45.806567943+00:00 |
| 14 | web | page | English | `hxxps://gitlab[.]musictribe[.]com/help/user/gitlab_com/index[.]md` | Index · Gitlab com · User · Help | — | — | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:11:04.526084542+00:00 |
| 15 | web | page | English | `hxxps://git[.]cs[.]hofstra[.]edu/help/user/gitlab_com/index[.]md` | Index · Gitlab com · User · Help | — | — | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:11:19.580735293+00:00 |
| 16 | web | page | English | `hxxps://github[.]com/oneuptime/blog/tree/master/posts/2026-01-07-ubuntu-gitlab-runner` | blog/posts/2026-01-07-ubuntu-gitlab-runner at master · OneUptime/blog | — | — | Medium — multiple title terms match query | mf_search | langsearch | 2026-09-24T14:10:56.677539332+00:00 |
| 17 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/configuration/advanced-configuration` | Advanced configuration \| GitLab Docs | — | — | Medium — partial query match | mf_search | serper | 2026-09-24T14:11:42.891392913+00:00 |
| 18 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/install/docker` | Run GitLab Runner in a container \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:11:28.049582411+00:00 |
| 19 | web | page | English | `hxxps://docs[.]gitlab[.]com/18[.]7/user/get_started/get_started_runner` | Get started with GitLab Runner \| GitLab Docs | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:10:48.224419273+00:00 |
| 20 | web | page | English | `hxxps://docs[.]gitlab[.]com/user/get_started/get_started_runner` | Get started with GitLab Runner \| GitLab Docs | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:11:16.647441597+00:00 |
| 21 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/GitHub` | GitHub | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:10:54.166322127+00:00 |
| 22 | web | page | English | `hxxps://docs[.]gitlab[.]com/ci/runners` | Runners \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:11:38.422975432+00:00 |
| 23 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Unity_(game_engine)` | Unity (game engine) | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:11:12.939714454+00:00 |
| 24 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/List_of_commercial_video_games_with_later_released_source_code` | List of commercial video games with later released source code | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:11:36.861321966+00:00 |
| 25 | web | page | English | `hxxps://forum[.]gitlab[.]com/t/how-does-a-shared-runner-work/103603` | How does a shared runner work? | — | 2024-04-27 | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:11:51.727751118+00:00 |
| 26 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/List_of_commercial_video_games_with_available_source_code` | List of commercial video games with available source code | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:11:49.119541374+00:00 |
| 27 | web | page | English | `hxxps://www[.]warp2search[.]net/story/powertoys-preview-v010126520-ships-with-command-palette-and-window-hopper-fixes` | PowerToys Preview v0.101.2652.0 Ships with Command Palette and Window Hopper Fixes | [Xaren Lysander Valtor] | 2026-09-23 | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:12:28.460317752+00:00 |
| 28 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/executors/docker_autoscaler` | Docker Autoscaler executor \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:12:01.290156875+00:00 |
| 29 | web | page | English | `hxxps://archives[.]docs[.]gitlab[.]com/17[.]0/runner/executors/docker_autoscaler[.]html` | Docker Autoscaler executor \| GitLab | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:11:56.507376199+00:00 |
| 30 | web | page | English | `hxxps://archives[.]docs[.]gitlab[.]com/17[.]2/runner/executors/docker_autoscaler[.]html` | Docker Autoscaler executor \| GitLab | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:12:08.215439411+00:00 |
| 31 | web | page | English | `hxxps://docs[.]gitlab[.]com/18[.]0/runner/executors/docker_autoscaler` | Docker Autoscaler executor \| GitLab Docs | — | — | High — title + snippet match query | mf_search | langsearch | 2026-09-24T14:12:15.955625428+00:00 |
| 32 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/runner_autoscale/gitlab-runner-autoscaler` | GitLab Runner instance group autoscaler \| GitLab Docs | — | — | High — title matches query | mf_search | serper | 2026-09-24T14:12:31.247119935+00:00 |
| 33 | web | page | English | `hxxps://forum[.]gitlab[.]com/t/google-cloud-autoscale-windows-runner-with-gitlab/39407` | Google Cloud: AutoScale Windows runner with GitLab | — | 2020-06-30 | High — title matches query | mf_search | langsearch | 2026-09-24T14:12:12.563818302+00:00 |
| 34 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/executors/docker_autoscaler[.]html` | Docker Autoscaler executor \| GitLab Docs | — | — | High — title + snippet match query | mf_search | langsearch | 2026-09-24T14:12:23.198664793+00:00 |
| 35 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/security` | Security for self-managed runners \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:12:36.744956354+00:00 |
| 36 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/executors` | Executors \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:12:46.129654441+00:00 |
| 37 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner/executors/shell` | The Shell executor \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:13:01.011529730+00:00 |
| 38 | web | page | English | `hxxps://dev[.]to/zenika/gitlab-runners-which-topology-for-fastest-job-execution-5bma` | 🦊 GitLab Runners: Which Topology for Fastest Job Execution? | [@] | 2026-01-30 | High — title matches query | mf_search | serper | 2026-09-24T14:13:07.931736866+00:00 |
| 39 | web | page | English | `hxxps://archives[.]docs[.]gitlab[.]com/17[.]2/runner/executors/shell[.]html` | The Shell executor \| GitLab | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:13:33.521781558+00:00 |
| 40 | web | page | English | `hxxps://archives[.]docs[.]gitlab[.]com/16[.]1/runner/executors/shell[.]html` | The Shell executor \| GitLab | — | — | High — title matches query | mf_search | langsearch | 2026-09-24T14:13:22.888893387+00:00 |
| 41 | web | page | English | `hxxps://github[.]com/libguestfs/nbdkit/tree/380c559696b88349200e37e7e9a10107c84e4abc/ci` | nbdkit/ci at 380c559696b88349200e37e7e9a10107c84e4abc · libguestfs/nbdkit | — | — | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:13:18.833382351+00:00 |
| 42 | web | page | English | `hxxps://github[.]com/libguestfs/nbdkit/tree/1965e89b5fc2d815ca238adf14742e87261d462e/ci` | nbdkit/ci at 1965e89b5fc2d815ca238adf14742e87261d462e · libguestfs/nbdkit | — | — | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:13:27.513060762+00:00 |
| 43 | web | page | English | `hxxps://docs[.]gitlab[.]com/ci` | Get started with GitLab CI/CD \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:13:13.756583172+00:00 |
| 44 | web | page | English | `hxxps://forum[.]gitlab[.]com/t/recently-evaluating-gitlab-ci-is-my-setup-ideal/3863` | Recently evaluating Gitlab-CI...is my setup ideal? | — | 2016-08-06 | Medium — partial query match | mf_search | langsearch | 2026-09-24T14:13:53.614056612+00:00 |
| 45 | web | page | English | `hxxps://docs[.]gitlab[.]com/runner` | GitLab Runner \| GitLab Docs | — | — | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:13:47.470317794+00:00 |
| 46 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/Yale_University` | Yale University | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:13:43.820306453+00:00 |
| 47 | web | page | English | `hxxps://en[.]wikipedia[.]org/wiki/List_of_security_hacking_incidents` | List of security hacking incidents | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-24T14:13:45.264087489+00:00 |
| 48 | web | page | English | `hxxps://forum[.]gitlab[.]com/t/where-should-you-install-gitlab-runner/99147` | Where should you install gitlab-runner? | — | 2024-01-31 | Medium — multiple title terms match query | mf_search | serper | 2026-09-24T14:13:58.137873272+00:00 |
| 49 | web | page | English | `hxxps://statusgator[.]com/services/gitlab/cicd---hosted-runners-on-windows` | GitLab CI/CD - Hosted runners on Windows Status. Check if GitLab CI/CD - Hosted runners on Windows is down or having… | [@statusgator] | 2016-12-22 | Medium — multiple title terms match query | mf_search | langsearch | 2026-09-24T14:14:01.838160963+00:00 |
