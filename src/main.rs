mod api;
mod config;
mod models;
mod player;
mod provider;
mod registry;
mod tui;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use directories::ProjectDirs;
use rand::seq::SliceRandom;
use rand::thread_rng;
use ratatui::{Terminal, backend::CrosstermBackend};
use serde_json::json;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::config::ConfigManager;
use crate::player::traits::{EpisodeAction, EpisodeNavigator, PlayOptions, Player};
use crate::provider::allanime::AllAnimeProvider;
use crate::registry::RegistryManager;
use crate::tui::app::{App, Focus, ListMode};
use crate::tui::events::TuiEvent;

const ANILIST_AUTH_URL: &str =
    "https://anilist.co/api/v2/oauth/authorize?client_id=33837&response_type=token";

#[derive(Parser)]
#[command(name = "ani-l")]
#[command(about = "Terminal Anime Library & Streamer", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Search {
        #[command(subcommand)]
        mode: SearchMode,
        #[arg(long, default_value_t = 1)]
        page: i32,
        #[arg(long, default_value_t = 10)]
        per_page: i32,
    },
    Play {
        #[arg(short, long)]
        url: String,
        #[arg(short, long)]
        title: Option<String>,
    },
    Watch {
        #[arg(short, long)]
        query: String,
        #[arg(short, long, default_value = "1")]
        episode: String,
    },
    Auth {
        #[arg(required = false)]
        token_input: Option<String>,
        #[arg(long, short)]
        logout: bool,
    },
    Tui,
}

#[derive(Subcommand)]
enum SearchMode {
    Query {
        #[arg(short, long)]
        text: String,
    },
    Trending,
    Popular,
    Random,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config_manager = ConfigManager::new()?;
    let _registry_manager = RegistryManager::new()?;

    if let Some(proj_dirs) = ProjectDirs::from("com", "sleepy-foundry", "ani-l")
        && std::env::args().len() > 1
        && !std::env::args().any(|a| a == "tui")
    {
        println!(
            "📂 Configuration & Registry loaded from: {:?}",
            proj_dirs.config_dir()
        );
    }

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Tui);

    match command {
        Commands::Auth {
            token_input,
            logout,
        } => {
            if logout {
                config_manager.auth.anilist_token = None;
                config_manager.auth.username = None;
                config_manager.save_auth()?;
                println!("✅ Logged out successfully.");
                return Ok(());
            }

            let token_to_verify = if let Some(input) = token_input {
                let path = Path::new(&input);
                if path.exists() && path.is_file() {
                    println!("📂 Reading token from file: {:?}", path);
                    std::fs::read_to_string(path)?.trim().to_string()
                } else {
                    input
                }
            } else {
                println!("🌍 Opening browser for authentication...");
                println!("🔗 If it doesn't open, visit: {}", ANILIST_AUTH_URL);

                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open")
                    .arg(ANILIST_AUTH_URL)
                    .spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open")
                    .arg(ANILIST_AUTH_URL)
                    .spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .arg("/C")
                    .arg("start")
                    .arg(ANILIST_AUTH_URL)
                    .spawn();

                print!("🔑 Paste your token here: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                input.trim().to_string()
            };

            if token_to_verify.is_empty() {
                println!("❌ No token provided.");
                return Ok(());
            }

            println!("🔄 Verifying token...");
            match api::authenticate_user(&token_to_verify).await {
                Ok(user) => {
                    println!("✅ Successfully logged in as: {}", user.name);
                    config_manager.auth.anilist_token = Some(token_to_verify);
                    config_manager.auth.username = Some(user.name);
                    config_manager.save_auth()?;
                }
                Err(e) => {
                    eprintln!("❌ Authentication failed: {}", e);
                }
            }
        }
        Commands::Search {
            mode,
            page,
            per_page,
        } => {
            let variables = match mode {
                SearchMode::Query { text } => {
                    println!("🔍 Searching for '{}' (Page {})...", text, page);
                    json!({ "search": text, "page": page, "perPage": per_page, "sort": "POPULARITY_DESC" })
                }
                SearchMode::Trending => {
                    println!("🔥 Fetching Trending Anime (Page {})...", page);
                    json!({ "page": page, "perPage": per_page, "sort": "TRENDING_DESC" })
                }
                SearchMode::Popular => {
                    println!("✨ Fetching Popular Anime (Page {})...", page);
                    json!({ "page": page, "perPage": per_page, "sort": "POPULARITY_DESC" })
                }
                SearchMode::Random => {
                    println!("🎲 Fetching Random Anime...");
                    let buffer_size = 50;
                    let mut rng = thread_rng();
                    let range: Vec<i32> = (1..18000).collect();
                    let random_ids: Vec<i32> = range
                        .choose_multiple(&mut rng, buffer_size)
                        .cloned()
                        .collect();
                    json!({ "id_in": random_ids, "perPage": buffer_size })
                }
            };

            let response = api::fetch_media(variables).await?;
            if let Some(page) = response.data.page {
                let media_list = page.media;

                if media_list.is_empty() {
                    println!("No results found.");
                    return Ok(());
                }

                let display_count = per_page as usize;
                for (i, media) in media_list.iter().take(display_count).enumerate() {
                    let title = media.preferred_title();
                    let score = media
                        .average_score
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "N/A".to_string());
                    let episodes = media
                        .episodes
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    println!("\n{}. {} (Score: {}%)", i + 1, title, score);
                    println!("   Episodes: {} | ID: {}", episodes, media.id);
                }
            }
        }
        Commands::Play { url, title } => {
            let player = crate::player::mpv::MpvPlayer;
            let options = crate::player::traits::PlayOptions {
                url,
                title,
                ..Default::default()
            };
            // Interactive player via CLI doesn't support next/prev logic yet
            if let Err(e) = player.play(options, None).await {
                eprintln!("❌ Playback failed: {}", e);
            }
        }
        Commands::Watch { query, episode } => {
            perform_watch(query, episode, None, &config_manager).await?;
        }
        Commands::Tui => {
            run_tui(config_manager).await?;
        }
    }

    Ok(())
}

