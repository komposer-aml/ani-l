use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy)]
pub enum EpisodeAction {
    Next,
    Previous,
}

pub type EpisodeNavigator =
    Box<dyn Fn(EpisodeAction) -> BoxFuture<'static, Result<Option<PlayOptions>>> + Send + Sync>;

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
        navigator: Option<EpisodeNavigator>,
    ) -> impl Future<Output = Result<f64>> + Send;
}
