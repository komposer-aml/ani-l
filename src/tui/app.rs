use crate::models::Media;
use crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::ListState;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use std::io::{self, Write};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    SearchBar,
    List,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListMode {
    MainMenu,
    SearchResults,
    AnimeList(String),
    AnimeActions,
    EpisodeSelect,
    SubMenu(String),
}

pub struct App {
    pub running: bool,
    pub focus: Focus,
    pub list_mode: ListMode,

    pub history_stack: Vec<(ListMode, usize, Option<Media>)>,

    pub search_query: String,

    pub list_state: ListState,

    pub main_menu_items: Vec<String>,
    pub anime_action_items: Vec<String>,

    pub media_list: Vec<Media>,

    pub cube_angle: f64,
    pub active_media: Option<Media>,

    pub status_message: Option<String>,
    pub is_loading: bool,

    pub image_picker: Option<Picker>,
    pub current_cover_image: Option<Box<dyn StatefulProtocol>>,
    pub image_tx: Sender<Vec<u8>>,
    pub image_rx: Receiver<Vec<u8>>,
    pub is_fetching_image: bool,
}

impl App {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let (tx, rx) = std::sync::mpsc::channel();

        Self {
            running: true,
            focus: Focus::List,
            list_mode: ListMode::MainMenu,
            history_stack: Vec::new(),
            search_query: String::new(),
            list_state,
            main_menu_items: vec![
                t!("main_menu.trending").to_string(),
                t!("main_menu.popular").to_string(),
                t!("main_menu.top_scored").to_string(),
                t!("main_menu.recently_updated").to_string(),
                t!("main_menu.random").to_string(),
                t!("main_menu.exit").to_string(),
            ],
            anime_action_items: vec![
                t!("actions.stream").to_string(),
                t!("actions.episodes").to_string(),
                t!("actions.trailer").to_string(),
                t!("actions.reviews").to_string(),
                t!("actions.schedule").to_string(),
                t!("actions.characters").to_string(),
                t!("actions.related").to_string(),
                t!("actions.recommendations").to_string(),
            ],
            media_list: vec![],
            cube_angle: 0.0,
            active_media: None,
            status_message: None,
            is_loading: false,
            image_picker: None,
            current_cover_image: None,
            image_tx: tx,
            image_rx: rx,
            is_fetching_image: false,
        }
    }

    pub fn init_image_picker(&mut self, protocol: Option<String>) {
        // 1. Determine Font Size
        // Try ioctl first (fastest)
        let mut font_size = Picker::from_termios()
            .ok()
            .map(|p| p.font_size)
            .unwrap_or((0, 0));

        // If ioctl failed (common in multiplexers), try escape sequence query
        if (font_size.0 == 0 || font_size.1 == 0)
            && let Ok(pixels) = self.query_terminal_pixels()
            && let Ok((cols, rows)) = crossterm::terminal::size()
            && cols > 0
            && rows > 0
        {
            font_size = (pixels.0 / cols, pixels.1 / rows);
        }

        // Fallback if everything fails
        if font_size.0 == 0 || font_size.1 == 0 {
            font_size = (10, 20);
        }

        // 2. Initialize Picker
        let mut picker = Picker::new(font_size);

        // 3. Set Protocol (Auto or Forced)
        if let Some(forced_proto) = protocol {
            picker.protocol_type = match forced_proto.to_lowercase().as_str() {
                "kitty" => ProtocolType::Kitty,
                "sixel" => ProtocolType::Sixel,
                "iterm2" => ProtocolType::Iterm2,
                _ => ProtocolType::Halfblocks,
            };
        } else if let Ok(p) = Picker::from_termios() {
            // If we didn't force it, trust the auto-detected one from termios check
            picker.protocol_type = p.protocol_type;
        }

        self.image_picker = Some(picker);
    }

    // Helper to query terminal size in pixels via CSI 14 t
    fn query_terminal_pixels(&self) -> anyhow::Result<(u16, u16)> {
        let mut stdout = io::stdout();
        write!(stdout, "\x1b[14t")?;
        stdout.flush()?;

        let mut response = String::new();
        let start = Instant::now();

        // Read response loop (timeout 500ms)
        while start.elapsed() < Duration::from_millis(500) {
            if event::poll(Duration::from_millis(10))?
                && let Event::Key(key) = event::read()?
            {
                match key.code {
                    KeyCode::Char(c) => response.push(c),
                    KeyCode::Esc => response.push('\x1b'),
                    _ => {}
                }
                if response.ends_with('t') {
                    break;
                }
            }
        }

        // Parse: \x1b[4;<h>;<w>t
        if let Some(start_idx) = response.find("\x1b[4;") {
            let content = &response[start_idx + 4..response.len() - 1]; // skip prefix and 't'
            let parts: Vec<&str> = content.split(';').collect();
            if parts.len() >= 2 {
                let h: u16 = parts[0].parse().unwrap_or(0);
                let w: u16 = parts[1].parse().unwrap_or(0);
                return Ok((w, h));
            }
        }

        Err(anyhow::anyhow!("Failed to parse window size"))
    }

    pub fn on_tick(&mut self) {
        self.cube_angle += 0.02;
        if self.cube_angle > 360.0 {
            self.cube_angle = 0.0;
        }

        if let Ok(bytes) = self.image_rx.try_recv() {
            if let Some(picker) = &mut self.image_picker
                && let Ok(img) = image::load_from_memory(&bytes)
            {
                let protocol = picker.new_resize_protocol(img);
                self.current_cover_image = Some(protocol);
            }
            self.is_fetching_image = false;
        }
    }

    pub fn set_status<S: Into<String>>(&mut self, msg: S) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn get_selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.list_len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.list_len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn jump_forward(&mut self, amount: usize) {
        let current = self.get_selected_index();
        let max = self.list_len().saturating_sub(1);
        let next = std::cmp::min(current + amount, max);
        self.list_state.select(Some(next));
    }

    pub fn jump_backward(&mut self, amount: usize) {
        let current = self.get_selected_index();
        let next = current.saturating_sub(amount);
        self.list_state.select(Some(next));
    }

    pub fn go_to_mode(&mut self, mode: ListMode, reset_index: bool) {
        let current_index = self.get_selected_index();
        self.history_stack.push((
            self.list_mode.clone(),
            current_index,
            self.active_media.clone(),
        ));

        self.list_mode = mode;
        if reset_index {
            self.list_state.select(Some(0));
        }
    }

    pub fn go_back(&mut self) {
        if let Some((prev_mode, prev_index, prev_media)) = self.history_stack.pop() {
            self.list_mode = prev_mode;
            self.list_state.select(Some(prev_index));
            self.active_media = prev_media;
            self.clear_status();
            self.current_cover_image = None;
        } else if matches!(self.list_mode, ListMode::MainMenu) {
            self.running = false;
        } else {
            self.reset_to_main_menu();
        }
    }

    pub fn reset_to_main_menu(&mut self) {
        self.list_mode = ListMode::MainMenu;
        self.history_stack.clear();
        self.media_list.clear();
        self.list_state.select(Some(0));
        self.active_media = None;
        self.search_query.clear();
        self.focus = Focus::List;
        self.clear_status();
        self.current_cover_image = None;
    }

    pub fn list_len(&self) -> usize {
        match self.list_mode {
            ListMode::MainMenu => self.main_menu_items.len(),
            ListMode::AnimeActions => self.anime_action_items.len(),
            ListMode::EpisodeSelect => self
                .active_media
                .as_ref()
                .and_then(|m| m.episodes)
                .unwrap_or(100) as usize,
            ListMode::SubMenu(_) => 0,
            _ => self.media_list.len(),
        }
    }
}
