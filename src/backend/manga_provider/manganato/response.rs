use std::error::Error;
use std::fmt::Display;
use std::num::ParseIntError;

use scraper::selectable::Selectable;
use scraper::{html, Selector};
use serde::{Deserialize, Serialize};

use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::{
    Genres, GetMangasResponse, Languages, MangaStatus, PopularManga, Rating, RecentlyAddedManga, SearchManga,
};

#[derive(Debug, Default)]
pub(super) struct PopularMangaItem {
    pub(super) manga_page_url: String,
    pub(super) title: String,
    pub(super) cover_img_url: String,
    pub(super) additional_data: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoGenre {
    pub(super) name: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoStatus {
    pub(super) name: String,
}

#[derive(Debug)]
pub(super) struct PopularMangaItemError {
    reason: String,
}

impl<T: Into<String>> From<T> for PopularMangaItemError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for PopularMangaItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get popular manga on manganato, reason: {}", self.reason)
    }
}

impl Error for PopularMangaItemError {}

impl ParseHtml for PopularMangaItem {
    type ParseError = PopularMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let a_selector = Selector::parse(".item div h3 a").unwrap();
        let a_selector_additional_info = Selector::parse(".item div a").unwrap();
        let img_selector = Selector::parse(".item img").unwrap();

        let a_tag = div.select(&a_selector).next().ok_or("Could not find div element containing manga info")?;
        let a_tag_additional_info = div
            .select(&a_selector_additional_info)
            .last()
            .ok_or("could not find a tag containing additional information")?;
        let title = a_tag.attr("title").ok_or("Could not find manga title")?;
        let manga_page_url = a_tag.attr("href").ok_or("Could not find manga page url")?;

        let additiona_info = a_tag_additional_info.inner_html();

        let img_element = div
            .select(&img_selector)
            .next()
            .ok_or("Could not find the img element containing the cover")?;

        let cover_img_url = img_element.attr("src").ok_or("Could not find the cover img url")?;

        Ok(Self {
            manga_page_url: manga_page_url.to_string(),
            title: title.to_string(),
            cover_img_url: cover_img_url.to_string(),
            additional_data: format!("Latest chapter: {additiona_info}"),
        })
    }
}

impl From<PopularMangaItem> for PopularManga {
    fn from(value: PopularMangaItem) -> Self {
        PopularManga {
            id: value.manga_page_url.to_string(),
            title: value.title.to_string(),
            genres: vec![],
            description: value.additional_data,
            status: None,
            cover_img_url: Some(value.cover_img_url.to_string()),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GetPopularMangasResponse {
    pub(super) mangas: Vec<PopularMangaItem>,
}

impl ParseHtml for GetPopularMangasResponse {
    type ParseError = PopularMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let selector = Selector::parse("#owl-slider > *").unwrap();

        let mut mangas: Vec<Result<PopularMangaItem, <PopularMangaItem as ParseHtml>::ParseError>> = vec![];

        for child in doc.select(&selector) {
            mangas.push(PopularMangaItem::parse_html(HtmlElement::new(child.html())));
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

#[derive(Debug, Default, Clone)]
pub(super) struct NewAddedMangas {
    pub(super) mangas: Vec<NewMangaAddedToolTip>,
}

#[derive(Debug)]
pub(super) struct NewAddedMangasError {
    reason: String,
}

impl<T: Into<String>> From<T> for NewAddedMangasError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for NewAddedMangasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to recently added mangason manganato, reason: {}", self.reason)
    }
}

impl Error for NewAddedMangasError {}

impl ParseHtml for NewAddedMangas {
    type ParseError = NewAddedMangasError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let selector = Selector::parse(".panel-newest-content > a").unwrap();

        let mut mangas: Vec<Result<NewMangaAddedToolTip, Self::ParseError>> = vec![];

        for child in doc.select(&selector).take(5) {
            let new_manga: Result<NewMangaAddedToolTip, Self::ParseError> = {
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

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchMangaItem {
    pub(super) manga_page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) rating: String,
    pub(super) latest_chapters: String,
    pub(super) description: Option<String>,
}

#[derive(Debug)]
pub(super) struct SearchMangaResponseError {
    reason: String,
}

impl<T: Into<String>> From<T> for SearchMangaResponseError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for SearchMangaResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to search mangas on manganato, reason: {}", self.reason)
    }
}

impl Error for SearchMangaResponseError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchMangaResponse {
    pub(super) mangas: Vec<SearchMangaItem>,
    pub(super) total_mangas: u32,
}

impl ParseHtml for SearchMangaResponse {
    type ParseError = SearchMangaResponseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let check_search_is_not_found = Selector::parse(".panel-content-genres").unwrap();

        if doc.select(&check_search_is_not_found).next().is_none() {
            return Ok(SearchMangaResponse {
                mangas: vec![],
                total_mangas: 0,
            });
        }

        // at this point search contains mangas

        let selector_div_containing_mangas = Selector::parse(".panel-content-genres > *").unwrap();

        let selector_total_mangas = Selector::parse(".panel-page-number > .group-qty > a").unwrap();

        let mut mangas: Vec<Result<SearchMangaItem, <SearchMangaItem as ParseHtml>::ParseError>> = vec![];

        for div in doc.select(&selector_div_containing_mangas) {
            mangas.push(SearchMangaItem::parse_html(HtmlElement::new(div.html())));
        }

        let maybe_total_mangas = doc.select(&selector_total_mangas).next();

        // if this tag is not present then there is only one page
        let total_mangas: u32 = if let Some(total) = maybe_total_mangas {
            let total_mangas: Result<u32, ParseIntError> = {
                let total_mangas = total.inner_html();

                let total_mangas = total_mangas.split(":").last().ok_or("no total")?;
                let total_mangas: String = total_mangas.split(",").collect();
                total_mangas.trim().parse()
            };

            total_mangas.map_err(|e| e.to_string())?
        } else {
            mangas.len() as u32
        };

        Ok(Self {
            mangas: mangas.into_iter().map(|may| may.unwrap()).collect(),
            total_mangas,
        })
    }
}

#[derive(Debug)]
pub(super) struct SearchMangaItemError {
    reason: String,
}

impl<T: Into<String>> From<T> for SearchMangaItemError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for SearchMangaItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get manga item on search on manganato, reason: {}", self.reason)
    }
}

