use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

pub enum TuiEvent {
    Key(KeyCode),
    Tick,
    Quit,
}

pub fn handle_input() -> Result<TuiEvent> {
    if event::poll(Duration::from_millis(16))?
        && let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
    {
        return match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Ok(TuiEvent::Quit),
            code => Ok(TuiEvent::Key(code)),
        };
    }
    Ok(TuiEvent::Tick)
}
