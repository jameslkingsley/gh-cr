use std::{ops::Deref, sync::Arc};

use chrono::TimeDelta;
use chrono_humanize::Humanize;
use octocrab::models::pulls::Comment;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, StatefulWidgetRef, Widget, Wrap},
};

use crate::{actors::Actor, app::Context};

#[derive(Debug)]
pub struct ThreadComment(pub Comment);

impl Deref for ThreadComment {
    type Target = Comment;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl StatefulWidgetRef for ThreadComment {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let time_delta = TimeDelta::from_std(state.delta()).expect("invalid delta");

        let block = Block::new()
            .borders(Borders::all())
            .title(format!(
                "{} {}",
                self.user
                    .as_ref()
                    .map(|a| a.login.as_str())
                    .unwrap_or_else(|| "(unknown)"),
                self.created_at
                    .checked_add_signed(time_delta)
                    .unwrap_or(self.created_at)
                    .humanize()
            ))
            .title_alignment(ratatui::layout::Alignment::Left);

        let p = Paragraph::new(self.body.clone())
            .wrap(Wrap { trim: false })
            .left_aligned()
            .block(block);

        p.render(area, buf);
    }
}

impl Actor for ThreadComment {}
