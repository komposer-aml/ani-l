use crate::tui::app::{App, Focus, ListMode};
use ratatui::{
    prelude::*,
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Wrap,
        canvas::{Canvas, Line as CanvasLine},
    },
};
use ratatui_image::{Resize, StatefulImage};

pub fn draw(f: &mut Frame, app: &mut App) {
    let main_area = f.area();

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_area);

    let right_col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Min(1),    // List
            Constraint::Length(1), // Status
        ])
        .split(layout[1]);

    draw_left_panel(f, layout[0], app);
    draw_search_bar(f, right_col[0], app);
    draw_list_panel(f, right_col[1], app);
    draw_status_bar(f, right_col[2], app);

    if app.show_update_modal {
        draw_update_modal(f, app);
    }

    if matches!(app.list_mode, ListMode::StreamLogging) {
        draw_stream_logs(f, app);
    }
}

fn draw_left_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(t!("titles.ani_l").to_string());
    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(media) = &app.active_media {
        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(inner);

        let top_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(left_layout[0]);

        if let Some(protocol) = &mut app.current_cover_image {
            let image = StatefulImage::new().resize(Resize::Fit(None));
            f.render_stateful_widget(image, top_layout[0], protocol);
        } else {
            let message = if app.is_fetching_image {
                "Loading Image..."
            } else if app.image_picker.is_none() {
                "Terminal not supported.\nTry WezTerm, Ghostty, iTerm2 or Kitty."
            } else {
                "No Image Found"
            };

            let placeholder = Paragraph::new(message)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(placeholder, top_layout[0]);
        }

        let details = vec![
            Line::from(Span::styled(
                media.preferred_title(),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Score: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}%", media.average_score.unwrap_or(0))),
                Span::raw(" | "),
                Span::styled("Favs: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", media.favourites.unwrap_or(0))),
            ]),
            Line::from(vec![
                Span::styled("Pop: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", media.popularity.unwrap_or(0))),
                Span::raw(" | "),
                Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                Span::raw(media.status.clone().unwrap_or("Unknown".into())),
            ]),
            Line::from(vec![
                Span::styled("Format: ", Style::default().fg(Color::Cyan)),
                Span::raw(media.format.clone().unwrap_or("?".into())),
            ]),
            Line::from("----"),
            Line::from(vec![Span::styled(
                "Genres: ",
                Style::default().fg(Color::Cyan),
            )]),
            Line::from(media.genres.join(", ")),
        ];

        f.render_widget(
            Paragraph::new(details).wrap(Wrap { trim: true }).block(
                Block::default()
                    .borders(Borders::NONE)
                    .padding(ratatui::widgets::Padding::new(1, 0, 0, 0)),
            ),
            top_layout[1],
        );

        let bottom_text = vec![
            Line::from(Span::styled(
                "Description:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(
                media
                    .description
                    .clone()
                    .unwrap_or_default()
                    .replace("<br>", "\n")
                    .replace("<i>", "")
                    .replace("</i>", ""),
            ),
            Line::from(""),
            Line::from(Span::styled("Details:", Style::default().fg(Color::Cyan))),
            Line::from(vec![
                Span::raw("Studios: "),
                Span::raw(
                    media
                        .studios
                        .as_ref()
                        .map(|s| {
                            s.nodes
                                .iter()
                                .map(|n| n.name.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or("-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::raw("Aired: "),
                Span::raw(format!(
                    "{} to {}",
                    media.formatted_start_date(),
                    media.formatted_end_date()
                )),
            ]),
        ];

        f.render_widget(
            Paragraph::new(bottom_text)
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::TOP)),
            left_layout[1],
        );
    } else {
        draw_cube(f, inner, app.cube_angle);
    }
}

fn draw_stream_logs(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 60, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ▶️  Stream Initializing ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let logs: Vec<ListItem> = app
        .stream_logs
        .iter()
        .map(|log| {
            ListItem::new(Line::from(vec![
                Span::styled(" > ", Style::default().fg(Color::Cyan)),
                Span::raw(log),
            ]))
        })
        .collect();

    let list = List::new(logs).highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, inner);
}

fn draw_list_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let border_style = if app.focus == Focus::List {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let title = match &app.list_mode {
        ListMode::MainMenu => t!("titles.main_menu").to_string(),
        ListMode::SearchResults => t!("titles.search_results").to_string(),
        ListMode::AnimeActions => t!("titles.actions").to_string(),
        ListMode::EpisodeSelect => t!("titles.select_episode").to_string(),
        ListMode::Options => t!("titles.options").to_string(),
        ListMode::StreamLogging => " Stream Logs ".to_string(),
        ListMode::AnimeList(t) => format!(" {} ", t),
        ListMode::SubMenu(t) => format!(" {} ", t),
    };

    let pad = |s: &str| format!("   {}   ", s);

    let items: Vec<ListItem> = match &app.list_mode {
        ListMode::MainMenu => app
            .main_menu_items
            .iter()
            .map(|i| ListItem::new(pad(i)))
            .collect(),
        ListMode::AnimeActions => app
            .anime_action_items
            .iter()
            .map(|i| ListItem::new(pad(i)))
            .collect(),
        ListMode::Options => vec![
            ListItem::new(pad(&t!(
                "options.quality",
                val = app.config_manager.config.stream.quality
            ))),
            ListItem::new(pad(&t!(
                "options.translation",
                val = app.config_manager.config.stream.translation_type
            ))),
            ListItem::new(pad(&t!(
                "options.language",
                val = app.config_manager.config.general.language
            ))),
        ],
        ListMode::EpisodeSelect => {
            let count = app.list_len();
            (1..=count)
                .map(|i| ListItem::new(pad(&t!("ui.episode_prefix", num = i).to_string())))
                .collect()
        }
        ListMode::SubMenu(_) => vec![ListItem::new(pad(&t!("ui.feature_soon").to_string()))],
        _ => app
            .media_list
            .iter()
            .map(|m| {
                let title = m.preferred_title();
                let display_title = if title.len() > 30 {
                    format!("{}...", &title[..27])
                } else {
                    title.to_string()
                };
                ListItem::new(pad(&display_title))
            })
            .collect(),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let border_style = if app.focus == Focus::SearchBar {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let text = if app.search_query.is_empty() && app.focus != Focus::SearchBar {
        t!("ui.search_placeholder").to_string()
    } else {
        app.search_query.clone()
    };

    f.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(t!("titles.search").to_string()),
        ),
        area,
    );
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (bg, fg, text) = if app.is_loading {
        (Color::Yellow, Color::Black, t!("ui.loading").to_string())
    } else if let Some(msg) = &app.status_message {
        (Color::Blue, Color::White, format!(" ℹ️  {} ", msg))
    } else {
        let help = match app.focus {
            Focus::SearchBar => t!("ui.help_search").to_string(),
            Focus::List => match app.list_mode {
                ListMode::MainMenu => t!("ui.help_nav_select_quit").to_string(),
                ListMode::AnimeActions => t!("ui.help_nav_select_back").to_string(),
                _ => t!("ui.help_full").to_string(),
            },
        };
        (Color::DarkGray, Color::White, format!(" {} ", help))
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(bg).fg(fg)),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_cube(f: &mut Frame, area: Rect, angle: f64) {
    let canvas = Canvas::default()
        .x_bounds([-2.0, 2.0])
        .y_bounds([-2.0, 2.0])
        .paint(move |ctx| {
            let a = angle;
            let nodes = [
                [-1.0, -1.0, -1.0],
                [1.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ];
            let mut projected = vec![];
            for node in nodes {
                let x = node[0] * a.cos() - node[2] * a.sin();
                let z = node[0] * a.sin() + node[2] * a.cos();
                let y = node[1];
                let y_rot = y * a.cos() - z * a.sin();
                projected.push((x, y_rot));
            }
            let edges = [
                (0, 1),
                (1, 2),
                (2, 3),
                (3, 0),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 4),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];
            for (start, end) in edges {
                ctx.draw(&CanvasLine {
                    x1: projected[start].0,
                    y1: projected[start].1,
                    x2: projected[end].0,
                    y2: projected[end].1,
                    color: Color::Cyan,
                });
            }
            ctx.print(0.0, -1.5, "ani-l");
        });
    f.render_widget(canvas, area);
}

fn draw_update_modal(f: &mut Frame, app: &App) {
    if let Some(new_ver) = &app.new_version {
        let area = centered_rect(60, 20, f.area());

        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" 🚀 Update Available ")
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));

        let text = vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("A new version of "),
                Span::styled(
                    "ani-l",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Cyan),
                ),
                Span::raw(" is available!"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Current Version: "),
                Span::styled(env!("CARGO_PKG_VERSION"), Style::default().fg(Color::Red)),
            ]),
            Line::from(vec![
                Span::raw("Latest Version:  "),
                Span::styled(new_ver, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from("To update, run:"),
            Line::from(Span::styled(
                "cargo install ani-l",
                Style::default().bg(Color::Black).fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [ESC] or [Enter] to close",
                Style::default().fg(Color::Gray),
            )),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }
}
