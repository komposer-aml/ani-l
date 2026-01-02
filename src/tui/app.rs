use crate::config::ConfigManager;
use crate::models::Media;
use ratatui::widgets::ListState;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Notify, mpsc};

#[derive(Debug, Clone)]
pub enum Action {
    Tick,
    Quit,
    ToggleFocus,
    NavigateUp,
    NavigateDown,
    NavigatePageUp,
    NavigatePageDown,
    GoBack,
    Select,
    SearchStarted,
    SearchCompleted(Vec<Media>, Option<String>),
    SearchError(String),
    ImageLoaded(Vec<u8>),
    UpdateAvailable(String),
    StreamStarted,
    StreamLog(String),
    StreamFinished,
    Suspend(Arc<Notify>),
    Resume,
}

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
    Options,
    StreamLogging,
    SubMenu(String),
}

pub struct App {
    pub running: bool,
    pub focus: Focus,
    pub list_mode: ListMode,
    pub search_query: String,
    pub list_state: ListState,
    pub main_menu_items: Vec<String>,
    pub anime_action_items: Vec<String>,
    pub media_list: Vec<Media>,
    pub active_media: Option<Media>,
    pub config_manager: ConfigManager,
    pub history_stack: Vec<(ListMode, usize, Option<Media>)>,
    pub action_tx: mpsc::UnboundedSender<Action>,
    pub action_rx: mpsc::UnboundedReceiver<Action>,
    pub cube_angle: f64,
    pub is_loading: bool,
    pub status_message: Option<String>,
    pub stream_logs: VecDeque<String>,
    pub image_picker: Option<Picker>,
    pub current_cover_image: Option<StatefulProtocol>,
    pub is_fetching_image: bool,
    pub new_version: Option<String>,
    pub show_update_modal: bool,
}

impl App {
    pub fn new(config_manager: ConfigManager) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        let mut app = Self {
            running: true,
            focus: Focus::List,
            list_mode: ListMode::MainMenu,
            search_query: String::new(),
            list_state,
            main_menu_items: vec![],
            anime_action_items: vec![],
            media_list: vec![],
            active_media: None,
            config_manager,
            history_stack: Vec::new(),
            action_tx,
            action_rx,
            cube_angle: 0.0,
            is_loading: false,
            status_message: None,
            stream_logs: VecDeque::with_capacity(20),
            image_picker: None,
            current_cover_image: None,
            is_fetching_image: false,
            new_version: None,
            show_update_modal: false,
        };
        app.update_localized_items();
        app
    }

    pub fn update_localized_items(&mut self) {
        self.main_menu_items = vec![
            t!("main_menu.trending").to_string(),
            t!("main_menu.popular").to_string(),
            t!("main_menu.top_scored").to_string(),
            t!("main_menu.recently_updated").to_string(),
            t!("main_menu.random").to_string(),
            t!("main_menu.options").to_string(),
            t!("main_menu.exit").to_string(),
        ];
        self.anime_action_items = vec![
            t!("actions.stream").to_string(),
            t!("actions.episodes").to_string(),
            t!("actions.trailer").to_string(),
            t!("actions.reviews").to_string(),
            t!("actions.schedule").to_string(),
            t!("actions.characters").to_string(),
            t!("actions.related").to_string(),
            t!("actions.recommendations").to_string(),
        ];
    }

    #[allow(deprecated)]
    pub fn init_image_picker(&mut self) {
        let picker = match Picker::from_query_stdio() {
            Ok(p) => p,
            Err(_) => Picker::from_fontsize((10, 20)),
        };
        self.image_picker = Some(picker);
    }

    pub fn on_tick(&mut self) {
        self.cube_angle += 0.02;
        if self.cube_angle > 360.0 {
            self.cube_angle = 0.0;
        }
    }

    pub fn log_stream(&mut self, msg: String) {
        if self.stream_logs.len() >= 20 {
            self.stream_logs.pop_front();
        }
        self.stream_logs.push_back(msg);
    }

    pub fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.list_len().saturating_sub(1) {
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

    pub fn list_len(&self) -> usize {
        match self.list_mode {
            ListMode::MainMenu => self.main_menu_items.len(),
            ListMode::AnimeActions => self.anime_action_items.len(),
            ListMode::EpisodeSelect => self
                .active_media
                .as_ref()
                .and_then(|m| m.episodes)
                .unwrap_or(100) as usize,
            ListMode::Options => 3,
            ListMode::SubMenu(_) => 1,
            _ => self.media_list.len(),
        }
    }

    pub fn get_selected_index(&self) -> usize {
        self.list_state.selected().unwrap_or(0)
    }

    pub fn go_to_mode(&mut self, mode: ListMode, reset_index: bool) {
        self.history_stack.push((
            self.list_mode.clone(),
            self.get_selected_index(),
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
            self.current_cover_image = None;
            self.stream_logs.clear();
        } else if matches!(self.list_mode, ListMode::MainMenu) {
            self.running = false;
        } else {
            self.list_mode = ListMode::MainMenu;
            self.history_stack.clear();
            self.list_state.select(Some(0));
            self.active_media = None;
            self.search_query.clear();
        }
    }
}
