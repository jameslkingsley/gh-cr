use std::fmt::Write;

use anyhow::Result;
use chrono::TimeDelta;
use chrono_humanize::Humanize;
use crossterm::style::Stylize;
use octocrab::models::pulls::Comment;

use crate::{app::App, components::Render, utils::write_stylized_block};

#[derive(Debug)]
pub struct CommentContext {
    pub is_selected: bool,
}

impl Render for Comment {
    type Context = CommentContext;

    fn render(&self, buf: &mut String, app: &App, ctx: Self::Context) -> Result<()> {
        let mut comment = String::with_capacity(self.body.len());

        let time_delta = TimeDelta::from_std(app.delta())?;

        write!(
            comment,
            "{} {}\n",
            self.user
                .as_ref()
                .map(|a| a.login.as_str())
                .unwrap_or_else(|| "(unknown)")
                .with(app.color_scheme.author.into()),
            self.created_at
                .checked_add_signed(time_delta)
                .unwrap_or(self.created_at)
                .humanize()
                .with(app.color_scheme.muted.into()),
        )?;

        for line in textwrap::wrap(&self.body, 80) {
            writeln!(
                comment,
                "{}",
                line.with(app.color_scheme.comment_body.into())
            )?;
        }

        write_stylized_block(
            buf,
            comment,
            if ctx.is_selected {
                app.color_scheme.border_active.into()
            } else {
                app.color_scheme.border.into()
            },
        )?;

        Ok(())
    }
}
