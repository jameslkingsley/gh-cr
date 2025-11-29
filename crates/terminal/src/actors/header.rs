use std::sync::Arc;

use chrono_humanize::Humanize;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, StatefulWidgetRef, Widget},
};

use crate::{actors::Actor, app::Context};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Header;

impl Actor for Header {
    fn dirty(&self) -> bool {
        false
    }
}

impl StatefulWidgetRef for Header {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let pr = &state.github.pr;

        let mut lines: Vec<Line> = Vec::new();

        let title = pr.title.as_deref().unwrap_or("Pull Request");
        lines.push(Line::from(Span::styled(
            title,
            Style::default()
                .fg(state.color_scheme.pr_title.into())
                .add_modifier(Modifier::BOLD),
        )));

        let mut meta: Vec<Span> = Vec::new();

        if let Some(author) = pr.user.as_ref() {
            meta.push(Span::styled(
                author.login.as_str(),
                Style::default().fg(state.color_scheme.author.into()),
            ));
        }

        if let Some(updated_at) = pr.updated_at {
            if !meta.is_empty() {
                meta.push(Span::styled(
                    "  ·  ",
                    Style::default().fg(state.color_scheme.muted.into()),
                ));
            }

            meta.push(Span::styled(
                updated_at.humanize(),
                Style::default().fg(state.color_scheme.muted.into()),
            ));
        }

        if let Some(count) = pr.changed_files {
            if !meta.is_empty() {
                meta.push(Span::styled(
                    "  ·  ",
                    Style::default().fg(state.color_scheme.muted.into()),
                ));
            }

            let label = format!(
                "{} file{} changed",
                count,
                if count == 1 { "" } else { "s" }
            );
            meta.push(Span::styled(
                label,
                Style::default().fg(state.color_scheme.muted.into()),
            ));
        }

        if !meta.is_empty() {
            lines.push(Line::from(meta));
        }

        lines.push(Line::default());

        Paragraph::new(Text::from(lines)).render(area, buf);
    }
}