impl Error for SearchMangaItemError {}

impl ParseHtml for SearchMangaItem {
    type ParseError = SearchMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());

        let img_selector = Selector::parse("img").unwrap();

        let img = div.select(&img_selector).next().ok_or("no img")?;
        let cover_url = img.attr("src").ok_or("no cover")?;

        let rating_selector = Selector::parse(".genres-item-rate").unwrap();
        let rating = div.select(&rating_selector).next().ok_or("no rating tag found")?;

        let a_containing_manga_page_url = Selector::parse("a").unwrap();
        let a_manga_page_url = div.select(&a_containing_manga_page_url).next().ok_or("no a tag found")?;

        let title = a_manga_page_url.attr("title").ok_or("title not found")?;
        let manga_page_url = a_manga_page_url.attr("href").ok_or("no href")?;

        let latest_chapters_selector = Selector::parse(".genres-item-chap").unwrap();
        let latest_chapters: String = div.select(&latest_chapters_selector).into_iter().map(|a| a.inner_html()).collect();

        let description_selector = Selector::parse(".genres-item-description").unwrap();
        let description = div.select(&description_selector).next().map(|desc| desc.inner_html().trim().to_string());

        Ok(Self {
            manga_page_url: manga_page_url.to_string(),
            title: title.to_string(),
            cover_url: cover_url.to_string(),
            rating: rating.inner_html(),
            latest_chapters: latest_chapters.trim().to_string(),
            description,
        })
    }
}

impl From<SearchMangaItem> for SearchManga {
    fn from(value: SearchMangaItem) -> Self {
        Self {
            id: value.manga_page_url,
            title: value.title,
            genres: vec![],
            description: value.description,
            status: None,
            cover_img_url: Some(value.cover_url),
            languages: vec![Languages::English],
            artist: None,
            author: None,
        }
    }
}

impl From<SearchMangaResponse> for GetMangasResponse {
    fn from(value: SearchMangaResponse) -> Self {
        Self {
            mangas: value.mangas.into_iter().map(SearchManga::from).collect(),
            total_mangas: value.total_mangas,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct MangaPageData {
    pub(super) title: String,
    pub(super) authors: String,
    pub(super) status: ManganatoStatus,
    pub(super) genres: Vec<ManganatoGenre>,
    pub(super) rating: String,
    pub(super) cover_url: String,
    pub(super) description: String,
}

#[derive(Debug)]
pub(super) struct MangaPageDataError {
    reason: String,
}

impl<T: Into<String>> From<T> for MangaPageDataError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for MangaPageDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get manga on manganato, reason: {}", self.reason)
    }
}

impl Error for MangaPageDataError {}

impl ParseHtml for MangaPageData {
    type ParseError = MangaPageDataError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let right_div_selector = Selector::parse(".story-info-right").unwrap();

