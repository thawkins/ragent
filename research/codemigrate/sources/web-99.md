# Web source

- URL: https://aws.amazon.com/blogs/machine-learning/streamline-code-migration-using-amazon-nova-premier-with-an-agentic-workflow
- Title: Streamline code migration using Amazon Nova Premier with an agentic workflow | Amazon Web Services
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:37:45.248070232+00:00
- Relevance: Medium - multiple title terms match query


```text
An AWS Machine Learning Blog post describes using the Amazon Bedrock Converse API with Amazon Nova Premier in an agentic workflow—implemented with Strands Agents v1.1.0+ and a custom BedrockInference class using text prefilling/continuation—to migrate legacy C code to Java/Spring applications. The workflow uses specialized agents for code analysis, conversion, security assessment, validation, refine (up to five feedback iterations), integration, and SQL DBIO-to-MyBatis XML conversion; it categorizes files as Simple (0–300 lines), Medium (300–700), and Complex (>700). Prerequisites include AWS Bedrock access to Nova Premier, Python 3.10+ with Boto3/Strands, Java 11+ with Maven/Gradle, and Spring Framework 5.x/Spring Boot 2.x+. Validated results: Small files achieved 93% structural completeness and 100% framework compliance in 30–40 seconds; Medium files 81%/91% after feedback in ~7 minutes; Large files 62%/84% in ~21 minutes, supporting a hybrid approach where AI handles routine conversions and humans handle complex logic, integration, and architecture.
```
