---
title: "Last-Match-Wins Evaluation"
type: concept
generated: "2026-04-19T15:24:51.864377053+00:00"
---

# Last-Match-Wins Evaluation

### From: mod

The last-match-wins evaluation strategy is a rule precedence algorithm where the final matching rule in an ordered sequence determines the outcome, superseding any earlier matches. This concept, also known as "specificity" or "cascading" in different domains, is implemented in the ragent-core permission system through the `PermissionChecker::check()` method. When evaluating a permission request, the checker iterates through the entire `ruleset` vector, updating the result variable whenever a match is found, ensuring that later rules override earlier ones. This approach offers significant practical advantages for permission management: administrators can establish broad baseline policies early in the ruleset (such as denying all file edits) and place specific exceptions afterward (such as allowing edits to `src/**` paths). The evaluation model mirrors CSS selector specificity and iptables firewall rules, making it intuitive for administrators familiar with these systems. The implementation uses a simple mutable variable (`result`) that accumulates the final decision through sequential iteration, avoiding complex data structures while maintaining clear semantics. This design choice prioritizes predictability and debuggability, as the effective policy can be determined by reading the ruleset from top to bottom and applying the final matching rule.

## External Resources

- [CSS Specificity and cascade documentation](https://developer.mozilla.org/en-US/docs/Web/CSS/Specificity) - CSS Specificity and cascade documentation
- [Firewall rule processing and ordering](https://en.wikipedia.org/wiki/Firewall_(computing)) - Firewall rule processing and ordering

## Sources

- [mod](../sources/mod.md)
