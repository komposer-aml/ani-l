use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AniListResponse {
    pub data: Data,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Data {
    #[serde(rename = "Page")]
    pub page: Page,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Page {
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
    pub media: Vec<Media>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PageInfo {
    pub total: i32,
    #[serde(rename = "currentPage")]
    pub current_page: i32,
    #[serde(rename = "hasNextPage")]
    pub has_next_page: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Media {
    pub id: i32,
    pub title: MediaTitle,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<CoverImage>,
    pub episodes: Option<i32>,
    pub description: Option<String>,
    #[serde(rename = "averageScore")]
    pub average_score: Option<i32>,
    pub genres: Vec<String>,
    pub studios: Option<StudioConnection>,
    pub trailer: Option<Trailer>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Trailer {
    pub id: Option<String>,
    pub site: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub medium: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StudioConnection {
    pub nodes: Vec<Studio>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Studio {
    pub name: String,
}

impl Media {
    pub fn preferred_title(&self) -> &str {
        self.title
            .english
            .as_deref()
            .or(self.title.romaji.as_deref())
            .or(self.title.native.as_deref())
            .unwrap_or("Unknown Title")
    }
}
