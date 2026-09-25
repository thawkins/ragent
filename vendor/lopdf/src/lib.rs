#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![deny(clippy::all)]
// Vendored (pinned) third-party crate: upstream exposes many `pub` items and
// keeps unused helper code that ragent's stricter CI lints flag. These
// crate-level allowances scope the suppression to this vendor crate only; the
// same treatment is applied in vendor/pdf-extract.
#![allow(unreachable_pub)]
#![allow(dead_code)]
#![allow(unused_imports)]

pub mod content;
pub mod encryption;
pub mod filters;
pub mod xobject;
pub mod xref;

#[macro_use]
mod object;
mod document;
mod incremental_document;

mod bookmarks;
mod cmap_section;
mod common_data_structures;
mod creator;
mod datetime;
mod destinations;
mod encodings;
mod error;
mod outlines;
mod processor;
mod toc;
mod writer;

mod object_stream;
mod parser;
mod parser_aux;
mod reader;
mod save_options;

mod font;

pub use document::Document;
pub use object::{Dictionary, Object, ObjectId, Stream, StringFormat};

pub use bookmarks::Bookmark;
pub use common_data_structures::{decode_text_string, text_string};
pub use destinations::Destination;
pub use encodings::{Encoding, encode_utf8, encode_utf16_be};
pub use encryption::{EncryptionState, EncryptionVersion, Permissions};
pub use error::{Error, Result};
pub use incremental_document::IncrementalDocument;
pub use object_stream::{ObjectStream, ObjectStreamBuilder, ObjectStreamConfig};
pub use outlines::Outline;
pub use reader::Reader;
pub use save_options::{SaveOptions, SaveOptionsBuilder};
pub use toc::Toc;

pub use parser_aux::substr;
pub use parser_aux::substring;

pub use font::FontData;
