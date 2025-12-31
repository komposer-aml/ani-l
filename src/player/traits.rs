#![allow(dead_code)]
use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type NextEpisodeResolver =
    Box<dyn Fn() -> BoxFuture<'static, Result<Option<PlayOptions>>> + Send + Sync>;

#[derive(Debug, Default, Clone)]
pub struct PlayOptions {
    pub url: String,
    pub title: Option<String>,
    pub start_time: Option<String>,
    pub headers: Option<Vec<(String, String)>>,
    pub subtitles: Option<Vec<String>>,
}

pub trait Player {
    fn play(
        &self,
        options: PlayOptions,
        next_resolver: Option<NextEpisodeResolver>,
    ) -> impl Future<Output = Result<f64>> + Send;
}
