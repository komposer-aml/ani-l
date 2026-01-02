mod api;
mod config;
mod models;
mod normalizer;
mod player;
mod provider;
mod registry;
mod tui;

#[macro_use]
extern crate rust_i18n;

i18n!("locales");

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use strsim::normalized_levenshtein;

use crate::config::ConfigManager;
use crate::player::traits::{EpisodeAction, EpisodeNavigator, PlayOptions, Player};
use crate::provider::allanime::AllAnimeProvider;
use crate::registry::RegistryManager;
use crate::tui::app::{Action, App, Focus, ListMode};

#[derive(Parser)]
#[command(name = "ani-l")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Tui,
    Auth {
        #[arg(required = false)]
        token_input: Option<String>,
        #[arg(long, short)]
        logout: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();

    let mut config_manager = ConfigManager::init_interactive().await?;
    let _registry_manager = RegistryManager::new()?;
    rust_i18n::set_locale(&config_manager.config.general.language);

    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Tui => run_tui(config_manager).await?,
        Commands::Auth {
            token_input,
            logout,
        } => {
            if logout {
                config_manager.auth.anilist_token = None;
                config_manager.auth.username = None;
                config_manager.save_auth()?;
                println!("✅ Logged out successfully.");
            } else if let Some(input) = token_input {
                config_manager.verify_and_save_token(&input).await?;
            } else {
                config_manager.authenticate_interactive().await?;
            }
        }
    }

    Ok(())
}

