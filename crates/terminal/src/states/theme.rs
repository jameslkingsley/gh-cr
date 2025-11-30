use ratatui::{crossterm::style::Color, style::Color as TuiColor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeState {
    pub pr_title: Rgb,
    pub muted: Rgb,
    pub author: Rgb,
    pub comment_body: Rgb,
    pub border: Rgb,
    pub border_active: Rgb,
    pub diff_added: Rgb,
    pub diff_removed: Rgb,
    pub diff_unchanged: Rgb,
}

impl Default for ThemeState {
    fn default() -> Self {
        Self {
            pr_title: Rgb(208, 208, 208),
            muted: Rgb(51, 53, 68),
            author: Rgb(18, 207, 192),
            comment_body: Rgb(208, 208, 208),
            border: Rgb(51, 53, 68),
            border_active: Rgb(18, 207, 192),
            diff_added: Rgb(218, 255, 166),
            diff_removed: Rgb(246, 144, 144),
            diff_unchanged: Rgb(51, 53, 68),
        }
    }
}

impl From<Rgb> for Color {
    fn from(rgb: Rgb) -> Self {
        Color::Rgb {
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
        }
    }
}

impl From<Rgb> for TuiColor {
    fn from(rgb: Rgb) -> Self {
        TuiColor::Rgb(rgb.0, rgb.1, rgb.2)
    }
}
