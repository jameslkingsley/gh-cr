use chrono_humanize::Humanize;
use colorhash::ColorHash;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget},
};

use crate::{
    github::threads::{IssueComment, ReviewComment},
    prelude::RenderableComment,
    states::AppState,
    utils::{sanitised_markdown, stylize_block, wrap_markdown_body},
};

pub enum CommentWidget<'a> {
    ReviewComment(&'a ReviewComment),
    #[allow(dead_code)]
    IssueComment(&'a IssueComment),
}

impl StatefulWidgetRef for CommentWidget<'_> {
    type State = AppState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let content = match self {
            CommentWidget::ReviewComment(comment) => as_text(comment, area.width),
            CommentWidget::IssueComment(comment) => as_text(comment, area.width),
        };

        Paragraph::new(content).render(area, buf);
    }
}

pub fn as_text<T: RenderableComment>(comment: &T, width: u16) -> Text<'_> {
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

    let mut content = Text::from(lines);

    let body = sanitised_markdown(comment.body());

    wrap_markdown_body(
        &body,
        wrap_width(width),
        &mut content,
        Style::new().gray(),
        Style::new().dark_gray(),
    );

    stylize_block(&mut content, Style::new().dark_gray());

    content
}

pub fn wrap_width(area_width: u16) -> usize {
    // Stylize block consumes 2 columns; clamp to a sensible range.
    let available = area_width.saturating_sub(2).max(1);
    usize::from(available).min(80)
}

pub fn comment_layout_height(comment: &ReviewComment, area_width: u16) -> u16 {
    as_text(&comment, area_width).lines.len() as u16
}
