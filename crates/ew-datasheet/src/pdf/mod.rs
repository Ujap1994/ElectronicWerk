//! A PDF reader for text and lines, and a small writer for synthetic
//! datasheets.

pub mod content;
pub mod crypt;
pub mod document;
pub mod encoding;
pub mod filter;
pub mod font;
pub mod metrics;
pub mod object;
pub mod write;

pub use document::{Document, PdfError};