async fn run_tui(config: ConfigManager) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    loop {
        terminal.draw(|f| tui::ui::draw(f, &mut app))?;

        match tui::events::handle_input()? {
            TuiEvent::Tick => {
                app.on_tick();
            }
            TuiEvent::Quit => {
                if matches!(app.list_mode, ListMode::MainMenu) {
                    app.running = false;
                } else {
                    app.reset_to_main_menu();
                }
            }
            TuiEvent::Key(code) => {
                use crossterm::event::KeyCode;

                if code == KeyCode::Char('/') {
                    let current_focus = app.focus.clone();
                    match current_focus {
                        Focus::List => app.focus = Focus::SearchBar,
                        Focus::SearchBar => app.focus = Focus::List,
                    }
                    continue;
                }

                let is_back_key = matches!(code, KeyCode::Esc)
                    || (matches!(code, KeyCode::Backspace) && app.focus == Focus::List);
                if is_back_key {
                    app.go_back();
                    continue;
                }

                let current_focus = app.focus.clone();
                match current_focus {
                    Focus::SearchBar => match code {
                        KeyCode::Char(c) => app.search_query.push(c),
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        KeyCode::Enter => {
                            if !app.search_query.is_empty() {
                                let q = app.search_query.clone();
                                app.set_status(format!("Searching for '{}'...", q));
                                app.is_loading = true;
                                terminal.draw(|f| tui::ui::draw(f, &mut app))?;

                                if let Ok(res) = api::fetch_media(json!({
                                    "search": q, "perPage": 20, "sort": "POPULARITY_DESC"
                                }))
                                .await
                                {
                                    if let Some(page) = res.data.page {
                                        app.media_list = page.media;
                                        app.go_to_mode(ListMode::SearchResults, true);
                                        app.active_media = app.media_list.first().cloned();
                                        app.focus = Focus::List;
                                        app.clear_status();
                                    }
                                } else {
                                    app.set_status("Search failed.");
                                }
                                app.is_loading = false;
                            }
                        }
                        _ => {}
                    },
                    Focus::List => match code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.next();
                            update_preview(&mut app);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.previous();
                            update_preview(&mut app);
                        }
                        KeyCode::Char('J') | KeyCode::PageDown => {
                            app.jump_forward(10);
                            update_preview(&mut app);
                        }
                        KeyCode::Char('K') | KeyCode::PageUp => {
                            app.jump_backward(10);
                            update_preview(&mut app);
                        }
                        KeyCode::Char('h') => {
                            app.reset_to_main_menu();
                        }
                        KeyCode::Enter => {
                            handle_enter(&mut app, &mut terminal, &config).await;
                        }
                        _ => {}
                    },
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
            app.active_media = Some(app.media_list[idx].clone());
        }
    }
}

