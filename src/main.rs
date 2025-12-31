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
use std::io;

use crate::config::ConfigManager;
use crate::player::traits::Player;
use crate::provider::allanime::AllAnimeProvider;
use crate::registry::RegistryManager;
use crate::tui::app::{App, Focus, ListMode};
use crate::tui::events::TuiEvent;

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
    let config_manager = ConfigManager::new()?;
    let _registry_manager = RegistryManager::new()?;

    if let Some(proj_dirs) = ProjectDirs::from("com", "sleepy-foundry", "ani-l") {
        println!(
            "📂 Configuration & Registry loaded from: {:?}",
            proj_dirs.config_dir()
        );
    }

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Commands::Tui);

    match command {
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
            let media_list = response.data.page.media;

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
        Commands::Play { url, title } => {
            let player = crate::player::mpv::MpvPlayer;
            let options = crate::player::traits::PlayOptions {
                url,
                title,
                ..Default::default()
            };
            if let Err(e) = player.play(options) {
                eprintln!("❌ Playback failed: {}", e);
            }
        }
        Commands::Watch { query, episode } => {
            perform_watch(query, episode).await?;
        }
        Commands::Tui => {
            run_tui(config_manager).await?;
        }
    }

    Ok(())
}

async fn run_tui(_config: ConfigManager) -> anyhow::Result<()> {
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
                                    app.media_list = res.data.page.media;
                                    app.go_to_mode(ListMode::SearchResults, true);
                                    app.active_media = app.media_list.first().cloned();
                                    app.focus = Focus::List;
                                    app.clear_status();
                                } else {
                                    app.set_status("Search failed.");
                                }
                                app.is_loading = false;
                            }
                        }
                        _ => {}
                    },
                    Focus::List => match code {
                        // Standard Nav
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.next();
                            update_preview(&mut app);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.previous();
                            update_preview(&mut app);
                        }
                        // Fast Nav (Jump 10)
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
                            handle_enter(&mut app, &mut terminal).await;
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
    // Only update preview if browsing media lists, not static menus
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
                    {
                        app.media_list = res.data.page.media;
                        app.active_media = app.media_list.first().cloned();
                        app.go_to_mode(ListMode::AnimeList("Trending".into()), true);
                    }
                }
                "✨ Popular" => {
                    if let Ok(res) =
                        api::fetch_media(json!({ "perPage": 20, "sort": "POPULARITY_DESC" })).await
                    {
                        app.media_list = res.data.page.media;
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
                    {
                        app.media_list = res.data.page.media;
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
                        suspend_and_watch(terminal, &media.preferred_title(), "1").await;
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
                suspend_and_watch(terminal, &media.preferred_title(), &episode_num).await;
            }
        }
        _ => {}
    }
}

// ... (suspend_and_watch and perform_watch functions remain exactly as before) ...
async fn suspend_and_watch<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    query: &str,
    ep: &str,
) {
    disable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
    terminal.show_cursor().unwrap();

    println!("▶️  Starting Playback: {} Episode {}...", query, ep);
    if let Err(e) = perform_watch(query.to_string(), ep.to_string()).await {
        println!("❌ Error: {}", e);
        println!("Press ENTER to continue...");
        let mut s = String::new();
        io::stdin().read_line(&mut s).unwrap();
    }

    enable_raw_mode().unwrap();
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableMouseCapture
    )
    .unwrap();
    terminal.hide_cursor().unwrap();
    terminal.clear().unwrap();
}

async fn perform_watch(query: String, episode: String) -> anyhow::Result<()> {
    // ... (Use same logic as provided in previous turns) ...
    let provider = AllAnimeProvider::new();
    println!("🔎 Searching AllAnime for '{}'...", query);

    let results = provider.search(&query).await?;
    if let Some(show) = results.first() {
        println!("Found: {} (ID: {})", show.name, show.id);
        println!("📺 Fetching Episode {}...", episode);

        let sources = provider.get_episode_sources(&show.id, &episode).await?;
        let priorities = ["S-mp4", "Luf-mp4", "Luf-Mp4", "Sak", "Default", "Yt-mp4"];
        let mut played_successfully = false;

        for source_name in priorities {
            if let Some(source) = sources.iter().find(|s| s.source_name == source_name) {
                println!("⚡ Attempting extraction from '{}'...", source_name);

                match provider.extract_clock_stream(&source.source_url).await {
                    Ok(mut options) => {
                        options.title = Some(format!("{} - Episode {}", show.name, episode));
                        let player = crate::player::mpv::MpvPlayer;
                        println!("🍿 Starting Player ({}) ...", source_name);

                        match player.play(options) {
                            Ok(_) => {
                                played_successfully = true;
                                break;
                            }
                            Err(e) => {
                                eprintln!("⚠️  Player failed for {}: {}", source_name, e);
                                println!("🔄 Trying next source...");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Extraction failed for {}: {}", source_name, e);
                    }
                }
            }
        }

        if !played_successfully {
            anyhow::bail!("All sources failed. Try checking your internet or MPV installation.");
        }
    } else {
        anyhow::bail!("No results found on AllAnime");
    }
    Ok(())
}
