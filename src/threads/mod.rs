use std::{collections::HashMap, fmt::Write, pin::Pin};

use anyhow::Result;
use chrono::TimeDelta;
use chrono_humanize::Humanize;
use crossterm::{
    event::{Event, KeyCode, KeyModifiers},
    style::Stylize,
};
use octocrab::{
    models::{CommentId, pulls::Comment},
    params::{Direction, pulls::comments::Sort},
};

use crate::{
    app::{App, Tick},
    components::{Component, Render},
    utils::write_stylized_block,
};

type Comments = Vec<Comment>;

#[derive(Debug)]
pub struct Threads {
    current_thread: usize,
    current_comment: Option<usize>,
    threads: Vec<Comments>,
    stale: bool,
}

impl Default for Threads {
    fn default() -> Self {
        Self {
            current_thread: 0,
            current_comment: None,
            threads: Vec::new(),
            stale: true,
        }
    }
}

impl Threads {
    pub async fn _reply(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct CommentContext {
    is_selected: bool,
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

impl Component for Threads {
    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick> {
        // Next thread
        if let Event::Key(key) = event
            && key.code == KeyCode::Right
        {
            self.current_thread = (self.current_thread + 1) % self.threads.len();
            self.current_comment = None;
            return Ok(Tick::Render);
        }

        // Previous thread
        if let Event::Key(key) = event
            && key.code == KeyCode::Left
        {
            self.current_thread =
                (self.current_thread + self.threads.len() - 1) % self.threads.len();
            self.current_comment = None;
            return Ok(Tick::Render);
        }

        // Next comment
        if let Event::Key(key) = event
            && key.code == KeyCode::Down
            && key.modifiers.contains(KeyModifiers::ALT)
        {
            self.current_comment = Some(match self.current_comment {
                Some(i) => (i + 1) % self.threads[self.current_thread].len(),
                None => 0,
            });
            return Ok(Tick::Render);
        }

        // Previous comment
        if let Event::Key(key) = event
            && key.code == KeyCode::Up
            && key.modifiers.contains(KeyModifiers::ALT)
        {
            self.current_comment = Some(match self.current_comment {
                Some(i) => {
                    (i + self.threads[self.current_thread].len() - 1)
                        % self.threads[self.current_thread].len()
                }
                None => self.threads[self.current_thread].len() - 1,
            });
            return Ok(Tick::Render);
        }

        // Reply
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('r')
        {
            let _body = app.suspend_for_editor("initial_contents".to_string())?;
            return Ok(Tick::Render);
        }

        Ok(Tick::Noop)
    }

    fn render(&self, buf: &mut String, app: &App) -> Result<()> {
        let Some(comments) = self.threads.get(self.current_thread) else {
            return Ok(());
        };

        let Some(first) = comments.first() else {
            return Ok(());
        };

        writeln!(buf)?;

        let diff_hunk = first
            .diff_hunk
            .as_str()
            .lines()
            .map(|l| {
                let mut chars = l.chars();

                let Some(first) = chars.next() else {
                    return l.to_owned().with(app.color_scheme.diff_unchanged.into());
                };

                let rest: String = chars.collect();

                match first {
                    '+' => format!("+  {}", rest).with(app.color_scheme.diff_added.into()),
                    '-' => format!("-  {}", rest).with(app.color_scheme.diff_removed.into()),
                    _ => format!("  {}", l).with(app.color_scheme.diff_unchanged.into()),
                }
            })
            .try_fold(
                {
                    let mut s = String::new();
                    let c = app.color_scheme.comment_body.into();
                    write!(
                        s,
                        "{}{}{}\n\n",
                        first.path.as_str().with(c),
                        ":".with(c),
                        first
                            .line
                            .unwrap_or(first.original_line.unwrap_or(1))
                            .to_string()
                            .with(c)
                    )?;
                    s
                },
                |mut acc, line| -> Result<String> {
                    writeln!(acc, "{}", line)?;
                    Ok(acc)
                },
            )?;

        write_stylized_block(buf, diff_hunk, app.color_scheme.muted.into())?;
        writeln!(buf)?;

        for (index, comment) in comments.iter().enumerate() {
            comment.render(
                buf,
                app,
                CommentContext {
                    is_selected: self.current_comment.is_some_and(|i| i == index),
                },
            )?;
            writeln!(buf)?;
        }

        let mut controls = String::new();

        write!(controls, "  ←/→ thread")?;
        write!(controls, "  (r)eply")?;
        write!(controls, "  (q)uit")?;

        let thread_count = self.threads.len();
        let current_thread = self.current_thread + 1;
        write!(
            controls,
            "  {}/{} thread{}",
            current_thread,
            thread_count,
            if thread_count > 1 { "s" } else { "" }
        )?;

        writeln!(buf, "{}", controls.with(app.color_scheme.muted.into()))?;

        Ok(())
    }

    fn tick_async<'a>(
        &'a mut self,
        app: &'a App,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if self.stale {
                let mut page = Some(1u32);
                let mut threads: HashMap<CommentId, Vec<Comment>> = HashMap::new();

                while let Some(page_number) = page {
                    let res = app
                        .github
                        .pulls()
                        .list_comments(Some(app.github.pr.number))
                        .direction(Direction::Ascending)
                        .sort(Sort::Created)
                        .per_page(100)
                        .page(page_number)
                        .send()
                        .await?;

                    for item in res.items {
                        threads
                            .entry(item.in_reply_to_id.unwrap_or(item.id))
                            .or_default()
                            .push(item);
                    }

                    if res.incomplete_results.is_some_and(|v| v) {
                        page.replace(page_number + 1);
                    } else {
                        page = None;
                    }
                }

                self.threads = Vec::with_capacity(threads.len());

                for (_, mut comments) in threads {
                    assert!(!comments.is_empty());
                    comments.sort_unstable_by_key(|c| c.created_at);
                    self.threads.push(comments);
                }

                self.threads.sort_unstable_by_key(|t| {
                    t.iter().min_by_key(|c| c.created_at).map(|c| c.created_at)
                });

                self.stale = false;
            }

            Ok(())
        })
    }
}
