use scraper::{Element, ElementRef, Selector, html, node};

use crate::backend::html_parser::{HtmlElement, HtmlParser};

#[derive(Debug)]
pub struct Scraper {
    document: html::Html,
}

impl Scraper {
    #[inline]
    pub fn new(document: HtmlElement) -> Self {
        let document = html::Html::parse_document(&document);
        Self { document }
    }
}

pub trait AsSelector {
    #![allow(clippy::wrong_self_convention)]
    fn as_selector(self) -> Selector;
}

impl AsSelector for &str {
    #[inline]
    fn as_selector(self) -> Selector {
        Selector::parse(self).unwrap()
    }
}

impl HtmlParser for Scraper {
    fn get_inner_text(&self, document: &super::HtmlElement) -> String {
        let el = html::Html::parse_fragment(document);

        for e in el.tree {
            if let node::Node::Text(element_re) = e {
                return element_re.trim().to_string();
            }
        }

        "".to_string()
    }

    #[inline]
    fn get_element_from(&self, from: &HtmlElement, class: &str) -> Option<HtmlElement> {
        html::Html::parse_fragment(&from)
            .select(&class.as_selector())
            .next()
            .map(|el| HtmlElement::new(el.html()))
    }

    #[inline]
    fn get_inner_html(&self, document: &super::HtmlElement) -> String {
        let el = html::Html::parse_fragment(document);
        el.select(&"*".as_selector()).skip(1).next().map(|el| el.inner_html()).unwrap_or_default()
    }

    #[inline]
    fn get_element(&self, class: &str) -> Option<super::HtmlElement> {
        self.document.select(&class.as_selector()).next().map(|el| HtmlElement::new(el.html()))
    }

    #[inline]
    fn get_element_attr(&self, element: &super::HtmlElement, attr_to_find: &str) -> Option<String> {
        let el = html::Html::parse_fragment(element);

        for e in el.select(&"*".as_selector()).skip(1) {
            return e.attr(attr_to_find).map(|atr| atr.to_string());
        }

        None
    }

    #[inline]
    fn get_element_children(&self, element: &super::HtmlElement) -> Vec<super::HtmlElement> {
        let doc = html::Html::parse_fragment(element);

        doc.select(&"*".as_selector()).skip(2).map(|el| HtmlElement::new(el.html())).collect()
    }

    #[inline]
    fn get_matching_elements(&self, selector: &str) -> Vec<HtmlElement> {
        self.document
            .select(&selector.as_selector())
            .map(|el| HtmlElement::new(el.html()))
            .collect()
    }

