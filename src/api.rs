const SEARCH_QUERY: &str = r#"
query (
  $search: String
  $perPage: Int
  $page: Int
  $sort: [MediaSort]
  $id_in: [Int]
) {
  Page(perPage: $perPage, page: $page) {
    pageInfo {
      total
      currentPage
      hasNextPage
    }
    media(
      search: $search
      id_in: $id_in
      sort: $sort
      type: ANIME
    ) {
      id
      title {
        romaji
        english
        native
      }
      coverImage {
        large
      }
      episodes
      averageScore
      genres
      description
      studios {
        nodes {
          name
        }
      }
      trailer {
        id
        site
      }
    }
  }
}
"#;

use crate::models::AniListResponse;
use anyhow::{Context, Result};
use serde_json::{Value, json};

const ANILIST_URL: &str = "https://graphql.anilist.co";

pub async fn fetch_media(variables: Value) -> Result<AniListResponse> {
    let client = reqwest::Client::new();

    let json_body = json!({
        "query": SEARCH_QUERY,
        "variables": variables
    });

    let res = client
        .post(ANILIST_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&json_body)
        .send()
        .await
        .context("Failed to send request to AniList")?;

    if !res.status().is_success() {
        let error_text = res.text().await?;
        anyhow::bail!("AniList API Error: {}", error_text);
    }

    let data: AniListResponse = res
        .json()
        .await
        .context("Failed to parse AniList response")?;
    Ok(data)
}