        let right_div_containing_most_info = div
            .select(&right_div_selector)
            .next()
            .ok_or("The div containing most information on the manga was not found")?;

        let title_selector = Selector::parse("h1").unwrap();

        let title = right_div_containing_most_info
            .select(&title_selector)
            .next()
            .ok_or("No title found")?
            .inner_html();

        let row_selector = Selector::parse("tr").unwrap();

        let authors_selector = Selector::parse(".table-label > .info-author").unwrap();
        let value_selector_a_tag = Selector::parse(".table-value > a").unwrap();

        let status_selector = Selector::parse(".table-label > .info-status").unwrap();

        let status_selector_value = Selector::parse(".table-value").unwrap();
        let genres_selector = Selector::parse(".table-label > .info-genres").unwrap();

        let mut authors = String::new();
        let mut status = ManganatoStatus {
            name: String::new(),
        };
        let mut genres: Vec<ManganatoGenre> = vec![];

        for rows in right_div_containing_most_info.select(&row_selector) {
            if rows.select(&authors_selector).next().is_some() {
                authors = rows
                    .select(&value_selector_a_tag)
                    .next()
                    .ok_or("no author")?
                    .inner_html()
                    .trim()
                    .to_string();
            } else if rows.select(&status_selector).next().is_some() {
                status.name = rows
                    .select(&status_selector_value)
                    .next()
                    .ok_or("no status tag")?
                    .inner_html()
                    .trim()
                    .to_string();
            } else if rows.select(&genres_selector).next().is_some() {
                for genre in rows.select(&value_selector_a_tag) {
                    genres.push(ManganatoGenre {
                        name: genre.inner_html(),
                    });
                }
            }
        }

        let rating_selector = Selector::parse(r#"em[property="v:average"]"#).unwrap();
        let rating = right_div_containing_most_info
            .select(&rating_selector)
            .next()
            .ok_or("no rating tag")?
            .inner_html();

        let rating = format!("{rating} out of 5");

        let img_selector = Selector::parse(".story-info-left .img-loading").unwrap();

        let cover_url = div
            .select(&img_selector)
            .next()
            .ok_or("no img tag found")?
            .attr("src")
            .ok_or("no src attribute on img tag")?
            .to_string();

        let description_selector = Selector::parse("#panel-story-info-description").unwrap();
        let description_div = div.select(&description_selector).next().ok_or("no description tag found")?;
        let mut description = String::new();

        // This is how we can obtain the inner text of a element without whithout tags
        for node in html::Html::parse_fragment(&description_div.inner_html()).tree {
            if let scraper::node::Node::Text(text) = node {
                description = text.to_string();
            }
        }

        Ok(Self {
            description,
            title,
            authors,
            status,
            genres,
            rating,
            cover_url,
        })
    }
}

impl From<ManganatoGenre> for Genres {
    fn from(value: ManganatoGenre) -> Self {
        let rating = match value.name.to_lowercase().as_str() {
            "smut" => Rating::Nsfw,
            "adult" => Rating::Nsfw,
            "erotica" => Rating::Nsfw,
            "pornographic" => Rating::Nsfw,
            "ecchi" => Rating::Moderate,
            "doujinshi" => Rating::Doujinshi,
            "mature" => Rating::Nsfw,
            _ => Rating::Normal,
        };

        Genres::new(value.name, rating)
    }
}

impl From<ManganatoStatus> for MangaStatus {
    fn from(value: ManganatoStatus) -> Self {
        match value.name.to_lowercase().as_str() {
            "ongoing" => MangaStatus::Ongoing,
            "completed" => MangaStatus::Completed,
            _ => MangaStatus::Ongoing,
        }
    }
}

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
                                Binbou</a>
                        </h3>
                        <a rel="nofollow" class="text-nowrap a-h"
                            href="https://chapmanganato.to/manga-sn995770/chapter-30.1"
                            title="Yuusha Party O Oida Sareta Kiyou Binbou Chapter 30.1">Chapter 30.1</a>
                    </div>
                </div>
        "#;

