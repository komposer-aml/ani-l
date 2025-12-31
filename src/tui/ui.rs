use crate::tui::app::{App, Focus, ListMode};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Wrap,
        canvas::{Canvas, Line as CanvasLine},
    },
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(f.size());

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(main_chunks[1]);

    draw_left_panel(f, main_chunks[0], app);
    draw_search_bar(f, right_chunks[0], app);
    draw_list_panel(f, right_chunks[1], app);
    draw_status_bar(f, right_chunks[2], app);
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (bg, fg, text) = if app.is_loading {
        (Color::Yellow, Color::Black, " ⏳ Loading... ".to_string())
    } else if let Some(msg) = &app.status_message {
        (Color::Blue, Color::White, format!(" ℹ️  {} ", msg))
    } else {
        let help = match app.focus {
            Focus::SearchBar => "/:Menu | ENTER:Search",
            Focus::List => match app.list_mode {
                ListMode::MainMenu => "j/k:Nav | ENTER:Select | q:Quit",
                ListMode::AnimeActions => "j/k:Nav | ENTER:Select | ESC:Back",
                _ => "(SHIFT)j/k:Nav | ENTER:Select | ⌫:Back | ESC:Home | /:Search",
            },
        };
        (Color::DarkGray, Color::White, format!(" {} ", help))
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(bg).fg(fg)),
        area,
    );
}

fn draw_list_panel(f: &mut Frame, area: Rect, app: &mut App) {
    let border_color = if app.focus == Focus::List {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = match &app.list_mode {
        ListMode::MainMenu => " Main Menu ".to_string(),
        ListMode::SearchResults => " Search Results ".to_string(),
        ListMode::AnimeList(t) => format!(" {} ", t),
        ListMode::AnimeActions => " Actions ".to_string(),
        ListMode::EpisodeSelect => " Select Episode ".to_string(),
        ListMode::SubMenu(t) => format!(" {} ", t),
    };

    let pad = |s: &str| format!("   {}   ", s);

    let create_list = |items: Vec<String>| {
        items
            .into_iter()
            .map(|item| ListItem::new(pad(&item)).style(Style::default()))
            .collect::<Vec<ListItem>>()
    };

    let items: Vec<ListItem> = match &app.list_mode {
        ListMode::MainMenu => {
            create_list(app.main_menu_items.iter().map(|s| s.to_string()).collect())
        }
        ListMode::AnimeActions => create_list(
            app.anime_action_items
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        ListMode::EpisodeSelect => {
            let count = app.list_len();
            let ep_strings: Vec<String> = (1..=count).map(|i| format!("Episode {}", i)).collect();
            create_list(ep_strings)
        }
        ListMode::SubMenu(_) => vec![ListItem::new("  (Feature Coming Soon)")],
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
                ListItem::new(pad(&display_title)).style(Style::default())
            })
            .collect(),
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_left_panel(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Ani-L ");
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if let Some(media) = &app.active_media {
        let text = vec![
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
                Span::styled("Episodes: ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", media.episodes.unwrap_or(0))),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Description:",
                Style::default().fg(Color::Cyan),
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
        ];
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center),
            inner_area,
        );
    } else {
        draw_cube(f, inner_area, app.cube_angle);
    }
}

fn draw_search_bar(f: &mut Frame, area: Rect, app: &App) {
    let border_color = if app.focus == Focus::SearchBar {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let query_text = if app.search_query.is_empty() && app.focus != Focus::SearchBar {
        Span::styled(
            "Press '/' to search...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::raw(&app.search_query)
    };
    f.render_widget(
        Paragraph::new(query_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title(" Search "),
        ),
        area,
    );
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
