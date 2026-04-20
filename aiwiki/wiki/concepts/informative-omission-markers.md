---
title: "Informative Omission Markers"
type: concept
generated: "2026-04-19T17:03:16.929028392+00:00"
---

# Informative Omission Markers

### From: truncate

Informative omission markers serve as critical user interface elements in truncated content, transforming a simple ellipsis into a communicative element that conveys quantitative information about the hidden material. The truncate.rs implementation exemplifies this pattern through its carefully crafted marker format: `... (N lines omitted) ...`, which includes not only visual indication of interruption but specific data about the extent of truncation. The grammatical handling—producing "1 line omitted" versus "N lines omitted"—demonstrates attention to linguistic correctness that enhances perceived tool quality and user trust. This pattern emerges from recognition that users need situational awareness when content is suppressed: knowing whether 2 lines or 200 lines were omitted fundamentally changes how one interprets the visible content and decides whether to request complete output. The marker's symmetric placement with leading and trailing ellipsis creates visual rhythm that signals intentional formatting rather than data corruption or rendering errors. Similar patterns appear in database query result limitations, search engine result counts, and pagination interfaces across web and desktop applications. The implementation's use of the marker line within the `max_lines` budget—reserving one line for the marker itself—shows thoughtful integration of the notification into the display constraint rather than treating it as external metadata, ensuring predictable output sizing.

## External Resources

- [Nielsen Norman Group on progressive disclosure patterns in interface design](https://www.nngroup.com/articles/progressive-disclosure/) - Nielsen Norman Group on progressive disclosure patterns in interface design
- [Information foraging theory explaining user information-seeking behavior](https://en.wikipedia.org/wiki/Information_foraging) - Information foraging theory explaining user information-seeking behavior

## Sources

- [truncate](../sources/truncate.md)
