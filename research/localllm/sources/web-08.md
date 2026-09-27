# Web source

- URL: https://github.com/ggml-org/llama.cpp/discussions/16578
- Title: Performance of llama.cpp on NVIDIA DGX Spark · ggml-org/llama.cpp · Discussion #16578
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-18T05:34:20.171228201+00:00
- Relevance: Medium — multiple title terms match query


```text
A GitHub discussion post compares llama.cpp inference performance of gpt-oss-20b (GGUF) on a MacBook Pro M4 Max versus an NVIDIA DGX Spark (priced ~$4,000 per NVIDIA's spec sheet, as of 16 Nov 2025). Using identical server settings (llama-server, ctx-size 0, jinja, -ub 2048 -b 2048), the MacBook achieved 117.32 tokens/sec while the DGX Spark—which required manual compilation with CUDA flags (GGML_CUDA=ON, GGML_BACKEND_DL=ON, etc.) and SSH port forwarding—reached 84.67 tokens/sec, and only ~72 tokens/sec when the browser ran directly on the Spark. The author computes a price ratio of 1.5 (6000/4000) versus a token throughput ratio of 1.38, and speculates that a headless Mac Studio M4 Max with 128GB RAM (~$5,000) would offer a better price-performance comparison, likely matching the MacBook Pro M4 Max's performance.
```
