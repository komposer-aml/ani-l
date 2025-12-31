use anyhow::Result;
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct PlayOptions {
    pub url: String,
    pub title: Option<String>,
    pub start_time: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    #[allow(dead_code)]
    pub subtitles: Option<Vec<String>>,
}

pub trait Player {
    fn play(&self, options: PlayOptions) -> Result<()>;
}
