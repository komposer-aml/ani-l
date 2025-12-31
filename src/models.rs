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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_media_preferred_title() {
        // Case 1: English title exists
        let m1 = Media {
            id: 1,
            title: MediaTitle {
                english: Some("Naruto".into()),
                romaji: Some("Naruto".into()),
                native: None,
            },
            cover_image: None,
            episodes: None,
            description: None,
            average_score: None,
            genres: vec![],
            studios: None,
            trailer: None,
        };
        assert_eq!(m1.preferred_title(), "Naruto");

        // Case 2: Only Romaji exists
        let m2 = Media {
            id: 2,
            title: MediaTitle {
                english: None,
                romaji: Some("Shingeki no Kyojin".into()),
                native: None,
            },
            cover_image: None,
            episodes: None,
            description: None,
            average_score: None,
            genres: vec![],
            studios: None,
            trailer: None,
        };
        assert_eq!(m2.preferred_title(), "Shingeki no Kyojin");
    }

    #[test]
    fn test_anilist_response_deserialization() {
        let json_data = json!({
            "data": {
                "Page": {
                    "pageInfo": {
                        "total": 100,
                        "currentPage": 1,
                        "hasNextPage": true
                    },
                    "media": [
                        {
                            "id": 1,
                            "title": { "english": "Cowboy Bebop" },
                            "genres": ["Action", "Sci-Fi"]
                        }
                    ]
                }
            }
        });

        let response: AniListResponse =
            serde_json::from_value(json_data).expect("Failed to deserialize mock AniList response");

        let page = response.data.page.unwrap();
        assert_eq!(page.media.len(), 1);
        assert_eq!(page.media[0].preferred_title(), "Cowboy Bebop");
    }
}