        let popular_manga = PopularMangaItem::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(popular_manga.manga_page_url, "https://manganato.com/manga-sn995770");
        assert_eq!(popular_manga.title, "Yuusha Party O Oida Sareta Kiyou Binbou");
        assert_eq!(popular_manga.cover_img_url, "https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp");
        assert_eq!(popular_manga.additional_data, "Latest chapter: Chapter 30.1");

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

        let response = GetPopularMangasResponse::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(response.mangas[0].title, "Inside the Cave of Obscenity");
        assert_eq!(response.mangas[1].title, "Creepy Pharmacist: All My Patients are Horrific");
        assert_eq!(response.mangas[2].title, "Yuusha Party O Oida Sareta Kiyou Binbou");

        Ok(())
    }

    #[test]
    fn latest_manga_update_parses_from_html() -> Result<(), Box<dyn Error>> {
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
                    </div>
        "#;

        let new_added_mangas = NewAddedMangas::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(new_added_mangas.mangas[0].id, "55557");
        assert_eq!(new_added_mangas.mangas[1].id, "55535");
        assert_eq!(new_added_mangas.mangas.len(), 5);

        Ok(())
    }

    #[test]
    fn search_results_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
            <div class="panel-content-genres">
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="MzM2MTU="
                        href="https://chapmanganato.to/manga-hp984972" title="National School Prince Is A Girl">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="National School Prince Is A Girl" />
                        <em class="genres-item-rate">4.7</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-hp984972"
                                title="National School Prince Is A Girl">National School Prince Is A Girl</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-hp984972/chapter-504"
                            title="National School Prince Is A Girl Chapter 504">Chapter 504</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">21.4M</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Warring Young Seven,战七少 战七少</span>
                        </p>
                        <div class="genres-item-description">
                            Fu Jiu appears to be a normal lad in high school on the surface.
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-hp984972">Read more</a>
                    </div>
                </div>
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="NTI5NzE="
                        href="https://chapmanganato.to/manga-at1004328"
                        title="26 Sai Shojo, Charao Joushi ni Dakaremashita">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/18/b/18-1733328343-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="26 Sai Shojo, Charao Joushi ni Dakaremashita" />
                        <em class="genres-item-rate">4.4</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-at1004328"
                                title="26 Sai Shojo, Charao Joushi ni Dakaremashita">26 Sai Shojo, Charao Joushi Ni
                                Dakaremashita</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-at1004328/chapter-15"
                            title="26 Sai Shojo, Charao Joushi ni Dakaremashita Chapter 15">Chapter 15</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">96.9K</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Ryo Nakaharu , NAKAHARU Ryou</span>
                        </p>
                        <div class="genres-item-description">
                            He's going at her so hard. She knows it's wrong, but it feels so good, she can't stop!
                            Chikage Ayashiro (26) has been thrown into a project with her boss, Toru Aogiri, who's a
                            playboy and hard to grasp. After a kick-off party, she ends up alone with him for some
                            reason! But, once the two talk, he seems more serious and... Drunk, they wind up at a hotel
                            where Toru turns out to be kind... As her bod <i class="genres-item-description-linear"></i>
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-at1004328">Read more</a>
                    </div>
                </div>
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="NDkwOTM="
                        href="https://chapmanganato.to/manga-xp1000450" title="Vanilla land">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/69/o/16-1733318212-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Vanilla land" />
                        <em class="genres-item-rate">3.5</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-xp1000450" title="Vanilla land">Vanilla Land</a>
                        </h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-xp1000450/chapter-69.5"
                            title="Vanilla land Chapter 69.5: The Plan">Chapter 69.5: The Plan</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">46.6K</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Couple Comics</span>
                        </p>
                        <div class="genres-item-description">
                            In lands far..far away, where scary monsters and horrific creatures live. where hidden
                            treasures and magic exists, where there are still undiscovered kingdoms and empires mankind
                            discovered a new source of energy to empower their mechanical machines & weapons. we
                            discovered vanilla the source of unlimited energy. follow the journey of gen orchid, rosa &
                            altamiras as they seek the djinn chef master <i class="genres-item-description-linear"></i>
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-xp1000450">Read more</a>
                    </div>
                </div>
            </div>

                <div class="panel-page-number">
                    <div class="group-page"><a href="https://manganato.com/search/story/death?page=1"
                            class="page-blue">FIRST(1)</a><a class="page-blue">1</a><a
                            href="https://manganato.com/search/story/death?page=2">2</a><a
                            href="https://manganato.com/search/story/death?page=3">3</a><a
                            href="https://manganato.com/search/story/death?page=12"
                            class="page-blue page-last">LAST(12)</a></div>
                    <div class="group-qty"><a class="page-blue">TOTAL : 3</a></div>
                </div>
        "#;

        let search_manga_response = SearchMangaResponse::parse_html(HtmlElement::new(html.to_string()))?;

        let expected1 = SearchMangaItem {
            manga_page_url: "https://chapmanganato.to/manga-hp984972".to_string(),
            title: "National School Prince Is A Girl".to_string(),
            latest_chapters: "Chapter 504".to_string(),
            rating: "4.7".to_string(),
            cover_url: "https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp".to_string(),
            description: Some("Fu Jiu appears to be a normal lad in high school on the surface.".to_string()),
        };

        //let expected2 = SearchMangaItem {
        //    manga_page_url: "https://chapmanganato.to/manga-hs985227".to_string(),
        //    title: "Villains Are Destined to Die".to_string(),
        //    latest_chapters: "Latest chapters: Chapter 162 Chapter 161 ".to_string(),
        //    rating: "4.9".to_string(),
        //    cover_url: "https://avt.mkklcdnv6temp.com/fld/96/y/10-1732803814-nw.webp".to_string(),
        //    updated_at: "Updated : Sep 13,2024 - 16:34".to_string(),
        //    authors: Some("Gwon Gyeoeul".to_string()),
        //};

        assert!(search_manga_response.mangas.len() > 2);
        assert_eq!(search_manga_response.total_mangas, 3);
        assert_eq!(expected1, search_manga_response.mangas[0]);
        //assert_eq!(expected2, search_manga_response.mangas[1]);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        // When there is no more total mangas than what is found
        let html = r#"
              <div class="panel-content-genres">
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="MzM2MTU="
                        href="https://chapmanganato.to/manga-hp984972" title="National School Prince Is A Girl">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="National School Prince Is A Girl" />
                        <em class="genres-item-rate">4.7</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-hp984972"
                                title="National School Prince Is A Girl">National School Prince Is A Girl</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-hp984972/chapter-504"
                            title="National School Prince Is A Girl Chapter 504">Chapter 504</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">21.4M</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Warring Young Seven,战七少 战七少</span>
                        </p>
                        <div class="genres-item-description">
                            Fu Jiu appears to be a normal lad in high school on the surface.
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-hp984972">Read more</a>
                    </div>
                </div>
            </div>
        "#;

        let response = SearchMangaResponse::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(response.total_mangas, 1);
        Ok(())
    }

    #[test]
    fn manga_page_response_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"

                            <!-- this img tag may seem random but there are multiple imgs with this classname-->
                            <img class="img-loading" src="https://notthisone.webp"
                                alt="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                title="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';" />

                <div class="panel-story-info">
                    <div class="story-info-left">
                        <span class="info-image">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/43/y/15-1733309270-nw.webp"
                                alt="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                title="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';" />
                            <em class="item-hot"></em> </span>

                        <span class="style-btn btn-chapterlist">READ CHAPTER LIST</span>
                    </div>

                    <div class="story-info-right">
                        <h1>Dungeon Ni Hisomu Yandere Na Kanojo Ni Ore Wa Nando Mo Korosareru</h1>

                        <table class="variations-tableInfo">
                            <tbody>
                                <tr>
                                    <td class="table-label"><i class="info-alternative"></i>Alternative :</td>
                                    <td class="table-value">
                                        <h2>ダンジョンに潜むヤンデレな彼女に俺は何度も殺される ; My Yandere Girlfriend Lurks in the Dungeon and
                                            Kills Me Over and Over Again</h2>
                                    </td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-author"></i>Author(s) :</td>
                                    <td class="table-value">
                                        <a rel="nofollow" class='a-h'
                                            href='https://manganato.com/author/story/a2l0YWdhd2FfbmlraXRh'>Kitagawa Nikita</a> - <a rel="nofollow" class='a-h'
                                            href='https://manganato.com/author/story/fHxub211cmFfZWpp'>Nomura Eji</a>
                                    </td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-status"></i>Status :</td>
                                    <td class="table-value">Ongoing</td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-genres"></i>Genres :</td>
                                    <td class="table-value">
                                        <a class='a-h' href='https://manganato.com/genre-2'>Action</a> - <a class='a-h'
                                            href='https://manganato.com/genre-4'>Adventure</a> - <a class='a-h'
                                            href='https://manganato.com/genre-10'>Drama</a> - <a class='a-h'
                                            href='https://manganato.com/genre-12'>Fantasy</a> - <a class='a-h'
                                            href='https://manganato.com/genre-14'>Harem</a> - <a class='a-h'
                                            href='https://manganato.com/genre-27'>Romance</a> - <a class='a-h'
                                            href='https://manganato.com/genre-30'>Seinen</a> - <a class='a-h'
                                            href='https://manganato.com/genre-38'>Supernatural</a> 
                                    </td>
                                </tr>
                            </tbody>
                        </table>

                        <div class="story-info-right-extent">
                            <p><span class="stre-label"><i class="info-time"></i>Updated :</span><span
                                    class="stre-value">Jan 31,2025 - 23:03 PM</span></p>
                            <p><span class="stre-label"><i class="info-view"></i>View :</span><span
                                    class="stre-value">4.5M</span></p>
                            <p>
                                <span class="stre-label"><i class="info-rate"></i>Rating :</span>
                                <span class="stre-value">
                                    <em class="rate_row" id="rate_row"></em>
                                    <em class="rate_row_result"></em>
                                </span>
                            </p>
                            <p>
                                <em id="rate_row_cmd">
                                    <em xmlns:v="http://rdf.data-vocabulary.org/#" typeof="v:Review-aggregate">
                                        <em property="v:itemreviewed">MangaNato.com</em>
                                        <em rel="v:rating">
                                            <em typeof="v:Rating">rate :
                                                <em property="v:average">4.49</em>/
                                                <em property="v:best">5</em>
                                            </em>
                                        </em> - <em property="v:votes">3281</em> votes
                                    </em>
                                </em>
                            </p>
                            <p class="user_btn_follow_i info-bookmark"></p>
                            <p class="noti-bookmark"></p>

                            <p class="fb-save" data-uri="https://manganato.com/manga-tc997159" data-size="small"
                                id="savefilmfb"></p>
                            <div class="fb-like" data-href="https://manganato.com/manga-tc997159"
                                data-layout="button_count" data-action="like" data-show-faces="false" data-share="true">
                            </div>

                        </div>
                    </div>

                    <div class="panel-story-info-description" id="panel-story-info-description"> <h3>Description :</h3> "Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru" is a fantasy story about Kiska, a boy who is mistreated by the villagers. He manages to survive thanks to his childhood friend Namia, who was the only one by his side. One day, Namia is murdered by three men</div>
                    <div id="panel-description-linear">
                        <i id="pn-description-linear-gradient" style="display: none;"></i>
                    </div>
                    <span style="display: none;" id="panel-story-info-description-show-more" class="a-h">SHOW MORE
                        ⇩</span>
                    <span style="display: initial;" id="panel-story-info-description-show-less" class="a-h">SHOW LESS
                        ⇧</span>
                </div>
        "#;

        let genres: Vec<ManganatoGenre> = ["Action", "Adventure", "Drama", "Fantasy", "Harem", "Romance", "Seinen", "Supernatural"]
            .into_iter()
            .map(|gen| ManganatoGenre {
                name: gen.to_string(),
            })
            .collect();

        let expected: MangaPageData = MangaPageData {
            title: "Dungeon Ni Hisomu Yandere Na Kanojo Ni Ore Wa Nando Mo Korosareru".to_string(),
            authors: "Kitagawa Nikita".to_string(),
            status: ManganatoStatus{name : "Ongoing".to_string()},
            genres,
            rating: "4.49 out of 5".to_string(),
            description: r#" "Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru" is a fantasy story about Kiska, a boy who is mistreated by the villagers. He manages to survive thanks to his childhood friend Namia, who was the only one by his side. One day, Namia is murdered by three men"#
            .to_string(),
            cover_url: "https://avt.mkklcdnv6temp.com/fld/43/y/15-1733309270-nw.webp".to_string(),
        };

        let result = MangaPageData::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(expected, result);
        Ok(())
    }
}
