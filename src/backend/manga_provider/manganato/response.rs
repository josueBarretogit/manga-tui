use std::error::Error;

use scraper::{html, Selector};
use serde::{Deserialize, Serialize};

use super::FromHtml;
use crate::backend::html_parser::HtmlElement;
use crate::backend::manga_provider::{MangaStatus, PopularManga, RecentlyAddedManga};

#[derive(Debug, Default)]
pub(super) struct PopularMangaItem {
    pub(super) manga_page_url: String,
    pub(super) title: String,
    pub(super) cover_img_url: String,
}

impl FromHtml for PopularMangaItem {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn std::error::Error>> {
        let div = html::Html::parse_fragment(html.as_str());
        let a_selector = Selector::parse(".item div h3 a").unwrap();
        let img_selector = Selector::parse(".item img").unwrap();

        let a_tag = div.select(&a_selector).next().ok_or("Could not find div element containing manga info")?;
        let title = a_tag.attr("title").ok_or("Could not find manga title url")?;
        let manga_page_url = a_tag.attr("href").ok_or("Could not find manga page url")?;

        let img_element = div
            .select(&img_selector)
            .next()
            .ok_or("Could not find the img element containing the cover")?;

        let cover_img_url = img_element.attr("src").ok_or("Could not find the cover img url")?;

        Ok(Self {
            manga_page_url: manga_page_url.to_string(),
            title: title.to_string(),
            cover_img_url: cover_img_url.to_string(),
        })
    }
}

