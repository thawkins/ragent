# Web source

- URL: https://github.com/joshpxyne/gpt-migrate?tab=readme-ov-file
- Title: GitHub - joshpxyne/gpt-migrate: Easily migrate your codebase from one framework or language to another.
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:26:10.261032093+00:00
- Relevance: Medium - multiple title terms match query


```text
GPT-Migrate is an alpha, not-production tool for migrating codebases between frameworks/languages; it requires Docker, recommends GPT-4/GPT-4-32k, installs via Poetry with `OPENROUTER_API_KEY` and/or `OPENAI_API_KEY`, and runs e.g. `python main.py --targetlang nodejs`. Its main options include `--model` (default `gpt-4-32k`), `--temperature` 0, `--sourcedir`/`--targetdir` defaults under `../benchmarks/flask-nodejs`, `--sourceentry app.py`, optional `--sourceport`, `--targetport` 8080, `--guidelines`, and `--step` setup/migrate/test/all. It creates a Docker environment for the target language, recursively identifies dependencies, rebuilds code from the source entrypoint, develops tests using Python `unittest`, optionally validates them against the original app via `--sourceport`, and iteratively debugs using logs/errors while requesting clearance for shell scripts. It uses composable subprompts organized in a p1–p4 hierarchy via `prompt_constructor()`; current benchmarks are REST API apps with a few endpoints, and it succeeds on easy Python/JavaScript ~50% of the time but cannot handle C++/Rust without human assistance.
```
