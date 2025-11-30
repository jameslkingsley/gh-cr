mod scroll;
mod theme;

use crate::states::{scroll::ScrollState, theme::ThemeState};

#[derive(Debug, Default, Copy, Clone)]
pub enum View {
    #[default]
    Threads,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct AppState {
    pub scroll: ScrollState,
    pub theme: ThemeState,
    pub view: View,
}
