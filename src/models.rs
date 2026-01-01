#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AniListResponse {
    pub data: Data,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Data {
    #[serde(rename = "Page")]
    pub page: Option<Page>,
    #[serde(rename = "Viewer")]
    pub viewer: Option<User>,
    #[serde(rename = "SaveMediaListEntry")]
    pub saved_entry: Option<MediaListEntry>,
    #[serde(rename = "MediaList")]
    pub media_list: Option<MediaListEntry>,
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
    pub popularity: Option<i32>,
    pub favourites: Option<i32>,
    pub status: Option<String>,
    pub format: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<FuzzyDate>,
    #[serde(rename = "endDate")]
    pub end_date: Option<FuzzyDate>,
    pub synonyms: Option<Vec<String>>,
    pub tags: Option<Vec<MediaTag>>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaListEntry {
    pub id: Option<i32>,
    #[serde(rename = "mediaId")]
    pub media_id: Option<i32>,
    pub status: Option<String>,
    pub progress: Option<i32>,
    pub score: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FuzzyDate {
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaTag {
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

    pub fn formatted_start_date(&self) -> String {
        self.start_date
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    pub fn formatted_end_date(&self) -> String {
        self.end_date
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "?".to_string())
    }
}

impl ToString for FuzzyDate {
    fn to_string(&self) -> String {
        match (self.year, self.month, self.day) {
            (Some(y), Some(m), Some(d)) => format!("{:04}-{:02}-{:02}", y, m, d),
            (Some(y), Some(m), None) => format!("{:04}-{:02}", y, m),
            (Some(y), None, None) => format!("{:04}", y),
            _ => "?".to_string(),
        }
    }
}