impl From<PopularMangaItem> for PopularManga {
    fn from(value: PopularMangaItem) -> Self {
        PopularManga {
            id: value.manga_page_url.to_string(),
            title: value.title.to_string(),
            genres: vec![],
            description: "".to_string(),
            status: None,
            cover_img_url: Some(value.cover_img_url.to_string()),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GetPopularMangasResponse {
    pub(super) mangas: Vec<PopularMangaItem>,
}

impl FromHtml for GetPopularMangasResponse {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn Error>> {
        let doc = html::Html::parse_document(html.as_str());
        let selector = Selector::parse("#owl-slider *").unwrap();

        let mut mangas: Vec<Result<PopularMangaItem, Box<dyn Error>>> = vec![];

        for child in doc.select(&selector) {
            mangas.push(PopularMangaItem::from_html(HtmlElement::new(child.html())));
        }

        Ok(Self {
            mangas: mangas.into_iter().flatten().collect(),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct NewMangaAddedToolTip {
    pub(super) manga_page_url: String,
    pub(super) id: String,
}

impl FromHtml for NewMangaAddedToolTip {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn Error>> {
        let a_tag = html::Html::parse_fragment(html.as_str());
        let selector = Selector::parse("a").unwrap();

        let a = a_tag.select(&selector).next().unwrap();

        let tool_tip = a.attr("data-tooltip").unwrap();

        let id = tool_tip.split("_").last().unwrap();
        let manga_page_url = a.attr("href").unwrap();

        Ok(Self {
            manga_page_url: manga_page_url.to_string(),
            id: id.to_string(),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct NewAddedMangas {
    pub(super) mangas: Vec<NewMangaAddedToolTip>,
}

impl FromHtml for NewAddedMangas {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn Error>> {
        let doc = html::Html::parse_document(html.as_str());

        let selector = Selector::parse(".panel-newest-content a").unwrap();

        let mut mangas: Vec<Result<NewMangaAddedToolTip, Box<dyn Error>>> = vec![];

        for child in doc.select(&selector).take(5) {
            let new_manga: Result<NewMangaAddedToolTip, Box<dyn Error>> = {
                let tool_tip = child.attr("data-tooltip").ok_or("")?;

                let id = tool_tip.split("_").last().ok_or("")?;
                let manga_page_url = child.attr("href").ok_or("")?;

                Ok(NewMangaAddedToolTip {
                    id: id.to_string(),
                    manga_page_url: manga_page_url.to_string(),
                })
            };

            mangas.push(new_manga);
        }

        Ok(NewAddedMangas {
            mangas: mangas.into_iter().flatten().collect(),
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ToolTipItem {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) image: String,
    pub(super) description: String,
}

#[derive(Debug, Default)]
pub(super) struct LatestMangaUpdatesResponse {}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn popular_manga_item_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="item"> 
                <img class="img-loading"
                        src="https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp"
                        onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                        alt="Yuusha Party O Oida Sareta Kiyou Binbou" />
                    <div class="slide-caption">
                        <h3>
                        <a class="text-nowrap a-h" href="https://manganato.com/manga-sn995770"
                                title="Yuusha Party O Oida Sareta Kiyou Binbou">Yuusha Party O Oida Sareta Kiyou
                                Binbou</a></h3><a rel="nofollow" class="text-nowrap a-h"
                            href="https://chapmanganato.to/manga-sn995770/chapter-30.1"
                            title="Yuusha Party O Oida Sareta Kiyou Binbou Chapter 30.1">Chapter 30.1</a>
                    </div>
                </div>
        "#;

        let popular_manga = PopularMangaItem::from_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(popular_manga.manga_page_url, "https://manganato.com/manga-sn995770");
        assert_eq!(popular_manga.title, "Yuusha Party O Oida Sareta Kiyou Binbou");
        assert_eq!(popular_manga.cover_img_url, "https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp");

        Ok(())
    }

    #[test]
    fn popular_manga_response_gets_inner_items() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div id="owl-slider" class="owl-carousel">
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/87/w/13-1733298325-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Inside the Cave of Obscenity" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-pk993067"
                                    title="Inside the Cave of Obscenity">Inside The Cave Of Obscenity</a></h3><a
                                rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-pk993067/chapter-21"
                                title="Inside the Cave of Obscenity Chapter 21">Chapter 21</a>
                        </div>
                    </div>
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/24/n/15-1733308050-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Creepy Pharmacist: All My Patients are Horrific" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-tp996650"
                                    title="Creepy Pharmacist: All My Patients are Horrific">Creepy Pharmacist: All My
                                    Patients Are Horrific</a></h3><a rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-tp996650/chapter-109"
                                title="Creepy Pharmacist: All My Patients are Horrific Chapter 109">Chapter 109</a>
                        </div>
                    </div>
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Yuusha Party O Oida Sareta Kiyou Binbou" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-sn995770"
                                    title="Yuusha Party O Oida Sareta Kiyou Binbou">Yuusha Party O Oida Sareta Kiyou
                                    Binbou</a></h3><a rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-sn995770/chapter-30.1"
                                title="Yuusha Party O Oida Sareta Kiyou Binbou Chapter 30.1">Chapter 30.1</a>
                        </div>
                    </div>
                </div>
        "#;

        let response = GetPopularMangasResponse::from_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(response.mangas[0].title, "Inside the Cave of Obscenity");
        assert_eq!(response.mangas[1].title, "Creepy Pharmacist: All My Patients are Horrific");
        assert_eq!(response.mangas[2].title, "Yuusha Party O Oida Sareta Kiyou Binbou");

        Ok(())
    }

    #[test]
    fn latest_manga_update_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                    <a data-tooltip="sticky_55557" class="tooltip" href="https://manganato.com/manga-df1006914">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/12/z/19-1738684967-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Gokinjo JK Isezaki-san wa Isekaigaeri no Daiseijo" /> 
                    </a>
        "#;

        let latest_manga = NewMangaAddedToolTip::from_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(latest_manga.manga_page_url, "https://manganato.com/manga-df1006914");
        assert_eq!(latest_manga.id, "55557");

        let html = r#"
                    <div class="panel-newest-content">
                        <a data-tooltip="sticky_55557" class="tooltip" href="https://manganato.com/manga-df1006914">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/12/z/19-1738684967-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Gokinjo JK Isezaki-san wa Isekaigaeri no Daiseijo" /> 
                        </a>
                        <a
                            data-tooltip="sticky_55535" class="tooltip" href="https://manganato.com/manga-dj1006892">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/12/d/19-1737905848.jpg"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="This is But a Hell of a Dream" /> 
                        </a>
                        <a data-tooltip="sticky_55534"
                            class="tooltip" href="https://manganato.com/manga-di1006891"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/12/c/19-1737905712-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Revenge Agent Hizumi-san" /> 
                        </a>
                        <a data-tooltip="sticky_55508" class="tooltip"
                            href="https://manganato.com/manga-di1006865"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/11/c/19-1737731251.jpg"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="The Saintess And The Curse" /> 
                        </a>
                        <a data-tooltip="sticky_55490" class="tooltip"
                            href="https://manganato.com/manga-dm1006847"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/10/j/19-1737559942.jpg"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="The Peculiar Room" /> 
                        </a>
                        <a data-tooltip="sticky_55482" class="tooltip"
                            href="https://manganato.com/manga-de1006839"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/10/b/19-1737519016.jpg"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Regression of the Yong Clan Heir" /> 
                        </a>
                        <a data-tooltip="sticky_55469"
                            class="tooltip" href="https://manganato.com/manga-dr1006826"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/9/o/19-1737470368.jpg"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="The Genius Wants to be Ordinary!" /> 
                        </a>
                        <a data-tooltip="sticky_55458"
                            class="tooltip" href="https://manganato.com/manga-dg1006815"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/9/d/19-1737383386-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Tenshi no Saezuri" /> 
                        </a>
                        <a data-tooltip="sticky_55456" class="tooltip"
                            href="https://manganato.com/manga-de1006813"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/9/b/19-1737383242-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="The Villain of the Octagon" /> 
                        </a><a data-tooltip="sticky_55452" class="tooltip"
                            href="https://manganato.com/manga-da1006809"> <img class="img-loading"
                                src="https://avt.mkklcdnv6temp.com/fld/8/x/19-1737345440-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Abandoned: The Hero Who's So Strong He Breaks Every Weapon, and the Elf Weaponsmith" />
                        </a>
                    </div>
        "#;

        let new_added_mangas = NewAddedMangas::from_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(new_added_mangas.mangas[0].id, "55557");
        assert_eq!(new_added_mangas.mangas[1].id, "55535");

        Ok(())
    }
}