async fn handle_enter<B: ratatui::backend::Backend + std::io::Write>(
    app: &mut App,
    terminal: &mut Terminal<B>,
    config: &ConfigManager,
) {
    let current_mode = app.list_mode.clone();
    match current_mode {
        ListMode::MainMenu => {
            let idx = app.get_selected_index();
            if idx >= app.main_menu_items.len() {
                return;
            }
            let item = app.main_menu_items[idx];
            app.set_status(format!("Loading {}...", item));
            let _ = terminal.draw(|f| tui::ui::draw(f, app));

            match item {
                "❌ Exit" => app.running = false,
                "🔥 Trending" => {
                    if let Ok(res) =
                        api::fetch_media(json!({ "perPage": 20, "sort": "TRENDING_DESC" })).await
                        && let Some(page) = res.data.page
                    {
                        app.media_list = page.media;
                        app.active_media = app.media_list.first().cloned();
                        app.go_to_mode(ListMode::AnimeList("Trending".into()), true);
                    }
                }
                "✨ Popular" => {
                    if let Ok(res) =
                        api::fetch_media(json!({ "perPage": 20, "sort": "POPULARITY_DESC" })).await
                        && let Some(page) = res.data.page
                    {
                        app.media_list = page.media;
                        app.active_media = app.media_list.first().cloned();
                        app.go_to_mode(ListMode::AnimeList("Popular".into()), true);
                    }
                }
                "🎲 Random" => {
                    let buffer_size = 20;
                    let mut rng = thread_rng();
                    let range: Vec<i32> = (1..18000).collect();
                    let random_ids: Vec<i32> = range
                        .choose_multiple(&mut rng, buffer_size)
                        .cloned()
                        .collect();
                    if let Ok(res) =
                        api::fetch_media(json!({ "id_in": random_ids, "perPage": buffer_size }))
                            .await
                        && let Some(page) = res.data.page
                    {
                        app.media_list = page.media;
                        app.active_media = app.media_list.first().cloned();
                        app.go_to_mode(ListMode::AnimeList("Random".into()), true);
                    }
                }
                _ => {
                    app.set_status("Feature coming soon!");
                }
            }
            app.clear_status();
        }
        ListMode::SearchResults | ListMode::AnimeList(_) => {
            if app.active_media.is_some() {
                app.go_to_mode(ListMode::AnimeActions, true);
            }
        }
        ListMode::AnimeActions => {
            let idx = app.get_selected_index();
            if idx >= app.anime_action_items.len() {
                return;
            }
            let action = app.anime_action_items[idx];

            if let Some(media) = app.active_media.clone() {
                match action {
                    "▶️  Stream (Resume)" => {
                        let mut next_episode = "1".to_string();

                        if let (Some(token), Some(username)) =
                            (&config.auth.anilist_token, &config.auth.username)
                        {
                            app.set_status("Checking AniList progress...");
                            terminal.draw(|f| tui::ui::draw(f, app)).unwrap();

                            match api::get_user_progress(token, media.id, username).await {
                                Ok(Some(progress)) => {
                                    next_episode = (progress + 1).to_string();
                                    app.set_status(format!(
                                        "Resuming at Episode {}...",
                                        next_episode
                                    ));
                                }
                                Ok(None) => {
                                    app.set_status("Not in list. Starting at Episode 1.");
                                }
                                Err(e) => {
                                    app.set_status(format!(
                                        "Sync failed: {}. Defaulting to Ep 1.",
                                        e
                                    ));
                                }
                            }
                            tokio::time::sleep(Duration::from_millis(800)).await;
                        } else {
                            app.set_status("Not logged in. Starting at Episode 1.");
                            tokio::time::sleep(Duration::from_millis(800)).await;
                        }

                        suspend_and_watch(
                            terminal,
                            media.preferred_title(),
                            &next_episode,
                            Some(media.id),
                            config,
                        )
                        .await;
                        app.clear_status();
                    }
                    "📺 Episodes" => {
                        app.go_to_mode(ListMode::EpisodeSelect, true);
                    }
                    "🎞️  Watch Trailer" => {
                        if let Some(trailer) = &media.trailer {
                            let site = trailer.site.as_deref().unwrap_or("youtube");
                            let id = trailer.id.as_deref().unwrap_or("");
                            if site.eq_ignore_ascii_case("youtube") && !id.is_empty() {
                                let url = format!("https://www.youtube.com/watch?v={}", id);
                                app.set_status(format!("Opening {}", url));
                                let _ = std::process::Command::new("open")
                                    .arg(&url)
                                    .spawn()
                                    .or_else(|_| {
                                        std::process::Command::new("xdg-open").arg(&url).spawn()
                                    });
                            } else {
                                app.set_status("No YouTube trailer available.");
                            }
                        } else {
                            app.set_status("No trailer info found.");
                        }
                    }
                    _ => {
                        app.go_to_mode(ListMode::SubMenu(action.to_string()), true);
                    }
                }
            }
        }
        ListMode::EpisodeSelect => {
            let episode_num = (app.get_selected_index() + 1).to_string();
            if let Some(media) = app.active_media.clone() {
                suspend_and_watch(
                    terminal,
                    media.preferred_title(),
                    &episode_num,
                    Some(media.id),
                    config,
                )
                .await;
            }
        }
        _ => {}
    }
}

