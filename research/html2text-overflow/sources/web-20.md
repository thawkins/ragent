# Web source

- URL: https://rtrentinsworld.com/home
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:21:42.861963040+00:00

```text
[Skip to content][1]

# [RTrentin's world][2]

## Secure Multi-Cloud Networking

Primary Menu
* [Home][3]
* [Videos][4]
* [Archive][5]
* [Dump Notes][6]
* [About Me][7]
Search Search for:

# Home
* ## [Visualizer][8]
  
  [[Visualizer]][9]
  
  If you work with Aviatrix Distributed Cloud Firewall, you know CoPilot is the official UI. There are things it doesn’t
  do, or doesn’t do well enough when you’re deep in a policy audit, trying to figure out why traffic is behaving the way
  it is, or explaining your segmentation posture to someone who didn’t design it.
  
  So I built something. It’s called DCF Visualizer, it lives at [https://dcf-visualizer.vercel.app/][10], and it’s
  completely unofficial. No Aviatrix affiliation, no support contract, just a tool I put together to fill the gaps I
  kept running into.
  
  ## What it does
  
  The core is a policy matrix: rows are source SmartGroups, columns are destinations. Each cell shows the policies that
  match that pair. You can click into a cell to view, create, edit, or delete policies without losing that context.
  
  There’s also a graph view. Instead of a flat circular layout, it places SmartGroups into trust zones based on group
  names and criteria: Internet at the top, DMZ, app tier, data tier. Policies are directed edges. If a connection goes
  upward (data tier to internet, say), it looks visually wrong. That’s on purpose. The layout surfaces posture issues
  without you having to hunt for them.
  
  Traffic simulator: type in a source/destination IP and see which policy would match, with optional FQDN, threat group,
  and geo overrides
  * Live controller import: pulls SmartGroups and policies from your Aviatrix controller via the 8.x API
  * Terraform HCL import and export (module style or raw resources)
  * Policy evaluator with 29 checks: shadowing, missing deny-all, overly permissive rules, L4/L7 interaction issues
  * AI integration: OpenAI, Anthropic, Google, Bedrock, Ollama, LM Studio
  
  One thing worth saying This is not an Aviatrix product. Not affiliated, not supported. It doesn’t replace CoPilot. I
  built it for myself and figured someone else running DCF might find it useful.
  
  ## References
  
  [https://dcf-visualizer.vercel.app][11]
  
  [June 1, 2026][12]
  [multi-cloud networking][13]
* ## [Packet Surfer: AI-Assisted PCAP Triage in Your Browser][14]
  
  [[Packet Surfer: AI-Assisted PCAP Triage in Your Browser]][15]
  
  If you have spent any time debugging a network issue from a packet capture, you know the workflow: open Wireshark,
  wait for the file to load, apply a filter, stare at 50,000 packets, apply another filter, look for the SYN-ACK timing,
  open a second terminal for tshark, paste the output into a document, write the summary manually. By the time you
  finish, you have spent more time on the report than on the actual analysis.
  
  I built Packet Surfer to fix that: Packet Surfer is a browser-first PCAP analysis tool. You upload a `.pcap` file, the
  app parses the packet metadata entirely in your browser, and then you can ask your own AI model to analyze what it
  finds. The result is a structured report with Wireshark filters, tshark commands, Mermaid diagrams, Zeek-format logs,
  and exportable findings.
  
  The app is located at: [https://packet-surfer.vercel.app/][16]
  
  **Packet Surfer is not trying to replace Wireshark. It is the layer between “I have a capture file” and “I know
  exactly where to look in Wireshark.”**
  
  Packet Surfer parses `.pcap` files in the browser, extracts structured metadata, generates Zeek-format logs from that
  metadata, and forwards only the redacted summary to whatever AI model the user configures. The raw capture never
  leaves the machine. Once a flow is submitted, the summary page quickly presents a list of key observations.
  
  Flows are shown in the Flows tab. They can either be left ungrouped or organized into groups. When grouping is
  enabled, flows can be organized by server and by TCP handshake:
  
  You can generate multiple types of diagrams—such as flow diagrams, top talker views, and multi‑server flow
  visualizations—from the Diagrams page.
  
  For AI analysis, choose the model you want to use and enter your API key. Your key is stored only for your current
  session. Click “Apply to Session” to configure it.
  
  You can review the redaction, regex, payload preview, and Zeek log preview options before submitting the capture for
  analysis. Packet Surfer streams the report in real time while working on it:
  
  Once the report is ready, it can be viewed and exported in multiple formats from the Report menu.
  
  ## References
  
  [https://packet-surfer.vercel.app][17]
  
  [https://github.com/rtrentinavx/packet-surfer][18]
  
  [April 26, 2026][19]
  [multi-cloud networking][20]
  [captures][21], [cloud networking][22], [pcap][23], [troubleshoot][24], [wireshark][25]
* ## [RAG on Azure: Self-Hosted vs Managed Stack][26]
  
  [[RAG on Azure: Self-Hosted vs Managed Stack]][27]
  
  ## What is RAG
  
  ### The Problem RAG Solves
  
  Large Language Models (LLM) learn from a massive but **frozen** snapshot of the world. Once training ends, the model’s
  knowledge is sealed. It cannot read your internal documentation, does not know what changed last quarter, and has
  never seen your proprietary data.
  
  The result: when you ask an LLM about anything outside its training data, it **fabricates a plausible-sounding
  answer**. This is called hallucination — and it is not a bug that will be fixed. It is a fundamental property of how
  language models work.
  
  Three strategies exist to close this gap:
  
  ───────────────────┬──────────────────────────────────────────────┬───────────────────────────────────────────────────
  Strategy           │How it works                                  │Problem                                            
  ───────────────────┼──────────────────────────────────────────────┼───────────────────────────────────────────────────
  **Fine-tuning**    │Retrain the model on your data                │Expensive, slow, knowledge freezes again           
                     │                                              │immediately                                        
  ───────────────────┼──────────────────────────────────────────────┼───────────────────────────────────────────────────
  **Prompt           │Paste the whole document into the prompt      │Only works if data fits in the context window      
  injection**        │                                              │                                                   
  ───────────────────┼──────────────────────────────────────────────┼───────────────────────────────────────────────────
  **RAG**            │Retrieve only the relevant pieces at query    │Adds infrastructure, but scales to any corpus size 
                     │time                                          │                                                   
  ───────────────────┴──────────────────────────────────────────────┴───────────────────────────────────────────────────
  
  RAG was introduced by Meta AI researchers in 2020 (Lewis et al., *“Retrieval-Augmented Generation for
  Knowledge-Intensive NLP Tasks”*).
  
  ### How RAG Works
  
  RAG combines two systems: a **retriever** that finds relevant information, and a **generator** (the LLM) that
  formulates an answer using that information. Neither system works well alone — the retriever cannot answer questions,
  and the LLM without retrieval will hallucinate.
  
  There are exactly **two phases**:
  * **Ingestion (offline):** Documents are split into chunks, each chunk is converted into a vector (a list of numbers
    that captures its meaning), and stored in a vector database. This runs once — or on a schedule when documents
    change.
  * **Retrieval (online):** When a user asks a question, the question is also converted into a vector. The vector
    database finds the chunks whose vectors are most similar — these are the chunks most likely to contain the answer.
    They are injected into the LLM prompt alongside the original question.
  
  ### The Three Core Components
  
  **Embedding model** — Transforms text into a high-dimensional vector where semantically similar texts end up
  geometrically close. “car” and “automobile” will be near each other. “car” and “quarterly earnings” will be far apart.
  This is how the retriever finds relevant chunks without keyword matching.
  
  **Vector store** — A specialized database optimized for similarity search. Unlike a SQL database that matches exact
  values, a vector store finds the *k* vectors most similar to a query vector using Approximate Nearest Neighbor (ANN)
  algorithms. It also stores the original text so retrieved chunks can be read by the LLM.
  
  **LLM (generator)** — Takes the user’s question plus the retrieved chunks and produces an answer. The critical
  difference from a standard LLM call: the model is explicitly instructed to answer *only from the provided context*. If
  the answer is not in the chunks, it should say so — not invent one.
  
  ### What RAG Is Not
  
  ────────────────────┬─────────────────────────────────────────────────────────────────────────────────────────────────
  Misconception       │Reality                                                                                          
  ────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────────────
  “RAG eliminates     │RAG *reduces* hallucination. If the wrong chunks are retrieved, the LLM still hallucinates — from
  hallucination”      │bad context instead of no context.                                                               
  ────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────────────
  “RAG replaces       │They solve different problems. RAG = access to external facts. Fine-tuning = changing model      
  fine-tuning”        │behavior and style.                                                                              
  ────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────────────
  “Better embedding   │Retrieval quality matters most, but chunking strategy and data quality have more impact than     
  model = better RAG” │switching from one good embedding model to another.                                              
  ────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────────────
  “Vector search finds│Vector search finds the most *semantically similar* chunks — not necessarily the                 
  the right answer”   │most *correct* ones. A chunk about a related but wrong topic can score highly.                   
  ────────────────────┼─────────────────────────────────────────────────────────────────────────────────────────────────
  “RAG works out of   │A basic pipeline is easy to stand up. Production quality requires tuning chunk size, overlap,    
  the box”            │top-k, the prompt, and the embedding model — and measuring all of it with an eval dataset.       
  ────────────────────┴─────────────────────────────────────────────────────────────────────────────────────────────────
  
  The #1 failure mode in production RAG is not the LLM — it’s the retriever silently returning irrelevant chunks. The
  LLM then confidently answers from wrong context. Always instrument retrieval separately from generation so you can
  tell them apart when quality degrades.
  
  ## Where Can RAG Run?
  
  RAG is not tied to any specific platform. The three components — embedding model, vector store, LLM — can run anywhere
  that can execute Python and make HTTP calls. The platform choice is an infrastructure decision, not an ML decision.
  
  ### Deployment Options Compared
  
  ───────────────────┬────────────────────────────────────────────────────┬───────────────────────┬─────────────────────
  Platform           │Best for                                            │Components you manage  │Components Azure     
                     │                                                    │                       │manages              
  ───────────────────┼────────────────────────────────────────────────────┼───────────────────────┼─────────────────────
  **Virtual Machine**│Simplest self-hosted setup, prototyping             │OS, runtime, all RAG   │Nothing              
                     │                                                    │components             │                     
  ───────────────────┼────────────────────────────────────────────────────┼───────────────────────┼─────────────────────
  **AKS              │Production self-hosted, GPU workloads,              │Pod specs, scaling     │Control plane        
  (Kubernetes)**     │scale-to-zero, existing K8s investment              │rules, storage         │                     
  ───────────────────┼────────────────────────────────────────────────────┼───────────────────────┼─────────────────────
  **Azure Container  │Self-hosted without K8s ops overhead, event-driven  │Container images,      │Orchestration, OS,   
  Apps**             │scaling                                             │scaling rules          │networking           
  ───────────────────┼────────────────────────────────────────────────────┼───────────────────────┼─────────────────────
  **Azure ML / AI    │Data science teams, experiment tracking, model      │Pipeline definitions   │Compute, model       
  Foundry**          │registry integration                                │                       │serving, MLflow      
  ───────────────────┼────────────────────────────────────────────────────┼───────────────────────┼─────────────────────
  **Azure OpenAI + AI│Fully managed, no infra, fastest to production      │Application code only  │Everything           
  Search**           │                                                    │                       │                     
  ───────────────────┴────────────────────────────────────────────────────┴───────────────────────┴─────────────────────
  
  ### Platform Decision Tree
  
  ## Questions to Ask Before Building Anything
  
  Before writing a single line of code, run a discovery session with stakeholders. These questions determine whether you
  need RAG at all, what stack fits, and where the hard constraints live. Skipping this step is the most common reason
  RAG projects are rebuilt from scratch after three months.
  
  ### Use Case & Users
  
  Goal: understand what the system must do and who will use it.
  
  ─┬─────────────────────────────────────────────────────────┬──────────────────────────────────────────────────────────
  #│Question                                                 │Why it matters                                            
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  1│What question types will users ask — factual lookup,     │Each type has different retrieval and prompting           
   │summarization, comparison, or multi-step reasoning?      │requirements                                              
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  2│Who are the users — internal employees, external         │Drives auth model, SLA, and acceptable error rate         
   │customers, automated systems?                            │                                                          
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  3│How many concurrent users do you expect at peak?         │Determines replica count and scaling strategy             
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  4│What happens if the system returns a wrong answer?       │Sets the quality bar — a wrong answer in a legal context  
   │                                                         │is not the same as in an internal FAQ                     
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  5│Do users need to see the source documents behind the     │If yes, citation support is a hard requirement — affects  
   │answer?                                                  │chunking and metadata schema                              
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  6│What is the acceptable response latency?                 │Under 2s feels real-time; 5–10s is acceptable for complex 
   │                                                         │queries; above that needs a streaming response            
  ─┼─────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────────
  7│Will the system replace a human process or augment it?   │If replacing, the quality bar is much higher — plan for an
   │                                                         │evaluation phase                                          
  ─┴─────────────────────────────────────────────────────────┴──────────────────────────────────────────────────────────
  
  Question 4 is the most important. If a wrong answer has legal, financial, or safety consequences, you need a
  human-in-the-loop review step and a confidence threshold — not just better retrieval.
  
  ### Data & Knowledge Base
  
  Goal: understand the corpus — its size, format, freshness, and quality.
  
  ──┬──────────────────────────────────────────────────────┬────────────────────────────────────────────────────────────
  # │Question                                              │Why it matters                                              
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  8 │What are the source systems? (SharePoint, Blob        │Determines the document loaders and connectors needed       
    │Storage, databases, web, APIs)                        │                                                            
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  9 │What formats are the documents? (PDF, Word, HTML,     │Scanned PDFs require OCR — Azure Document Intelligence, not 
    │Markdown, structured data)                            │a text splitter                                             
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  10│How large is the corpus today — in document count and │If < 128K tokens total, context stuffing may be simpler than
    │estimated tokens?                                     │RAG                                                         
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  11│How frequently does the content change? (static,      │Static → bulk ingestion; daily → scheduled `CronJob`;       
    │daily, real-time)                                     │real-time → Event Grid trigger                              
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  12│Who owns the source data and who has permission to    │Determines service identity and RBAC setup for the ingestion
    │read it?                                              │pipeline                                                    
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  13│Is there duplicate or conflicting content across      │Requires deduplication strategy — without it, contradictory 
    │documents?                                            │chunks confuse the LLM                                      
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  14│What languages are the documents in?                  │Multilingual corpora need a multilingual embedding model    
    │                                                      │(e.g. `multilingual-e5-large`)                              
  ──┼──────────────────────────────────────────────────────┼────────────────────────────────────────────────────────────
  15│How is the content structured — flat files,           │Drives chunking strategy selection                          
    │hierarchical sections, or mixed?                      │                                                            
  ──┴──────────────────────────────────────────────────────┴────────────────────────────────────────────────────────────
  
  Ask to see 10–20 sample documents before the discovery session ends. Written answers about “well-structured PDFs”
  often mean scanned images with inconsistent formatting. Eyes on the data beats any description.
  
  ### Security & Compliance
  
  Goal: identify hard constraints that eliminate options before any design work starts.
  
  ──┬─────────────────────────────────────────────────────────┬─────────────────────────────────────────────────────────
  # │Question                                                 │Why it matters                                           
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  16│Does the data contain PII, PHI, financial records, or    │Requires PII scrubbing before indexing and strict access 
    │trade secrets?                                           │control on the vector store                              
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  17│What compliance frameworks apply? (HIPAA, PCI-DSS, GDPR, │May mandate data residency, encryption requirements, and 
    │SOC 2, ISO 27001)                                        │audit logging                                            
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  18│Can data leave the Azure VNet?                           │If no → Azure OpenAI with private endpoints or fully     
    │                                                         │self-hosted; rules out external APIs                     
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  19│Does the organization have a Microsoft BAA (Business     │Required for HIPAA workloads on Azure OpenAI             
    │Associate Agreement) in place?                           │                                                         
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  20│Who is allowed to query what? (row-level, document-level,│Requires metadata-filtered retrieval — not all users     
    │or topic-level access control)                           │should see all chunks                                    
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  21│Is there a data retention policy that affects how long   │Drives index TTL and deletion pipeline design            
    │chunks can live in the index?                            │                                                         
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  22│Who can upload documents to the knowledge base?          │Open upload = RAG poisoning risk; must have an approval  
    │                                                         │workflow                                                 
  ──┴─────────────────────────────────────────────────────────┴─────────────────────────────────────────────────────────
  
  Question 18 is a binary gate. If the answer is “no, data cannot leave the VNet”, Azure OpenAI with Managed Private
  Endpoints is the minimum — and fully self-hosted on AKS may be required.
  
  Question 20 is frequently forgotten. A user asking “what is the salary band for a senior engineer?” should not receive
  chunks from an HR document they have no permission to view — even if those chunks are the most relevant.
  Document-level access control in the vector store is a hard requirement for multi-tenant or role-separated knowledge
  bases.
  
  ### Infrastructure & Operations
  
  Goal: understand the existing environment and the team’s capacity to operate new components.
  
  ──┬───────────────────────────────────────────────────────┬───────────────────────────────────────────────────────────
  # │Question                                               │Why it matters                                             
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  23│What Azure services are already in use? (AKS, AOAI, AI │Reuse existing investments — avoids provisioning what is   
    │Search, Blob)                                          │already available                                          
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  24│Is there an existing AKS cluster with GPU nodes?       │If yes (Lab 1), the self-hosted stack has zero additional  
    │                                                       │infrastructure cost to start                               
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  25│Does the team have MLOps or platform engineering       │No MLOps → Azure Managed stack; strong MLOps → self-hosted 
    │capacity?                                              │is viable                                                  
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  26│What is the deployment process? (GitOps, manual, CI/CD │Determines how ingestion jobs and app updates are shipped  
    │pipeline)                                              │                                                           
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  27│Is there an existing monitoring stack? (Prometheus,    │Avoid standing up duplicate observability infrastructure   
    │Grafana, Log Analytics)                                │                                                           
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  28│What is the on-call rotation? Who gets paged if the RAG│Self-hosted means your team owns the pager for Qdrant,     
    │pipeline fails at 2am?                                 │vLLM, and the embedding model                              
  ──┼───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────────
  29│What is the target environment — dev/test only, or     │Drives SLA requirements and whether a single-replica setup 
    │production from day one?                               │is acceptable                                              
  ──┴───────────────────────────────────────────────────────┴───────────────────────────────────────────────────────────
  
  Question 28 is the one that changes minds. Teams that choose self-hosted for cost reasons often switch to managed
  after the first 2am Qdrant OOM incident.
  
  ### Cost & Budget
  
  Goal: establish financial guardrails before stack selection.
  
  ──┬─────────────────────────────────────────────────┬─────────────────────────────────────────────────────────────────
  # │Question                                         │Why it matters                                                   
  ──┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────
  30│What is the monthly budget for this workload?    │Sets a ceiling — at some budgets, only one stack is viable       
  ──┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────
  31│Is there an existing Azure Consumption Commitment│If yes, Azure managed services contribute to the commitment;     
    │(MACC) that needs to be consumed?                │self-hosted compute partially does                               
  ──┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────
  32│Are there reserved instances or savings plans    │Existing reservations may make specific VM sizes nearly free     
    │already purchased?                               │                                                                 
  ──┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────
  33│Who pays — a central platform team or the product│Affects whether showback (tagging) or chargeback (billing split) 
    │team?                                            │is required                                                      
  ──┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────────
  34│What is the expected query growth over the next  │A system that starts at 1K queries/day but grows to 100K/day will
    │12 months?                                       │cross the self-hosted break-even point mid-year                  
  ──┴─────────────────────────────────────────────────┴─────────────────────────────────────────────────────────────────
  
  Model cost at three scenarios: current volume, 10× growth, and 50× growth. The stack choice that is cheapest today may
  not be cheapest at scale.
  
  ### Quality & Evaluation
  
  Goal: define what “good” looks like before building, so you have a way to know when you are done.
  
  ──┬─────────────────────────────────────────────────────────┬─────────────────────────────────────────────────────────
  # │Question                                                 │Why it matters                                           
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  35│Is there an existing set of questions with known correct │A golden eval dataset is the single most valuable asset  
    │answers?                                                 │in a RAG project                                         
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  36│Who will judge answer quality — domain experts, end      │Automated metrics (RAGAS) are fast but imperfect; expert 
    │users, or automated metrics?                             │review is slow but trustworthy                           
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  37│What is the acceptable hallucination rate? (answers not  │Must be quantified before go-live — “zero hallucinations”
    │grounded in retrieved documents)                         │is not a measurable target                               
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  38│Should the system refuse to answer when it does not know?│If yes, requires a confidence threshold or an explicit “I
    │                                                         │don’t know” fallback prompt                              
  ──┼─────────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
  39│Will the system be A/B tested?                           │If yes, plan for two stack configs from the start        
  ──┴─────────────────────────────────────────────────────────┴─────────────────────────────────────────────────────────
  
  If the answer to question 35 is “no, we don’t have any example Q&A pairs”, stop the discovery session and make
  building that dataset the first deliverable. Without a golden set, you cannot measure whether the RAG system

[Content truncated]
```
