use crate::models::{AniListResponse, MediaListEntry, User};
use anyhow::{Context, Result};
use serde_json::{Value, json};

const ANILIST_URL: &str = "https://graphql.anilist.co";

const SEARCH_QUERY: &str = r#"
query ($search: String, $perPage: Int, $page: Int, $sort: [MediaSort], $id_in: [Int]) {
  Page(perPage: $perPage, page: $page) {
    pageInfo { total, currentPage, hasNextPage }
    media(search: $search, id_in: $id_in, sort: $sort, type: ANIME) {
      id
      title { romaji, english, native }
      coverImage { extraLarge large medium }
      episodes
      averageScore
      popularity
      favourites
      status
      format
      genres
      description
      studios { nodes { name } }
      trailer { id, site }
      startDate { year month day }
      endDate { year month day }
      synonyms
      tags { name }
    }
  }
}
"#;

const VIEWER_QUERY: &str = r#"
query {
  Viewer {
    id
    name
  }
}
"#;

const SAVE_PROGRESS_MUTATION: &str = r#"
mutation ($mediaId: Int, $progress: Int, $status: MediaListStatus) {
  SaveMediaListEntry(mediaId: $mediaId, progress: $progress, status: $status) {
    id
    mediaId
    status
    progress
    score
  }
}
"#;

const GET_PROGRESS_QUERY: &str = r#"
query ($mediaId: Int, $userName: String) {
  MediaList(mediaId: $mediaId, userName: $userName, type: ANIME) {
    progress
    status
  }
}
"#;

pub async fn fetch_media(variables: Value) -> Result<AniListResponse> {
    send_request(SEARCH_QUERY, variables, None).await
}

pub async fn authenticate_user(token: &str) -> Result<User> {
    let response = send_request(VIEWER_QUERY, json!({}), Some(token)).await?;
    response
        .data
        .viewer
        .context("No Viewer data found in response")
}

pub async fn update_user_entry(
    token: &str,
    media_id: i32,
    progress: i32,
    status: &str,
) -> Result<MediaListEntry> {
    let variables = json!({
        "mediaId": media_id,
        "progress": progress,
        "status": status
    });
    let response = send_request(SAVE_PROGRESS_MUTATION, variables, Some(token)).await?;
    response.data.saved_entry.context("Failed to save entry")
}

pub async fn get_user_progress(token: &str, media_id: i32, username: &str) -> Result<Option<i32>> {
    let variables = json!({
        "mediaId": media_id,
        "userName": username
    });

    let client = reqwest::Client::new();
    let json_body = json!({ "query": GET_PROGRESS_QUERY, "variables": variables });

    let res = client
        .post(ANILIST_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .json(&json_body)
        .send()
        .await?;

    if !res.status().is_success() {
        return Ok(None);
    }

    let body_text = res.text().await?;
    if body_text.contains("\"errors\"") && body_text.contains("Not Found") {
        return Ok(None);
    }

    let data: AniListResponse = serde_json::from_str(&body_text)?;
    Ok(data.data.media_list.and_then(|entry| entry.progress))
}

async fn send_request(
    query: &str,
    variables: Value,
    token: Option<&str>,
) -> Result<AniListResponse> {
    let client = reqwest::Client::new();
    let mut req = client
        .post(ANILIST_URL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json");

    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }

    let json_body = json!({ "query": query, "variables": variables });
    let res = req
        .json(&json_body)
        .send()
        .await
        .context("Failed to send request")?;

    if !res.status().is_success() {
        anyhow::bail!("API Error: {}", res.text().await?);
    }

    res.json().await.context("Failed to parse response")
}
