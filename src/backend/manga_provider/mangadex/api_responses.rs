use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};

use crate::config::ImageQuality;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMangaResponse {
    pub result: String,
    pub response: String,
    pub data: Vec<Data>,
    pub limit: i32,
    pub offset: u32,
    pub total: u32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub id: String,
    pub attributes: Attributes,
    pub relationships: Vec<MangaSearchRelationship>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub title: Title,
    pub description: Option<Description>,
    pub status: String,
    pub tags: Vec<Tag>,
    pub content_rating: String,
    pub state: String,
    pub created_at: String,
    pub publication_demographic: Option<String>,
    pub available_translated_languages: Vec<Option<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Title {
    pub en: Option<String>,
    pub ja: Option<String>,
    #[serde(rename = "ja-ro")]
    pub ja_ro: Option<String>,
    pub jp: Option<String>,
    pub zh: Option<String>,
    pub ko: Option<String>,
    #[serde(rename = "zh-ro")]
    pub zh_ro: Option<String>,
    #[serde(rename = "ko-ro")]
    pub ko_ro: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub en: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaSearchRelationship {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: Option<MangaSearchAttributes>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MangaSearchAttributes {
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
    pub name: Option<String>,
    pub locale: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub attributes: TagAtributtes,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAtributtes {
    pub name: Name,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Name {
    pub en: String,
}

// manga chapter structs
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterResponse {
    pub data: Vec<ChapterData>,
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterData {
    pub id: String,
    pub attributes: ChapterAttribute,
    pub relationships: Vec<Relationship>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterAttribute {
    pub volume: Option<String>,
    pub chapter: Option<String>,
    pub title: Option<String>,
    pub translated_language: String,
    pub readable_at: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: Option<ChapterRelationshipAttribute>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterRelationshipAttribute {
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPagesResponse {
    pub result: String,
    pub base_url: String,
    pub chapter: ChapterPages,
}

impl ChapterPagesResponse {
    /// According to mangadex api the endpoint to get a chapter's panel is built as follows: `base_url`/`data`, data-saver`/`hash`
    pub fn get_image_url_endpoint(&self, quality: ImageQuality) -> String {
        format!("{}/{}/{}", self.base_url, quality.as_param(), self.chapter.hash)
    }

    /// Based on the mangadex api the `data_saver` array is used when image quality is low and
    /// `data` is used when ImageQuality is high
    pub fn get_files_based_on_quality(self, quality: ImageQuality) -> Vec<String> {
        match quality {
            ImageQuality::Low => self.chapter.data_saver,
            ImageQuality::High => self.chapter.data,
        }
    }

    /// Based on the mangadex api the `data_saver` array is used when image quality is low and
    /// `data` is used when ImageQuality is high
    pub fn get_files_based_on_quality_as_url(self, quality: ImageQuality) -> Vec<Url> {
        let base_endpoint = self.get_image_url_endpoint(quality);

        let endpoint_formatted = |raw_url: String| format!("{base_endpoint}/{}", raw_url).parse::<Url>();

        match quality {
            ImageQuality::Low => self
                .chapter
                .data_saver
                .into_iter()
                .map(endpoint_formatted)
                .filter_map(|res| res.ok())
                .collect(),
            ImageQuality::High => self.chapter.data.into_iter().map(endpoint_formatted).filter_map(|res| res.ok()).collect(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPages {
    pub hash: String,
    pub data: Vec<String>,
    pub data_saver: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaStatisticsResponse {
    pub result: String,
    pub statistics: HashMap<String, Statistics>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub rating: Rating,
    pub follows: Option<u64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    pub average: Option<f64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateChapterResponse {
    pub result: String,
    pub volumes: HashMap<String, Volumes>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volumes {
    pub volume: String,
    pub count: i32,
    #[serde(deserialize_with = "deserialize_aggregate_chapters")]
    pub chapters: HashMap<String, Chapters>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum VecOrHashMap {
    Hash(HashMap<String, Chapters>),
    Vec(Vec<Chapters>),
}

/// Sometimes when the manga has volume 0 the field `chapters` is not a `HashMap` but a `Vec<Chapters>`
pub fn deserialize_aggregate_chapters<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HashMap<String, Chapters>, D::Error> {
    let mut chapters = HashMap::new();

    let deserialized = VecOrHashMap::deserialize(deserializer)?;

    match deserialized {
        VecOrHashMap::Vec(chap) => {
            for (index, chapter) in chap.into_iter().enumerate() {
                chapters.insert(index.to_string(), chapter);
            }
        },
        VecOrHashMap::Hash(hash) => chapters = hash,
    }

    Ok(chapters)
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapters {
    pub chapter: String,
    pub id: String,
    pub others: Vec<String>,
    pub count: i32,
}

pub mod feed {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OneMangaResponse {
        pub result: String,
        pub response: String,
        pub data: super::Data,
    }
}

pub mod tags {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TagsResponse {
        pub result: String,
        pub response: String,
        pub data: Vec<TagsData>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TagsData {
        pub id: String,
        #[serde(rename = "type")]
        pub type_field: String,
        pub attributes: Attributes,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attributes {
        pub name: Name,
        pub group: String,
        pub version: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Name {
        pub en: String,
    }
}

pub mod authors {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuthorsResponse {
        pub result: String,
        pub response: String,
        pub data: Vec<Data>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Data {
        pub id: String,
        #[serde(rename = "type")]
        pub type_field: String,
        pub attributes: Attributes,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attributes {
        pub name: String,
        pub created_at: String,
        pub updated_at: String,
        pub version: i64,
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneChapterResponse {
    pub data: OneChapterData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneChapterData {
    pub id: String,
    pub attributes: ChapterAttribute,
}

/* as of v0.5.0 */
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMangaByIdResponse {
    pub data: GetMangaByIdData,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMangaByIdData {
    pub id: String,
    pub attributes: GetMangaByIdAttributes,
    pub relationships: Vec<MangaRelationship>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMangaByIdAttributes {
    pub title: Title,
    pub description: Option<Description>,
    pub publication_demograpchic: Option<String>,
    pub tags: Vec<Tag>,
    pub content_rating: String,
    pub status: String,
    pub available_translated_languages: Vec<Option<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaRelationship {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: Option<MangaRelationshipAttributes>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MangaRelationshipAttributes {
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn it_constructs_manga_panel_endpoint_based_on_image_quality() {
        let mut response = ChapterPagesResponse::default();

        response.chapter.data_saver = vec!["low_quality1.jpg".into(), "low_quality2.jpg".into()];
        response.chapter.data = vec!["high_quality1.jpg".into(), "high_quality2.jpg".into()];

        response.chapter.hash = "the_hash".to_string();
        response.base_url = "http://localhost".to_string();

        let image_quality = ImageQuality::Low;

        let expected: Url =
            format!("{}/{}/{}/low_quality1.jpg", response.base_url, image_quality.as_param(), response.chapter.hash,)
                .parse()
                .unwrap();

        assert_eq!(&expected, response.clone().get_files_based_on_quality_as_url(image_quality).first().unwrap());

        let image_quality = ImageQuality::High;

        let expected: Url =
            format!("{}/{}/{}/high_quality1.jpg", response.base_url, image_quality.as_param(), response.chapter.hash)
                .parse()
                .unwrap();

        assert_eq!(&expected, response.clone().get_files_based_on_quality_as_url(image_quality).first().unwrap());
    }

    #[test]
    fn endpoint_to_obtain_a_chapter_panel_is_built_correctly() {
        let response = ChapterPagesResponse {
            base_url: "http://some_url".to_string(),
            chapter: ChapterPages {
                hash: Uuid::new_v4().to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        let image_quality = ImageQuality::High;
        assert_eq!(format!("http://some_url/data/{}", response.chapter.hash), response.get_image_url_endpoint(image_quality));

        let image_quality = ImageQuality::Low;
        assert_eq!(format!("http://some_url/data-saver/{}", response.chapter.hash), response.get_image_url_endpoint(image_quality));
    }

    // These case happens when a manga has volume "0", the `chapters` field is and array instead of
    // a HashMap
    #[test]
    fn aggregate_response_deserializes_manga_with_volume_0() -> Result<(), Box<dyn std::error::Error>> {
        let example = r#"
{
"result": "ok",
  "volumes": {
    "0": {
      "volume": "0",
      "count": 1,
      "chapters": [
        {
          "chapter": "0",
          "id": "6676ffdf-ed39-4627-8cc2-643f761a79c7",
          "others": [],
          "count": 1
        }
      ]
    },
    "1": {
      "volume": "1",
      "count": 10,
      "chapters": {
        "1": {
          "chapter": "1",
          "id": "de7e7d14-6a13-427c-9438-feeec0f9ea96",
          "others": [
            "fa4059e4-3c0d-4d14-8f29-e82db74357d8"
          ],
          "count": 2
        },
        "2": {
          "chapter": "2",
          "id": "829e8d36-e243-4a4f-9fed-7a6bbdaa029d",
          "others": [
            "cbcd85a3-6fde-4ce9-8d2f-67041ae7aabf"
          ],
          "count": 2
        }
        
      }
    }
  }
}
        "#;

        let response: AggregateChapterResponse = serde_json::from_str(example)?;

        assert!(response.volumes.contains_key("0"));
        assert!(response.volumes.contains_key("1"));

        Ok(())
    }

    #[test]
    fn it_dese() {
        let test = r#"
{
  "result": "ok",
  "response": "entity",
  "data": {
    "id": "75ee72ab-c6bf-4b87-badd-de839156934c",
    "type": "manga",
    "attributes": {
      "title": {
        "en": "Death Note"
      },
      "altTitles": [
        {
          "ro": "Caietul morții"
        },
        {
          "es": "Cuaderno de la Muerte"
        },
        {
          "hu": "Death Note - A halállista"
        },
        {
          "vi": "Quyển Sổ Thiên Mệnh"
        },
        {
          "ru": "Тетрадь смерти"
        },
        {
          "he": "מחברת המוות"
        },
        {
          "th": "สมุดโน๊ตกระชากวิณญาณ"
        },
        {
          "zh": "死亡笔记"
        },
        {
          "ko": "데스 노트"
        },
        {
          "kk": "Ажал дәптері"
        },
        {
          "pt": "Caderno da Morte"
        },
        {
          "pt-br": "Caderno da Morte"
        },
        {
          "tr": "Ölüm Defteri"
        },
        {
          "ja": "DEATH NOTE"
        },
        {
          "ja": "デスノート"
        },
        {
          "ja-ro": "DEATH NOTE"
        },
        {
          "ja-ro": "Desu Nōto"
        },
        {
          "en": "DEATH NOTE"
        },
        {
          "cv": "Вилӗм типтерӗ"
        }
      ],
      "description": {
        "en": "A shinigami, as a god of death, can kill any person—provided they see their victim's face and write their victim's name in a notebook called a Death Note. One day, Ryuk, bored by the shinigami lifestyle and interested in seeing how a human would use a Death Note, drops one into the human realm.  \n  \nHigh school student and prodigy Light Yagami stumbles upon the Death Note and—since he deplores the state of the world—tests the deadly notebook by writing a criminal's name in it. When the criminal dies immediately following his experiment with the Death Note, Light is greatly surprised and quickly recognizes how devastating the power that has fallen into his hands could be.  \n  \nWith this divine capability, Light decides to extinguish all criminals in order to build a new world where crime does not exist and people worship him as a god. Police, however, quickly discover that a serial killer is targeting criminals and, consequently, try to apprehend the culprit. To do this, the Japanese investigators count on the assistance of the best detective in the world: a young and eccentric man known only by the name of L.",
        "kk": "Иәгами Лайт — үлкен болашағы бар оқушы және ол іші пысқаннан шаршап кетті. Бірақ оның бәрі ажал құдайы Рүк тастап кеткен Ажал Дәптерін тапқанда өзгереді. Есімі дәптерге жазылған кез келген адам өледі екен, енді Лайт дүниені зұлымдықтан тазарту үшін Ажал дәптерінің күшін пайдалануға ант етті. Алайда қылмыскерлер өле бастағанда, билік өлтірушіні іздеуге аты аңызға айналған детектив L-ді жібереді. L оның артынан аңди бастағанда, Лайт өзінің асыл мақсатынан ба... әлде өмірінен айырыла ма?",
        "pt": "A história gira em torno do estudante Yagami Raito que encontra por acaso o caderno de um \"Shinigami\", um \"Deus da Morte, cujo nome é Death Note\". Raito/Light percebe que ao escrever o nome de alguém no caderno, a pessoa literalmente morre, assim, ele pretende criar um mundo perfeito, um novo mundo.",
        "ru": "Ягами Лайт — образцовый 17-летний выпускник, баллы за экзамены которого находятся в первых строках рейтинга всей Японии. Сидя на уроке, он замечает, что за окном что-то упало. На перемене он поднимает загадочный предмет и им оказывается черная тетрадь с надписью «Тетрадь смерти». Внутри была инструкция по использованию: «Человек, имя которого будет записано в тетради, умрет». Имея свои взгляды на систему наказания, Лайт решает установить собственное правосудие, использовать тетрадь для «очищения» мира от зла — убивать преступников.Когда действия Лайта становятся заметны для мирового правительства, на след неуловимого «Киры» (так мир окрестил нового мессию, решившего искоренить зло на планете) выходит детектив мирового класса, называющий себя «L», который поставил себе цель — разоблачить убийцу.",
        "pt-br": "Sem nada de interessante para fazer no Mundo dos Shinigamis, o Deus da Morte Ryuk deixa cair intencionalmente na Terra o seu Death Note.O caderno possui poderes macabros: a pessoa que tem seu nome escrito nele, morre! O Death Note acaba indo parar na mão de Light Yagami. Aluno exemplar, porém entediado, ao descobrir os sinistros poderes do caderno negro, ele decide virar um justiceiro e varrer a criminalidade da face da Terra. As seguidas mortes de criminosos em vários países diferentes acabam chamando a atenção da Interpol, que, por sua vez, pede ajuda ao maior detetive do mundo, conhecido apenas por \"L\", para resolver o caso. Inicia-se assim um frenético jogo de gato e rato entre Light e seu perseguidor implacável , enquanto Ryuk diverte-se com os acontecimentos que se desenrolam em decorrência do uso do Death Note."
      },
      "isLocked": false,
      "links": {
        "al": "30021",
        "ap": "death-note",
        "bw": "series/13024/list",
        "kt": "57",
        "mu": "41",
        "amz": "https://www.amazon.co.jp/gp/product/B07572CPGF",
        "cdj": "https://www.cdjapan.co.jp/product/NEOBK-33313",
        "ebj": "https://ebookjapan.yahoo.co.jp/books/134328/",
        "mal": "21",
        "raw": "https://shonenjumpplus.com/episode/10833519556325021815",
        "engtl": "https://www.viz.com/death-note"
      },
      "originalLanguage": "ja",
      "lastVolume": "12",
      "lastChapter": "108",
      "publicationDemographic": "shounen",
      "status": "completed",
      "year": 2003,
      "contentRating": "safe",
      "tags": [
        {
          "id": "07251805-a27e-4d59-b488-f0bfbec15168",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Thriller"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "391b0423-d847-456f-aff0-8b0cfc03066b",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Action"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "39730448-9a5f-48a2-85b0-a70db87b1233",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Demons"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "3b60b75c-a2d7-4860-ab56-05f391bb889c",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Psychological"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "423e2eae-a7a2-4a8b-ac03-a8351462d71d",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Romance"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "5ca48985-9a9d-4bd8-be29-80dc0303db72",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Crime"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "5fff9cde-849c-4d78-aab0-0d52b2ee1d25",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Survival"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "92d6d951-ca5e-429c-ac78-451071cbf064",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Office Workers"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "b29d6a3d-1569-4e7a-8caf-7557bc92cd5d",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Gore"
            },
            "description": {},
            "group": "content",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "b9af3a63-f058-46de-a9a0-e0c13906197a",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Drama"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "caaa44eb-cd40-4177-b930-79d3ef2afe87",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "School Life"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "cdad7e68-1419-41dd-bdce-27753074a640",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Horror"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "cdc58593-87dd-415e-bbc0-2ec27bf404cc",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Fantasy"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "df33b754-73a3-4c54-80e6-1a74a8058539",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Police"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "eabc5b4c-6aff-42f3-b657-3e90cbd00b75",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Supernatural"
            },
            "description": {},
            "group": "theme",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "ee968100-4191-4968-93d3-f82d72be7e46",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Mystery"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        },
        {
          "id": "f8f62932-27da-4fe4-8ee1-6779a8c5edba",
          "type": "tag",
          "attributes": {
            "name": {
              "en": "Tragedy"
            },
            "description": {},
            "group": "genre",
            "version": 1
          },
          "relationships": []
        }
      ],
      "state": "published",
      "chapterNumbersResetOnNewVolume": false,
      "createdAt": "2018-01-19T17:09:33+00:00",
      "updatedAt": "2025-01-20T05:18:28+00:00",
      "version": 27,
      "availableTranslatedLanguages": [
        "pt-br",
        "en",
        "eu",
        "es",
        "cv",
        "tr",
        "kk"
      ],
      "latestUploadedChapter": "08e35702-f860-4f6a-817e-d857ff46f5e2"
    },
    "relationships": [
      {
        "id": "0669bf79-ca27-4f50-9b48-741fb235137f",
        "type": "author",
        "attributes": {
          "name": "Ohba Tsugumi",
          "imageUrl": null,
          "biography": {},
          "twitter": null,
          "pixiv": null,
          "melonBook": null,
          "fanBox": null,
          "booth": null,
          "namicomi": null,
          "nicoVideo": null,
          "skeb": null,
          "fantia": null,
          "tumblr": null,
          "youtube": null,
          "weibo": null,
          "naver": null,
          "website": null,
          "createdAt": "2021-04-19T21:59:45+00:00",
          "updatedAt": "2021-04-19T21:59:45+00:00",
          "version": 1
        }
      },
      {
        "id": "37ffda70-8f9e-4051-a020-073cae8d25a6",
        "type": "artist",
        "attributes": {
          "name": "Obata Takeshi",
          "imageUrl": null,
          "biography": {
            "en": "Takeshi Obata (小畑 健, Obata Takeshi, born February 11, 1969) is a Japanese manga artist that usually works as the illustrator in collaboration with a writer. He first gained international attention for Hikaru no Go (1998–2003) with [Yumi Hotta](https://mangadex.org/author/10e45c0d-a7d7-4f10-b56a-bd0a4445236c/hotta-yumi), but is better known for Death Note (2003–2006) and Bakuman (2008–2012) with [Tsugumi Ohba](https://mangadex.org/author/0669bf79-ca27-4f50-9b48-741fb235137f/ohba-tsugumi). Obata has mentored several well-known manga artists, including [Nobuhiro Watsuki](https://mangadex.org/author/e718ceea-6dff-4297-bd36-1cecaf077e83/watsuki-nobuhiro) of Rurouni Kenshin fame, Black Cat creator [Kentaro Yabuki](https://mangadex.org/author/a36f5f24-d009-46b5-bee3-b0ceb6a52067/yabuki-kentaro), and Eyeshield 21 artist [Yusuke Murata](https://mangadex.org/author/47cd4e57-3fc4-4d76-97e4-b3933a5b05ef/murata-yuusuke).\n\nOn September 6, 2006, Obata was arrested for illegal possession of an 8.6 cm knife when he was pulled over in Musashino, Tokyo for driving with his car's headlights off at 12:30am. The artist claimed he kept the knife in his car for when he goes camping."
          },
          "twitter": null,
          "pixiv": null,
          "melonBook": null,
          "fanBox": null,
          "booth": null,
          "namicomi": null,
          "nicoVideo": null,
          "skeb": null,
          "fantia": null,
          "tumblr": null,
          "youtube": null,
          "weibo": null,
          "naver": null,
          "website": null,
          "createdAt": "2021-04-19T21:59:45+00:00",
          "updatedAt": "2022-01-02T05:39:43+00:00",
          "version": 2
        }
      },
      {
        "id": "a3b22eeb-a853-4f21-b279-f6a93a10b3fc",
        "type": "cover_art",
        "attributes": {
          "description": "",
          "volume": "12",
          "fileName": "d6555598-8202-477d-acde-303202cb3475.jpg",
          "locale": "ja",
          "createdAt": "2021-05-23T08:08:39+00:00",
          "updatedAt": "2021-05-23T08:08:39+00:00",
          "version": 2
        }
      },
      {
        "id": "06d3bc04-4066-4318-b118-022a5b281ab5",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "1fd20767-f5fc-4228-a2fc-c88497eee318",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "25a76aba-e730-4a17-8f77-83c65a51036e",
        "type": "manga",
        "related": "sequel"
      },
      {
        "id": "36bdbf91-5e57-4b81-a813-4c557091dbce",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "3fc5c653-3fac-4ddb-aebf-f2e3d4d4adf6",
        "type": "manga",
        "related": "sequel"
      },
      {
        "id": "5520b5a8-e76a-4d61-b7ae-f33d1cf13d55",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "85b6f4e4-180b-43b0-a0ec-9a9de401aed0",
        "type": "manga",
        "related": "colored"
      },
      {
        "id": "8adaa90a-c779-47db-8a9c-c029e352ad97",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "8f8c4f0e-93f5-433e-ba13-017746b485ec",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "e5e79c84-f625-45c8-b4b6-7c53f0d6e1af",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "ee767a2a-20cb-4188-ad47-83730c43dfe8",
        "type": "manga",
        "related": "doujinshi"
      },
      {
        "id": "f27c8e31-6cd6-45ea-b1ee-2a9418fac2f1",
        "type": "manga",
        "related": "doujinshi"
      }
    ]
  }
}
        "#;

        let res: GetMangaByIdResponse = serde_json::from_str(test).unwrap();
    }
}
