use chrono_humanize::Humanize;
use colorhash::ColorHash;
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
};

use crate::{
    prelude::RenderableComment,
    utils::{sanitised_markdown, stylize_block, wrap_markdown_body},
};

pub fn comment_into_text<'a, T: RenderableComment>(
    text: &mut Text<'a>,
    comment: &'a T,
    width: u16,
) {
    let created_at = comment.created_at().humanize();
    let author = comment.author();

    let mut lines: Vec<Line> = Vec::new();

    let author_color = ColorHash::new().rgb(&author);
    let author_color = Color::Rgb(
        author_color.red().floor() as u8,
        author_color.green().floor() as u8,
        author_color.blue().floor() as u8,
    );

    lines.push(Line::from_iter([
        Span::styled(author, Style::default().fg(author_color).bold()),
        Span::raw(" "),
        Span::styled(created_at, Style::default().dim()),
    ]));

    let body = sanitised_markdown(comment.body());

    wrap_markdown_body(
        &body,
        wrap_width(width),
        &mut lines,
        Style::new().gray(),
        Style::new().dark_gray(),
    );

    stylize_block(&mut lines, Style::new().dark_gray());

    text.extend(lines);
}

pub fn wrap_width(area_width: u16) -> usize {
    // Stylize block consumes 2 columns; clamp to a sensible range.
    let available = area_width.saturating_sub(2).max(1);
    usize::from(available).min(80)
}