    fn get_matching_elements_from(&self, from: &HtmlElement, class: &str) -> Vec<HtmlElement> {
        html::Html::parse_fragment(&from)
            .select(&class.as_selector())
            .map(|el| HtmlElement::new(el.html()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::html_parser::HtmlElement;

    #[test]
    fn it_get_html_element() {
        let example = r#"
        <div>
            <h1> title </h1>
            <ul>
                <li>
                    <div class="container"> a text </div>
                </li>
            </ul>
        <div>
        "#;

        let expected = HtmlElement::new("<h1> title </h1>");
        let expected2 = HtmlElement::new(
            r#"<ul>
                <li>
                    <div class="container"> a text </div>
                </li>
            </ul>"#,
        );

        let expected_class = HtmlElement::new(r#"<div class="container"> a text </div>"#);

        let scraper = Scraper::new(HtmlElement::new(example));

        assert_eq!(expected, scraper.get_element("h1").unwrap());
        assert_eq!(expected2, scraper.get_element("ul").unwrap());
        assert_eq!(expected_class, scraper.get_element(".container").unwrap());
        assert_eq!(None, scraper.get_element("h2"));
    }

    #[test]
    fn it_gets_inner_text() {
        let example = r#"
        <div>
            <h1 id="to_match"> title </h1>
            <h1> some </h1>
            <h1> other </h1>
            <h1> title </h1>
            <h1> title </h1>
        <div>
        "#;

        let expected = "title";

        let scraper = Scraper::new(HtmlElement::new(example));

        assert_eq!(expected, scraper.get_inner_text(&scraper.get_element("#to_match").unwrap()));
    }

    #[test]
    fn it_gets_attribute_of_element() {
        let example = r#"
        <div>
            <article>
                <div>
                    <img src="https://cdn.readdetectiveconan.com/file/mangap/7710/10001000/10.jpeg" />
                <div>
            </article>
        </div>
        "#;

        let expected = "https://cdn.readdetectiveconan.com/file/mangap/7710/10001000/10.jpeg";

        let img = r#"<img src="https://cdn.readdetectiveconan.com/file/mangap/7710/10001000/10.jpeg" alt="some_alt"/>"#;

        let scraper = Scraper::new(HtmlElement::new(example));

        assert_eq!(expected, scraper.get_element_attr(&scraper.get_element("img").unwrap(), "src").unwrap())
    }

    #[test]
    fn it_gets_elements_children() {
        let example = r#"
        <div>
            <ul>
                <li> 1 </li>
                <li> 2 </li>
                <li> 3 </li>
                <li> 4 </li>
            </ul>
        </div>
        "#;

        let expected = vec![
            HtmlElement::new("<li> 1 </li>"),
            HtmlElement::new("<li> 2 </li>"),
            HtmlElement::new("<li> 3 </li>"),
            HtmlElement::new("<li> 4 </li>"),
        ];

        let scraper = Scraper::new(HtmlElement::new(example));

        assert_eq!(expected, scraper.get_element_children(&scraper.get_element("ul").unwrap()))
    }

    #[test]
    fn it_gets_inner_html() {
        let example = r#"
        <div><ul>
                <li> 1 </li>
                <li> 2 </li>
                <li> 3 </li>
                <li> 4 </li></ul>
        </div>
        "#;

        let expected = r#"<ul>
                <li> 1 </li>
                <li> 2 </li>
                <li> 3 </li>
                <li> 4 </li></ul>"#;

        let scraper = Scraper::new(HtmlElement::new(example));

        let result = dbg!(scraper.get_inner_html(&scraper.get_element("div").unwrap()));

        assert_eq!(expected, result.trim())
    }

    #[test]
    fn it_gets_elements_from_element() {
        let example = r#"
        <div><ul>
                <li> 

                    <div>
                        <h1> title </h1>
                        <h2> subtitle </h2>
                    </div>

                </li>
                <li> 2 </li>
                <li> 3 </li>
                <li> 4 </li></ul>
        </div>

        "#;

        let example2 = r#"
                    <div>
                        <a href="/chapters/8834-10007000/haikyo-no-meshi-chapter-7" class="relative block">
                            <figure class="w-full h-40 overflow-hidden bg-card rounded-md">
                                <img data-src="https://cdn.readdetectiveconan.com/file/mangapill/i/8834.jpeg"
                                    alt="Haikyo no Meshi Chapter 7"
                                    class="text-transparent lazy object-cover w-full h-full" />
                            </figure>
                        </a>
                        <div class="px-1">
                            <a href="/chapters/8834-10007000/haikyo-no-meshi-chapter-7">
                                <div class="mt-3 text-lg font-black leading-tight">#7</div>
                            </a>
                            <a href="/manga/8834/haikyo-no-meshi" class="mt-1.5 leading-tight text-secondary">
                                <div class="line-clamp-2 text-sm font-bold">Haikyo no Meshi</div>
                                <div class="line-clamp-2 text-xs mt-1">The Common Bread,Meals in the Ruins</div>
                            </a>
                            <div class="mt-1.5 text-xs text-secondary">
                                <time-ago datetime="2025-10-26T21:43:16Z">2025-10-26</time-ago>
                            </div>
                        </div>
                    </div>

        "#;

        let expected = HtmlElement::new(
            r#"<a class="mt-1.5 leading-tight text-secondary" href="/manga/8834/haikyo-no-meshi">
                                <div class="line-clamp-2 text-sm font-bold">Haikyo no Meshi</div>
                                <div class="line-clamp-2 text-xs mt-1">The Common Bread,Meals in the Ruins</div>
                            </a>"#,
        );

        let scraper = Scraper::new(HtmlElement::new(example2));

        let li = dbg!(scraper.get_element("div").unwrap());

        assert_eq!(expected, scraper.get_element_from(&li, ".px-1 > a:nth-of-type(2)").unwrap());
    }
}
