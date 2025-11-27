use std::{borrow::Cow, ops::Deref, sync::Arc};

use chrono::TimeDelta;
use chrono_humanize::Humanize;
use octocrab::models::pulls::Comment;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, ToLine},
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

impl ThreadComment {
    pub fn wrapped_body(&self) -> Vec<Cow<'_, str>> {
        textwrap::wrap(&self.body, 80)
    }

    pub fn layout_height(&self) -> u16 {
        self.wrapped_body().len() as u16 + 2
    }
}

impl StatefulWidgetRef for ThreadComment {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let time_delta = TimeDelta::from_std(state.delta()).expect("invalid delta");

        let block = Block::new()
            .borders(Borders::LEFT)
            .border_type(ratatui::widgets::BorderType::Rounded)
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

        let mut top = area.top();

        buf.set_line(
            area.left(),
            top,
            &format!(
                "{} {}",
                self.user
                    .as_ref()
                    .map(|a| a.login.as_str())
                    .unwrap_or_else(|| "(unknown)"),
                self.created_at
                    .checked_add_signed(time_delta)
                    .unwrap_or(self.created_at)
                    .humanize()
            )
            .to_line(),
            80,
        );

        top += 1;

        for (index, line) in self.wrapped_body().iter().enumerate() {
            let y = index as u16;
            buf.set_line(area.left(), top + y, &line.to_line(), 80);
        }

        // let p = Paragraph::new(self.body.clone())
        //     .wrap(Wrap { trim: false })
        //     .left_aligned()
        //     .block(block);

        // p.render(area, buf);
    }
}

impl Actor for ThreadComment {}
