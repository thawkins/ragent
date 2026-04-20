---
title: "AWS Access Key ID Format"
entity_type: "technology"
type: entity
generated: "2026-04-19T21:32:34.193085155+00:00"
---

# AWS Access Key ID Format

**Type:** technology

### From: sanitize

AWS Access Key IDs begin with the four-character prefix `AKIA` (Amazon Key ID AWS) followed by 16 uppercase alphanumeric characters, as captured by the pattern `AKIA[A-Z0-9]{16,}`. This format has been consistent since AWS's early years and serves as the public identifier portion of AWS's two-part credential system (paired with secret access keys that are 40-character base64 strings). The `AKIA` prefix specifically indicates long-term credentials associated with IAM users; other prefixes like `ASIA` (temporary credentials), `AGPA` (groups), and `AROA` (roles) exist but are less commonly hardcoded in applications. AWS's credential format design intentionally distinguishes between identifier and secret, allowing the Access Key ID to appear in logs and configuration files without immediate security risk—though the associated secret access key must be protected. However, the combination of Access Key ID and contextual information can sometimes enable credential stuffing attacks or facilitate social engineering. The pattern's strict uppercase requirement and fixed 20-character minimum (4 prefix + 16 body) provides high specificity with minimal false positive risk. AWS maintains extensive secret detection partnerships and automatically rotates credentials detected in public GitHub repositories.

## External Resources

- [AWS IAM identifier reference documentation](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_identifiers.html) - AWS IAM identifier reference documentation
- [AWS documentation on managing access keys](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html) - AWS documentation on managing access keys

## Sources

- [sanitize](../sources/sanitize.md)
