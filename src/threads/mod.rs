use std::{fmt::Write, pin::Pin};

use anyhow::Result;
use chrono::TimeDelta;
use chrono_humanize::Humanize;
use crossterm::{event::Event, style::Stylize};
use octocrab::{
    models::pulls::Comment,
    params::{Direction, pulls::comments::Sort},
};

use crate::{
    app::{App, Tick},
    components::{Component, Render},
    utils::write_stylized_block,
};

#[derive(Debug)]
pub struct Threads {
    comments: Vec<Comment>,
    stale: bool,
}

impl Default for Threads {
    fn default() -> Self {
        Self {
            comments: Vec::new(),
            stale: true,
        }
    }
}

impl Threads {
    pub async fn reply(&self) -> Result<()> {
        Ok(())
    }
}

impl Render for Comment {
    fn render(&self, buf: &mut String, app: &App) -> Result<()> {
        let mut comment = String::with_capacity(self.body.len());

        let time_delta = TimeDelta::from_std(app.delta())?;

        write!(
            comment,
            "{} {}\n",
            self.user
                .as_ref()
                .map(|a| a.login.as_str())
                .unwrap_or_else(|| "(unknown)")
                .with(app.color_scheme.author),
            self.created_at
                .checked_add_signed(time_delta)
                .unwrap_or(self.created_at)
                .humanize()
                .with(app.color_scheme.timestamp),
        )?;

        for line in textwrap::wrap(&self.body, 80) {
            writeln!(comment, "{}", line.with(app.color_scheme.comment_body))?;
        }

        write_stylized_block(buf, comment, app.color_scheme.borders)?;

        Ok(())
    }
}

impl Component for Threads {
    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick> {
        Ok(Tick::Noop)
    }

    fn render(&self, buf: &mut String, app: &App) -> Result<()> {
        for comment in &self.comments {
            comment.render(buf, app)?;
            writeln!(buf)?;
        }

        Ok(())
    }

    fn tick_async<'a>(
        &'a mut self,
        app: &'a App,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if self.stale {
                let mut page = Some(1u32);

                while let Some(page_number) = page {
                    let res = app
                        .github
                        .pulls()
                        .list_comments(Some(app.github.pr()))
                        .direction(Direction::Ascending)
                        .sort(Sort::Created)
                        .per_page(100)
                        .page(page_number)
                        .send()
                        .await?;

                    self.comments.extend(res.items);

                    if res.incomplete_results.is_some_and(|v| v) {
                        page.replace(page_number + 1);
                    } else {
                        page = None;
                    }
                }

                self.stale = false;
            }

            Ok(())
        })
    }
}
