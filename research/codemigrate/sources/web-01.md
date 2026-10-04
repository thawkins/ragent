# Web source

- URL: https://arxiv.org/html/2602.09944v1
- Title: Environment-in-the-Loop: Rethinking Code Migration with LLM-based Agents
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:23:55.279229353+00:00
- Relevance: Medium - multiple title terms match query


```text
The ReCode ’26 paper (DOI 10.1145/3786180.3788315) argues that automated code migration is only half complete without automated environment interaction, because current workflows rely on static initial-environment analysis and miss version-dependent runtime errors such as NumPy 1.x vs. 2.x constraints, causing rework and delays; it cites Cheng et al. (2025) that LLMs perform poorly at predicting execution outcomes, causing nearly 30% of runtime errors. It proposes an LLM-based environment-driven multi-agent framework with Migration Agent (M-Agent), Environment Agent (E-Agent), and Testsuite Agent (T-Agent) that iterates through migration planning, automated environment setup, test validation, and feedback refinement integrated into CI/CD; related work includes ExecutionAgent configuring 33 of 50 repositories across Python, Java, C, C++, and JavaScript, EnvBench covering thousands of Python/JVM repositories, and industrial examples such as Amazon Q dev mode and Aviator Agent Runbooks launching Docker sandboxes. Remaining challenges include environment reproduction and dependency management, multi-agent coordination, CI/CD integration, and extension to Docker/Kubernetes.
```
