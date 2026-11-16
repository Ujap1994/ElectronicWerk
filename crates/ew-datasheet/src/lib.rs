//! Datasheet reader (ADR-0017): reads the text of a PDF datasheet with its
//! positions, so that part data (pins, packages, dimensions) can be
//! recognised in it and linked back to its place on the page.
//!
//! The PDF reader is written for this purpose: it places characters and
//! straight lines, nothing is rendered. Coordinates are points in display
//! space (origin at the top left of the visible page, y down, the page
//! rotation applied), the space a viewer shows.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    reason = "PDF numbers are read leniently and limited by the file format"
)]

pub mod draft;
pub mod package;
pub mod part;
pub mod pdf;
pub mod recognize;
pub mod table;
pub mod text;

use serde::{Deserialize, Serialize};

pub use pdf::PdfError;
pub use text::{Line, Page, Rect, Word};

use pdf::content::{FontCache, interpret};
use pdf::document::Document;

/// A bookmark of the datasheet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    /// Title.
    pub title: String,
    /// Nesting level (0 at the top).
    pub level: usize,
    /// Page number (1-based), when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
}

/// A read datasheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Datasheet {
    /// Document title.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Author (often the manufacturer).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Subject (often the part description).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subject: String,
    /// Keywords.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keywords: String,
    /// The file was encrypted (opened with the empty password).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub encrypted: bool,
    /// Bookmarks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bookmarks: Vec<Bookmark>,
    /// Pages.
    pub pages: Vec<Page>,
}

/// Reads a datasheet from the bytes of a PDF file.
///
/// # Errors
///
/// Not a PDF file, no pages, or a file that needs a password.
pub fn read(data: Vec<u8>) -> Result<Datasheet, PdfError> {
    let doc = Document::open(data)?;
    let refs = doc.pages();
    let mut fonts = FontCache::default();
    let pages = refs
        .iter()
        .enumerate()
        .map(|(i, page)| text::page(i + 1, &interpret(&doc, page, &mut fonts)))
        .collect();
    let bookmarks = doc
        .bookmarks(&refs)
        .into_iter()
        .map(|b| Bookmark {
            title: b.title,
            level: b.level,
            page: b.page.map(|p| p + 1),
        })
        .collect();
    Ok(Datasheet {
        title: doc.info("Title"),
        author: doc.info("Author"),
        subject: doc.info("Subject"),
        keywords: doc.info("Keywords"),
        encrypted: doc.encrypted(),
        bookmarks,
        pages,
    })
}
