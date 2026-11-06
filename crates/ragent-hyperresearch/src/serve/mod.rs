//! HTTP serving surface and HTML renderer (FR-016, FR-026).
//!
//! Serves vault note pages, an index page, and a report page. The render path
//! performs no network fetch, and every interpolated field is HTML-escaped.

pub mod html;
pub mod server;
