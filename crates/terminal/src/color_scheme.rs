use ratatui::{crossterm::style::Color, style::Color as TuiColor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Serialize, Deserialize)]
pub struct ColorScheme {
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
