use std::{borrow::Cow, ops::Deref, sync::Arc};

use chrono::TimeDelta;
use chrono_humanize::Humanize;
use octocrab::models::pulls::Comment;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget, Wrap},
};
use textwrap::wrap;

use crate::{actors::Actor, app::Context};

#[derive(Debug)]
pub struct ThreadComment(pub Comment);

impl Deref for ThreadComment {
    type Target = Comment;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ThreadComment {
    pub fn wrapped_body_with_width(&self, width: usize) -> Vec<Cow<'_, str>> {
        wrap(&self.body, width)
    }

    pub fn layout_height(&self, width: usize) -> u16 {
        self.wrapped_body_with_width(width).len() as u16 + 3
    }
}

impl StatefulWidgetRef for ThreadComment {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let time_delta = TimeDelta::from_std(state.delta()).expect("invalid delta");
        let author = self
            .user
            .as_ref()
            .map(|a| a.login.as_str())
            .unwrap_or("(unknown)");
        let created_at = self
            .created_at
            .checked_add_signed(time_delta)
            .unwrap_or(self.created_at)
            .humanize();

        let wrap_width = usize::from(area.width.max(10).min(80));

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                author,
                Style::default()
                    .fg(state.color_scheme.author.into())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                created_at,
                Style::default().fg(state.color_scheme.muted.into()),
            ),
        ]));

        lines.push(Line::default());

        for wrapped in self.wrapped_body_with_width(wrap_width) {
            lines.push(Line::from(Span::raw(wrapped.into_owned())));
        }

        lines.push(Line::default());

        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

impl Actor for ThreadComment {}