async fn suspend_and_watch<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    query: &str,
    ep: &str,
    anilist_id: Option<i32>,
    config: &ConfigManager,
) {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = terminal.show_cursor();
    let _ = io::stdout().flush();

    println!("▶️  Starting Playback: {} Episode {}...", query, ep);
    if let Err(e) = perform_watch(query.to_string(), ep.to_string(), anilist_id, config).await {
        println!("❌ Error: {}", e);
        println!("Press ENTER to continue...");
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
    }

    let _ = enable_raw_mode();
    let _ = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture);
    let _ = terminal.hide_cursor();
    let _ = terminal.clear();
}

async fn resolve_stream_for_episode(
    provider: &AllAnimeProvider,
    show_id: &str,
    show_name: &str,
    episode: &str,
) -> anyhow::Result<Option<PlayOptions>> {
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

async fn perform_watch(
    query: String,
    mut episode: String,
    anilist_id: Option<i32>,
    config: &ConfigManager,
) -> anyhow::Result<()> {
    let provider = Arc::new(AllAnimeProvider::new());
    println!("🔎 Searching AllAnime for '{}'...", query);

    let results = provider.search(&query).await?;
    if let Some(show) = results.first() {
        println!("Found: {} (ID: {})", show.name, show.id);

        let show_id = show.id.clone();
        let show_name = show.name.clone();
        let provider_clone = provider.clone();

        println!("📺 Fetching Episode {}...", episode);
        if let Some(options) =
            resolve_stream_for_episode(&provider, &show_id, &show_name, &episode).await?
        {
            let current_ep_num =
                std::sync::Arc::new(tokio::sync::Mutex::new(episode.parse::<i32>().unwrap_or(1)));

            let navigator: EpisodeNavigator = {
                let p = provider_clone.clone();
                let s_id = show_id.clone();
                let s_name = show_name.clone();
                let ep_num_store = current_ep_num.clone();

                Box::new(move |action| {
                    let p = p.clone();
                    let s_id = s_id.clone();
                    let s_name = s_name.clone();
                    let ep_store = ep_num_store.clone();

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

                        let next_ep_str = num.to_string();
                        resolve_stream_for_episode(&p, &s_id, &s_name, &next_ep_str).await
                    })
                })
            };

            let player = crate::player::mpv::MpvPlayer;

            match player.play(options, Some(navigator)).await {
                Ok(percentage) => {
                    println!("\n✅ Playback finished. Max progress: {:.1}%", percentage);

                    let final_ep_num = *current_ep_num.lock().await;
                    let required_percentage = config.config.stream.episode_complete_at as f64;

                    if percentage >= required_percentage {
                        if let (Some(token), Some(username), Some(id)) = (
                            &config.auth.anilist_token,
                            &config.auth.username,
                            anilist_id,
                        ) {
                            let current_progress = api::get_user_progress(token, id, username)
                                .await?
                                .unwrap_or(0);

                            if final_ep_num > current_progress {
                                println!(
                                    "📝 Updating AniList progress to Episode {}...",
                                    final_ep_num
                                );
                                api::update_user_entry(token, id, final_ep_num, "CURRENT").await?;
                            } else {
                                println!(
                                    "ℹ️  Already watched episode {} (Progress: {}). Skipping update.",
                                    final_ep_num, current_progress
                                );
                            }
                        }
                    } else {
                        println!(
                            "⚠️  Watched less than {}%. Not marking as complete.",
                            required_percentage
                        );
                    }
                }
                Err(e) => eprintln!("Player error: {}", e),
            }
        } else {
            anyhow::bail!("No streams found.");
        }
    } else {
        anyhow::bail!("No results found on AllAnime");
    }
    Ok(())
}
