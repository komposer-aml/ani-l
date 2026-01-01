#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AllAnimeResponse<T> {
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct SearchResultData {
    pub shows: ShowsConnection,
}

#[derive(Debug, Deserialize)]
pub struct ShowsConnection {
    pub edges: Vec<ShowEdge>,
}

#[derive(Debug, Deserialize)]
pub struct ShowEdge {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    #[serde(rename = "availableEpisodes")]
    pub available_episodes: AvailableEpisodes,
}

#[derive(Debug, Deserialize)]
pub struct AvailableEpisodes {
    pub sub: usize,
    pub dub: usize,
    pub raw: usize,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeResultData {
    // FIX: Wrapped in Option to handle null API responses gracefully
    pub episode: Option<EpisodeData>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeData {
    #[serde(rename = "sourceUrls")]
    pub source_urls: Vec<SourceUrl>,
}

#[derive(Debug, Deserialize)]
pub struct SourceUrl {
    #[serde(rename = "sourceName")]
    pub source_name: String,
    #[serde(rename = "sourceUrl")]
    pub source_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GogoStreamResponse {
    pub links: Vec<GogoLink>,
}

#[derive(Debug, Deserialize)]
pub struct GogoLink {
    pub link: String,
    #[serde(rename = "resolutionStr")]
    pub resolution: String,
}
