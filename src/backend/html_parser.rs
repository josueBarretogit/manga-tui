/// Intended to represent html, not just a string
#[derive(Debug, Clone)]
pub struct HtmlElement(String);

pub mod scraper;

impl HtmlElement {
    pub fn new(raw_str: String) -> Self {
        Self(raw_str)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub trait HtmlParser {
    fn get_element_children(&self, element: HtmlElement) -> Vec<HtmlElement>;
    fn get_element_by_class(&self, document: &HtmlElement, class: &str) -> HtmlElement;
    fn get_element_by_id(&self, document: &HtmlElement, id: &str) -> HtmlElement;
    fn get_element_attr(&self, element: HtmlElement, attr: &str) -> Option<&str>;
}
