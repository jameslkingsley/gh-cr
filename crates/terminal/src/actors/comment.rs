use std::{ops::Deref, sync::Arc};

use chrono_humanize::Humanize;
use octocrab::models::pulls::Comment;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget},
};

use crate::{
    actors::Actor,
    app::Context,
    utils::{stylize_block, wrap_markdown_body},
};

impl StatefulWidgetRef for ThreadComment {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let content = self.as_text(area.width);

        Paragraph::new(content).render(area, buf);
    }
}

#[derive(Debug)]
pub struct ThreadComment(pub Comment);

impl Deref for ThreadComment {
    type Target = Comment;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ThreadComment {
    pub fn as_text(&self, width: u16) -> Text<'_> {
        let created_at = self.created_at.humanize();
        let author = self
            .user
            .as_ref()
            .map(|a| a.login.to_owned())
            .unwrap_or("(unknown)".to_owned());

        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::from_iter([
            Span::styled(author, Style::default().cyan()),
            Span::raw(" "),
            Span::styled(created_at, Style::default().dim()),
        ]));

        let mut content = Text::from(lines);

        wrap_markdown_body(
            &self.sanitised_body(),
            // Subtract 2 because the stylised block consumes 2 columns
            width.saturating_sub(2) as usize,
            &mut content,
            Style::new().gray(),
            Style::new().dark_gray(),
        );

        stylize_block(&mut content, Style::new().dark_gray());

        content
    }

    pub fn layout_height(&self, area_width: u16) -> u16 {
        self.as_text(area_width).lines.len() as u16
    }

    pub fn sanitised_body(&self) -> String {
        // Fixes ghost characters left by tabs
        self.body.replace("\t", "    ")
    }
}

impl Actor for ThreadComment {}
