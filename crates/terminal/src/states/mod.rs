mod scroll;
mod theme;

use anyhow::Result;
use ratatui::crossterm::event::Event;
use throbber_widgets_tui::ThrobberState;

use crate::states::{scroll::ScrollState, theme::ThemeState};

#[derive(Debug, Default, Copy, Clone)]
pub enum View {
    #[default]
    Threads,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AppState {
    pub scroll: ScrollState,
    pub theme: ThemeState,
    pub view: View,
    pub throbber: ThrobberState,
    pub show_diffs: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            scroll: Default::default(),
            theme: Default::default(),
            view: Default::default(),
            throbber: Default::default(),
            show_diffs: true,
        }
    }
}

impl AppState {
    pub fn tick(&mut self, event: &Event) -> Result<bool> {
        self.scroll.tick(event)
    }
}
