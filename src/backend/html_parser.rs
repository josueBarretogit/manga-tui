//! HTML parsing facade used across providers.
//!
//! This module defines:
//! - `HtmlElement`: a small wrapper for raw HTML snippets that signals intent ("this is HTML"), not an arbitrary `String`.
//! - `HtmlParser`: a trait that abstracts querying and extracting content from documents and fragments, implemented by
//!   backend-specific adapters (see `html_parser::scraper`).
//! - `ParseHtml`: a helper trait for types that can be constructed directly from an `HtmlElement`.
//!
//! The goal is to keep scraping code decoupled from any particular HTML
//! library. Providers operate only on this trait surface, which makes tests
//! fast and the implementation swappable.
use std::error::Error;
use std::ops::Deref;

/// Wrapper type that represents HTML, not just an arbitrary `String`.
///
/// This helps prevent accidental confusion between raw text and HTML
/// fragments/documents at the type level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlElement(String);

impl Deref for HtmlElement {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub mod scraper;

pub trait ParseHtml: Sized {
    /// Error type emitted during HTML parsing/construction.
    type ParseError: Error;
    /// Construct `Self` from an `HtmlElement`.
    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError>;
}
impl HtmlElement {
    #[inline]
    /// Creates a new `HtmlElement` from any `Into<String>` value.
    pub fn new<T: Into<String>>(raw_str: T) -> Self {
        let s: String = raw_str.into();
        Self(s)
    }
}

pub trait HtmlParser {
    /// Returns the direct children of `element` as separate `HtmlElement`s.
    fn get_element_children(&self, element: &HtmlElement) -> Vec<HtmlElement>;
    /// "Document-wide"
    /// Get the first matching element for the given selector / class
    fn get_element(&self, class: &str) -> Option<HtmlElement>;

    /// Like get_element but from an specific tag
    fn get_element_from(&self, from: &HtmlElement, class: &str) -> Option<HtmlElement>;

    /// Like get_matching_elements but from an specific tag
    fn get_matching_elements_from(&self, from: &HtmlElement, class: &str) -> Vec<HtmlElement>;

    /// "Document-wide"
    /// Returns all elements matching the provided CSS selector.
    fn get_matching_elements(&self, selector: &str) -> Vec<HtmlElement>;
    /// Returns the inner HTML string of the provided element.
    fn get_inner_html(&self, document: &HtmlElement) -> String;
    /// Returns the inner text of the provided element (implementation-defined
    /// granularity; usually the first text node trimmed).
    fn get_inner_text(&self, document: &HtmlElement) -> String;
    /// Returns the value of an attribute on the provided element, if present.
    fn get_element_attr(&self, element: &HtmlElement, attr_to_find: &str) -> Option<String>;
}
