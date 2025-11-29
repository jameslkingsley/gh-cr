use std::{borrow::Cow, ops::Deref, sync::Arc};

use chrono_humanize::Humanize;
use octocrab::models::pulls::Comment;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget},
};
use textwrap::wrap;

use crate::{
    actors::Actor,
    app::Context,
    utils::{stylize_block, wrap_markdown_body},
};

impl StatefulWidgetRef for ThreadComment {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
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
            area.width.saturating_sub(2) as usize,
            &mut content,
            Style::new().gray(),
            Style::new().dark_gray(),
        );

        stylize_block(&mut content, Style::new().dark_gray());

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
    pub fn wrap_width(area_width: u16) -> usize {
        usize::from(area_width.clamp(10, 80))
    }

    pub fn wrapped_body_with_width(&self, width: usize) -> Vec<Cow<'_, str>> {
        wrap(&self.body, width)
    }

    pub fn wrapped_body(&self, area_width: u16) -> Vec<Cow<'_, str>> {
        self.wrapped_body_with_width(Self::wrap_width(area_width))
    }

    pub fn layout_height(&self, area_width: u16) -> u16 {
        self.wrapped_body(area_width).len() as u16 + 3
    }

    pub fn sanitised_body(&self) -> String {
        // Fixes ghost characters left by tabs
        self.body.replace("\t", "    ")
    }
}

impl Actor for ThreadComment {}
