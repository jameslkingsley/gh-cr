use crossterm::style::Color;

#[derive(Debug)]
pub struct ColorScheme {
    pub author: Color,
    pub timestamp: Color,
    pub comment_body: Color,
    pub borders: Color,
}
