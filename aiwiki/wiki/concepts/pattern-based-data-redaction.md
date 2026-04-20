---
title: "Pattern-Based Data Redaction"
type: concept
generated: "2026-04-19T17:41:44.540029091+00:00"
---

# Pattern-Based Data Redaction

### From: get_env

Pattern-based data redaction is a data protection technique that identifies and masks sensitive information by matching against known patterns or keywords rather than analyzing data content semantically. This approach contrasts with more sophisticated methods like regular expression matching for specific formats (credit card numbers, Social Security numbers), machine learning classification of sensitivity, or format-preserving encryption. Pattern-based methods trade precision for simplicity and performance, making decisions based on structural cues like field names rather than data values themselves.

The implementation in GetEnvTool uses substring matching against uppercase-converted variable names, checking for keywords associated with credential storage across computing history. The `KEY` pattern catches `API_KEY`, `SECRET_KEY`, `ENCRYPTION_KEY`; `SECRET` matches `AWS_SECRET_ACCESS_KEY`, `CLIENT_SECRET`; `TOKEN` identifies `ACCESS_TOKEN`, `REFRESH_TOKEN`, `CSRF_TOKEN`; `PASSWORD` and `PASS` cover authentication credentials; and `CREDENTIAL` is a broad catch-all. This list evolved organically from common conventions in cloud provider documentation, open source software, and enterprise configuration management, rather than from a formal standard.

The limitations of pattern-based redaction are significant and inform when it is appropriate versus alternative approaches. False negatives occur when developers use unconventional naming like `MY_SPECIAL_VALUE` for an API key; false positives occur when legitimate non-sensitive variables contain matched substrings like `PASSED_TESTS` or `TOKENIZATION_STRATEGY`. GetEnvTool mitigates false positives partially through case sensitivity (requiring uppercase matches, so `password` in lowercase would not match `PASSWORD`), but this also creates evasion risks if attackers can influence variable naming. The technique is fundamentally heuristic rather than rigorous.

Despite limitations, pattern-based redaction remains widely deployed due to its minimal overhead and explainability. In logging systems, web application firewalls, and now AI agent tools, it provides a baseline protection that catches the majority of casual exposure risks. Modern systems often layer multiple techniques: pattern matching for immediate filtering, regular expressions for structured secrets, and ML-based classification for unstructured text. The irreversible nature of GetEnvTool's redaction (replacing values with `***REDACTED***` rather than encrypting) reflects a specific threat model where the consuming component should never see actual secrets, rather than a model where authorized reconstruction is needed.

## External Resources

- [Google Cloud DLP infoType reference showing pattern-based detection](https://cloud.google.com/dlp/docs/infotypes-reference) - Google Cloud DLP infoType reference showing pattern-based detection
- [Detect-secrets: open source tool for secrets detection using multiple techniques](https://github.com/Yelp/detect-secrets) - Detect-secrets: open source tool for secrets detection using multiple techniques
- [OWASP Logging Cheat Sheet with data sanitization guidance](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html) - OWASP Logging Cheat Sheet with data sanitization guidance

## Sources

- [get_env](../sources/get-env.md)
