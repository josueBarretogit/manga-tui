use scraper::Selector;

//pub struct Parser;

pub trait AsSelector {
    #![allow(clippy::wrong_self_convention)]
    fn as_selector(self) -> Selector;
}

impl AsSelector for &str {
    fn as_selector(self) -> Selector {
        Selector::parse(self).unwrap()
    }
}

//impl HtmlParser for Parser {
//    fn get_element_by_id(&self, document: &HtmlElement, class: &str) -> HtmlElement {
//        let document = html::Html::parse_document(document.as_str());
//        let selector = selector::Selector::parse(class).unwrap();
//
//        let element = document.select(&selector).next().unwrap();
//
//        HtmlElement::new(element.html())
//    }
//
//    fn get_element_children(&self, element: HtmlElement) -> Vec<HtmlElement> {
//        vec![]
//    }
//
//    fn get_element_by_class(&self, document: &HtmlElement, class: &str) -> HtmlElement {}
//
//    fn get_attr(&self, element: HtmlElement, attr: &str) -> Option<&str> {}
//}
