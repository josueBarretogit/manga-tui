use std::error::Error;
use std::ops::Deref;

/// Intended to represent html, not just a string
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
    type ParseError: Error;
    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError>;
}
impl HtmlElement {
    #[inline]
    pub fn new<T: Into<String>>(raw_str: T) -> Self {
        let s: String = raw_str.into();
        Self(s)
    }
}

pub trait HtmlParser {
    fn get_element_children(&self, element: &HtmlElement) -> Vec<HtmlElement>;
    /// Get the first matching element for the given selector / class
    fn get_element(&self, class: &str) -> Option<HtmlElement>;
    fn get_matching_elements(&self, selector: &str) -> Vec<HtmlElement>;
    fn get_inner_html<'a>(&'a self, document: &'a HtmlElement) -> String;
    fn get_inner_text(&self, document: &HtmlElement) -> String;
    fn get_element_attr(&self, element: &HtmlElement, attr_to_find: &str) -> Option<String>;
}
