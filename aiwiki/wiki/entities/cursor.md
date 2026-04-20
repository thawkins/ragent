---
title: "Cursor"
entity_type: "technology"
type: entity
generated: "2026-04-19T18:51:38.683189667+00:00"
---

# Cursor

**Type:** technology

### From: pdf_write

The Cursor struct is a private implementation detail that manages vertical positioning and pagination state during PDF generation. Named after the blinking cursor in text editors, this struct tracks the current Y coordinate in millimeters measured from the page bottom—following PDF's coordinate convention where (0,0) is the lower-left corner. The struct encapsulates three critical operations: initialization at the top margin, advancement after content rendering, and page break detection. Its design as a simple struct with a single `f32` field reflects Rust's zero-cost abstraction philosophy where high-level concepts incur no runtime overhead beyond the raw data they encapsulate.

The pagination algorithm implemented through Cursor methods represents a simplified version of document layout engines found in word processors and desktop publishing software. The `needs_new_page` method implements threshold-based pagination: when requested content height would place the bottom edge below the margin bottom, a page break is triggered. This greedy algorithm processes elements sequentially without look-ahead optimization, which may occasionally produce suboptimal breaks (e.g., avoiding single lines at page bottom) but ensures predictable, deterministic output. The `advance` method decrements Y position since PDF coordinates increase upward, a common source of confusion in graphics programming.

The Cursor's reset functionality enables multi-page document generation by repositioning to the top margin when `flush_page` completes a page. This stateful approach separates layout concerns from content generation, allowing the main rendering loop to focus on element processing while the Cursor handles spatial bookkeeping. The constants it references (PAGE_H, MARGIN_TOP, MARGIN_BOTTOM) define A4 paper with 25mm margins, a conservative layout suitable for professional documents and printing with binding allowance. This struct demonstrates how even simple state machines can implement sophisticated document layout when combined with careful coordinate arithmetic.

## Diagram

```mermaid
stateDiagram-v2
    [*] --> TopOfPage: Cursor::new()
    TopOfPage --> Advancing: advance(height)
    Advancing --> Advancing: render content
    Advancing --> PageBreak: needs_new_page() == true
    PageBreak --> TopOfPage: flush_page() / reset()
    TopOfPage --> [*]: document complete
```

## Sources

- [pdf_write](../sources/pdf-write.md)