async fn run_tui(config_manager: ConfigManager) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config_manager);
    app.init_image_picker();

    if app.config_manager.config.general.check_updates {
        let tx = app.action_tx.clone();
        tokio::spawn(async move {
            if let Ok(Some(version)) = api::check_for_updates().await {
                let _ = tx.send(Action::UpdateAvailable(version));
            }
        });
    }

    loop {
        terminal.draw(|f| tui::ui::draw(f, &mut app))?;

        let mut input_event = None;
        if crossterm::event::poll(Duration::from_millis(16))? {
            input_event = Some(crossterm::event::read()?);
        }

        if let Some(Event::Key(key)) = input_event {
            if key.kind == event::KeyEventKind::Press {
                if app.show_update_modal {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            app.show_update_modal = false;
                        }
                        _ => {}
                    }
                } else {
                    match app.focus {
                        Focus::SearchBar => match key.code {
                            KeyCode::Char('/') => app.action_tx.send(Action::ToggleFocus)?,
                            KeyCode::Enter => {
                                if !app.search_query.is_empty() {
                                    app.action_tx.send(Action::SearchStarted)?;
                                    let query = app.search_query.clone();
                                    let tx = app.action_tx.clone();
                                    tokio::spawn(async move {
                                        match api::fetch_media(serde_json::json!({
                                            "search": query, "perPage": 20, "sort": "POPULARITY_DESC"
                                        })).await {
                                            Ok(res) => {
                                                if let Some(page) = res.data.page {
                                                    let _ = tx.send(Action::SearchCompleted(page.media, None));
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx.send(Action::SearchError(e.to_string()));
                                            }
                                        }
                                    });
                                }
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                            }
                            KeyCode::Esc => {
                                app.focus = Focus::List;
                            }
                            _ => {}
                        },
                        Focus::List => match key.code {
                            KeyCode::Char('q') => app.action_tx.send(Action::Quit)?,
                            KeyCode::Char('/') => app.action_tx.send(Action::ToggleFocus)?,
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.action_tx.send(Action::NavigateDown)?
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.action_tx.send(Action::NavigateUp)?
                            }
                            KeyCode::PageDown | KeyCode::Char('J') => {
                                app.action_tx.send(Action::NavigatePageDown)?
                            }
                            KeyCode::PageUp | KeyCode::Char('K') => {
                                app.action_tx.send(Action::NavigatePageUp)?
                            }
                            KeyCode::Enter => app.action_tx.send(Action::Select)?,
                            KeyCode::Esc => app.action_tx.send(Action::GoBack)?,
                            KeyCode::Backspace => app.action_tx.send(Action::GoBack)?,
                            _ => {}
                        },
                    }
                }
            }
        } else {
            app.action_tx.send(Action::Tick)?;
        }

        while let Ok(action) = app.action_rx.try_recv() {
            match action {
                Action::Tick => app.on_tick(),
                Action::Quit => app.running = false,
                Action::ToggleFocus => {
                    app.focus = match app.focus {
                        Focus::List => Focus::SearchBar,
                        Focus::SearchBar => Focus::List,
                    };
                }
                Action::NavigateDown => {
                    app.next();
                    update_preview(&mut app);
                }
                Action::NavigateUp => {
                    app.previous();
                    update_preview(&mut app);
                }
                Action::NavigatePageDown => {
                    app.jump_forward(10);
                    update_preview(&mut app);
                }
                Action::NavigatePageUp => {
                    app.jump_backward(10);
                    update_preview(&mut app);
                }
                Action::GoBack => app.go_back(),
                Action::SearchStarted => {
                    app.is_loading = true;
                    app.status_message = Some(t!("status.searching").to_string());
                }
                Action::SearchCompleted(media, title_opt) => {
                    app.is_loading = false;
                    app.status_message = None;
                    app.media_list = media;
                    if let Some(title) = title_opt {
                        app.go_to_mode(ListMode::AnimeList(title), true);
                    } else {
                        app.go_to_mode(ListMode::SearchResults, true);
                    }
                    app.focus = Focus::List;
                    app.active_media = None;
                    update_preview(&mut app);
                }
                Action::SearchError(err) => {
                    app.is_loading = false;
                    app.status_message = Some(err);
                }
                Action::UpdateAvailable(version) => {
                    app.new_version = Some(version);
                    app.show_update_modal = true;
                }
                Action::StreamStarted => {
                    app.go_to_mode(ListMode::StreamLogging, false);
                    app.log_stream(t!("logs.starting_process").to_string());
                }
                Action::StreamLog(msg) => {
                    app.log_stream(msg);
                }
                Action::StreamFinished => {
                    app.go_back();
                    terminal.clear()?;
                }
                Action::Select => handle_selection(&mut app)?,
                Action::ImageLoaded(bytes) => {
                    if let Some(picker) = &mut app.image_picker
                        && let Ok(img) = image::load_from_memory(&bytes)
                    {
                        let protocol = picker.new_resize_protocol(img);
                        app.current_cover_image = Some(protocol);
                    }
                    app.is_fetching_image = false;
                }
            }
        }

        if !app.running {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn update_preview(app: &mut App) {
    if matches!(
        app.list_mode,
        ListMode::SearchResults | ListMode::AnimeList(_)
    ) {
        let idx = app.get_selected_index();
        if idx < app.media_list.len() {
            let media = app.media_list[idx].clone();
            if app.active_media.as_ref().map(|m| m.id) != Some(media.id) {
                app.active_media = Some(media.clone());
                app.current_cover_image = None;

                if let Some(cover) = media.cover_image {
                    let url_opt = cover.extra_large.or(cover.large).or(cover.medium);
                    if let Some(url) = url_opt {
                        app.is_fetching_image = true;
                        let tx = app.action_tx.clone();
                        tokio::task::spawn_blocking(move || {
                            if let Ok(resp) = reqwest::blocking::get(url)
                                && let Ok(bytes) = resp.bytes()
                            {
                                let _ = tx.send(Action::ImageLoaded(bytes.to_vec()));
                            }
                        });
                    }
                }
            }
        }
    }
}

fn handle_selection(app: &mut App) -> Result<()> {
    match app.list_mode.clone() {
        ListMode::MainMenu => {
            let idx = app.get_selected_index();
            if idx < app.main_menu_items.len() {
                let item = &app.main_menu_items[idx];
                if item == &t!("main_menu.exit") {
                    app.running = false;
                } else if item == &t!("main_menu.trending") {
                    app.action_tx.send(Action::SearchStarted)?;
                    let tx = app.action_tx.clone();
                    tokio::spawn(async move {
                        match api::fetch_media(
                            serde_json::json!({ "perPage": 20, "sort": "TRENDING_DESC" }),
                        )
                        .await
                        {
                            Ok(res) => {
                                if let Some(p) = res.data.page {
                                    let _ = tx.send(Action::SearchCompleted(
                                        p.media,
                                        Some(t!("main_menu.trending").to_string()),
                                    ));
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Action::SearchError(e.to_string()));
                            }
                        }
                    });
                } else if item == &t!("main_menu.popular") {
                    app.action_tx.send(Action::SearchStarted)?;
                    let tx = app.action_tx.clone();
                    tokio::spawn(async move {
                        match api::fetch_media(
                            serde_json::json!({ "perPage": 20, "sort": "POPULARITY_DESC" }),
                        )
                        .await
                        {
                            Ok(res) => {
                                if let Some(p) = res.data.page {
                                    let _ = tx.send(Action::SearchCompleted(
                                        p.media,
                                        Some(t!("main_menu.popular").to_string()),
                                    ));
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Action::SearchError(e.to_string()));
                            }
                        }
                    });
                } else if item == &t!("main_menu.options") {
                    app.go_to_mode(ListMode::Options, true);
                }
            }
        }
        ListMode::SearchResults | ListMode::AnimeList(_) => {
            let idx = app.get_selected_index();
            if idx < app.media_list.len() {
                app.active_media = Some(app.media_list[idx].clone());
                app.go_to_mode(ListMode::AnimeActions, true);
            }
        }
        ListMode::AnimeActions => {
            let idx = app.get_selected_index();
            if idx < app.anime_action_items.len() {
                let action = &app.anime_action_items[idx];
                if action == &t!("actions.stream") {
                    if let Some(media) = app.active_media.clone() {
                        start_stream_task(app, media, "1".to_string());
                    }
                } else if action == &t!("actions.episodes") {
                    app.go_to_mode(ListMode::EpisodeSelect, true);
                } else {
                    app.go_to_mode(ListMode::SubMenu(action.clone()), true);
                }
            }
        }
        ListMode::EpisodeSelect => {
            let ep_num = (app.get_selected_index() + 1).to_string();
            if let Some(media) = app.active_media.clone() {
                start_stream_task(app, media, ep_num);
            }
        }
        ListMode::Options => {
            let idx = app.get_selected_index();
            match idx {
                0 => {
                    let qualities = ["1080", "720", "480"];
                    let current = app.config_manager.config.stream.quality.as_str();
                    if let Some(pos) = qualities.iter().position(|&q| q == current) {
                        let next = (pos + 1) % qualities.len();
                        app.config_manager.config.stream.quality = qualities[next].to_string();
                    }
                }
                1 => {
                    let types = ["sub", "dub"];
                    let current = app.config_manager.config.stream.translation_type.as_str();
                    if let Some(pos) = types.iter().position(|&t| t == current) {
                        let next = (pos + 1) % types.len();
                        app.config_manager.config.stream.translation_type = types[next].to_string();
                    }
                }
                2 => {
                    let langs = ["en", "es", "pt", "fr", "id", "ru"];
                    let current = app.config_manager.config.general.language.as_str();
                    if let Some(pos) = langs.iter().position(|&l| l == current) {
                        let next = (pos + 1) % langs.len();
                        app.config_manager.config.general.language = langs[next].to_string();
                        rust_i18n::set_locale(&app.config_manager.config.general.language);
                        app.update_localized_items();
                    }
                }
                _ => {}
            }
            app.config_manager.save_config()?;
        }
        _ => {}
    }
    Ok(())
}

async fn resolve_stream_for_episode(
    provider: &AllAnimeProvider,
    show_id: &str,
    show_name: &str,
    episode: &str,
) -> Result<Option<PlayOptions>> {
    let sources = provider.get_episode_sources(show_id, episode).await?;
    let priorities = ["S-mp4", "Luf-mp4", "Luf-Mp4", "Sak", "Default", "Yt-mp4"];

    for source_name in priorities {
        if let Some(source) = sources.iter().find(|s| s.source_name == source_name) {
            match provider.extract_clock_stream(&source.source_url).await {
                Ok(mut options) => {
                    options.title = Some(format!("{} - Episode {}", show_name, episode));
                    return Ok(Some(options));
                }
                Err(_) => continue,
            }
        }
    }
    Ok(None)
}

fn start_stream_task(app: &App, media: crate::models::Media, episode: String) {
    let tx = app.action_tx.clone();
    let config = app.config_manager.clone();

    let _ = tx.send(Action::StreamStarted);

    tokio::spawn(async move {
        let query = media.preferred_title();
        let _ = tx.send(Action::StreamLog(
            t!("logs.searching_provider", query = query).to_string(),
        ));

        let provider = Arc::new(crate::provider::allanime::AllAnimeProvider::new(
            config.config.stream.translation_type.clone(),
        ));

        match provider.search(query).await {
            Ok(results) => {
                let best_match = results.iter().max_by(|a, b| {
                    let name_a = normalizer::normalize("allanime", &a.name);
                    let name_b = normalizer::normalize("allanime", &b.name);
                    let score_a =
                        normalized_levenshtein(&name_a.to_lowercase(), &query.to_lowercase());
                    let score_b =
                        normalized_levenshtein(&name_b.to_lowercase(), &query.to_lowercase());
                    score_a
                        .partial_cmp(&score_b)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                if let Some(show) = best_match {
                    let _ = tx.send(Action::StreamLog(
                        t!("logs.found", name = show.name, id = show.id).to_string(),
                    ));

                    let show_id = show.id.clone();
                    let show_name = show.name.clone();

                    let _ = tx.send(Action::StreamLog(
                        t!("logs.fetching_episode", ep = episode).to_string(),
                    ));

                    match resolve_stream_for_episode(&provider, &show_id, &show_name, &episode)
                        .await
                    {
                        Ok(Some(options)) => {
                            let _ = tx.send(Action::StreamLog(t!("logs.stream_found").to_string()));

                            let current_ep_num = Arc::new(tokio::sync::Mutex::new(
                                episode.parse::<i32>().unwrap_or(1),
                            ));
                            let provider_clone = provider.clone();
                            let s_id = show_id.clone();
                            let s_name = show_name.clone();

                            let navigator: EpisodeNavigator = {
                                let ep_store = current_ep_num.clone();
                                Box::new(move |action| {
                                    let p = provider_clone.clone();
                                    let s_id = s_id.clone();
                                    let s_name = s_name.clone();
                                    let ep_store = ep_store.clone();
                                    Box::pin(async move {
                                        let mut num = ep_store.lock().await;
                                        match action {
                                            EpisodeAction::Next => *num += 1,
                                            EpisodeAction::Previous => {
                                                if *num > 1 {
                                                    *num -= 1;
                                                } else {
                                                    return Ok(None);
                                                }
                                            }
                                        }
                                        resolve_stream_for_episode(
                                            &p,
                                            &s_id,
                                            &s_name,
                                            &num.to_string(),
                                        )
                                        .await
                                    })
                                })
                            };

                            let player = crate::player::mpv::MpvPlayer;
                            match player.play(options, Some(navigator)).await {
                                Ok(percentage) => {
                                    let _ = tx.send(Action::StreamLog(
                                        t!("logs.finished", prog = format!("{:.1}", percentage))
                                            .to_string(),
                                    ));

                                    let final_ep_num = *current_ep_num.lock().await;
                                    let required_percentage =
                                        config.config.stream.episode_complete_at as f64;

                                    if percentage >= required_percentage
                                        && let (Some(token), Some(username)) =
                                            (&config.auth.anilist_token, &config.auth.username)
                                    {
                                        let _ = tx.send(Action::StreamLog(
                                            t!("logs.updating_anilist").to_string(),
                                        ));
                                        match api::get_user_progress(token, media.id, username)
                                            .await
                                        {
                                            Ok(current_progress) => {
                                                let prog = current_progress.unwrap_or(0);
                                                if final_ep_num > prog {
                                                    if let Err(e) = api::update_user_entry(
                                                        token,
                                                        media.id,
                                                        final_ep_num,
                                                        "CURRENT",
                                                    )
                                                    .await
                                                    {
                                                        let _ = tx.send(Action::StreamLog(
                                                            t!("logs.update_failed", err = e)
                                                                .to_string(),
                                                        ));
                                                    } else {
                                                        let _ = tx.send(Action::StreamLog(
                                                            t!(
                                                                "logs.updated_to_ep",
                                                                ep = final_ep_num
                                                            )
                                                            .to_string(),
                                                        ));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let _ = tx.send(Action::StreamLog(
                                                    t!("logs.sync_error", err = e).to_string(),
                                                ));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::StreamLog(
                                        t!("logs.player_error", err = e).to_string(),
                                    ));
                                }
                            }
                        }
                        Ok(None) => {
                            let _ = tx.send(Action::StreamLog(t!("logs.no_stream").to_string()));
                        }
                        Err(e) => {
                            let _ = tx.send(Action::StreamLog(
                                t!("logs.source_error", err = e).to_string(),
                            ));
                        }
                    }
                } else {
                    let _ = tx.send(Action::StreamLog(t!("logs.no_results").to_string()));
                }
            }
            Err(e) => {
                let _ = tx.send(Action::StreamLog(
                    t!("logs.search_error", err = e).to_string(),
                ));
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = tx.send(Action::StreamFinished);
    });
}
