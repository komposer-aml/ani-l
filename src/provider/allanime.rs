use crate::player::traits::PlayOptions;
use crate::provider::models::*;
use anyhow::{Context, Result};
use reqwest::{Client, header};
use serde_json::json;
use urlencoding::encode;

const API_ENDPOINT: &str = "https://api.allanime.day/api";
const REFERER: &str = "https://allanime.to/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct AllAnimeProvider {
    client: Client,
}

impl AllAnimeProvider {
    pub fn new() -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::REFERER, header::HeaderValue::from_static(REFERER));
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(USER_AGENT),
        );

        let client = Client::builder().default_headers(headers).build().unwrap();
        Self { client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<ShowEdge>> {
        let gql = r#"
        query($search: SearchInput, $limit: Int, $page: Int, $translationType: VaildTranslationTypeEnumType, $countryOrigin: VaildCountryOriginEnumType) {
            shows(search: $search, limit: $limit, page: $page, translationType: $translationType, countryOrigin: $countryOrigin) {
                edges {
                    _id
                    name
                    availableEpisodes
                }
            }
        }
        "#;

        let variables = json!({
            "search": {
                "allowAdult": false,
                "allowUnknown": false,
                "query": query
            },
            "limit": 5,
            "page": 1,
            "translationType": "sub",
            "countryOrigin": "ALL"
        });

        let url = format!(
            "{}?variables={}&query={}",
            API_ENDPOINT,
            encode(&variables.to_string()),
            encode(gql)
        );

        let resp: AllAnimeResponse<SearchResultData> =
            self.client.get(&url).send().await?.json().await?;
        Ok(resp.data.shows.edges)
    }

    pub async fn get_episode_sources(
        &self,
        show_id: &str,
        episode_num: &str,
    ) -> Result<Vec<SourceUrl>> {
        let gql = r#"
        query($showId: String!, $translationType: VaildTranslationTypeEnumType!, $episodeString: String!) {
            episode(showId: $showId, translationType: $translationType, episodeString: $episodeString) {
                sourceUrls
            }
        }
        "#;

        let variables = json!({
            "showId": show_id,
            "translationType": "sub",
            "episodeString": episode_num
        });

        let url = format!(
            "{}?variables={}&query={}",
            API_ENDPOINT,
            encode(&variables.to_string()),
            encode(gql)
        );

        let resp: AllAnimeResponse<EpisodeResultData> =
            self.client.get(&url).send().await?.json().await?;
        Ok(resp.data.episode.source_urls)
    }

    pub async fn extract_clock_stream(&self, source_url: &str) -> Result<PlayOptions> {
        let clean_url = if let Some(stripped) = source_url.strip_prefix("--") {
            decrypt_source_url(stripped)?
        } else {
            source_url.to_string()
        };

        let base_path = if clean_url.starts_with("/") {
            clean_url
        } else {
            format!("/{}", clean_url)
        };

        let clock_url = format!(
            "https://allanime.day{}",
            base_path.replace("clock", "clock.json")
        );

        let resp: GogoStreamResponse = self.client.get(&clock_url).send().await?.json().await?;

        let best_link = resp
            .links
            .iter()
            .find(|l| l.resolution == "1080p")
            .or(resp.links.last())
            .context("No stream links found")?;

        let headers = vec![
            ("User-Agent".to_string(), USER_AGENT.to_string()),
            ("Referer".to_string(), "https://allanime.day/".to_string()),
        ];

        Ok(PlayOptions {
            url: best_link.link.clone(),
            title: Some("Anime Stream".to_string()),
            start_time: None,
            headers: Some(headers),
            subtitles: None,
        })
    }
}

fn decrypt_source_url(hex_string: &str) -> Result<String> {
    let password = 56u8;
    let mut decoded = String::new();

    let bytes = (0..hex_string.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_string[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .context("Failed to parse hex string")?;

    for b in bytes {
        let decrypted_byte = b ^ password;
        decoded.push(decrypted_byte as char);
    }

    Ok(decoded)
}
