use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use chrono_humanize::Humanize;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use octocrab::{
    models::CommentId,
    params::{Direction, pulls::comments::Sort},
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{List, ListItem, StatefulWidgetRef, Widget},
};
use tokio::sync::oneshot::Receiver;

use crate::{
    actor_task,
    actors::{Actor, comment::ThreadComment},
    app::Context,
    utils::{stylize_block, suspend_for_editor},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadKey {
    id: CommentId,
    ts: DateTime<Utc>,
}

impl Ord for ThreadKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ts.cmp(&other.ts)
    }
}

impl PartialOrd for ThreadKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default)]
pub struct Threads {
    threads: BTreeMap<ThreadKey, Vec<ThreadComment>>,
    comment_queue: VecDeque<PendingComment>,
    current_thread: Option<ThreadKey>,
    current_comment: Option<usize>,
    async_threads: Option<Receiver<BTreeMap<ThreadKey, Vec<ThreadComment>>>>,
}

#[derive(Debug)]
pub struct PendingComment {
    pub in_reply_to: CommentId,
    pub content: String,
}

impl Threads {
    pub fn queue_reply(&mut self, comment: PendingComment) -> Result<()> {
        self.comment_queue.push_back(comment);
        Ok(())
    }

    pub fn current_thread_index(&self) -> Result<usize> {
        let (thread_key, _) = self.current_thread()?;
        self.threads
            .iter()
            .position(|(k, _)| k == thread_key)
            .ok_or_else(|| anyhow!("Thread not found"))
    }

    pub fn current_thread(&self) -> Result<(&ThreadKey, &[ThreadComment])> {
        let Some(current) = &self.current_thread else {
            let Some(first) = self.threads.first_key_value() else {
                return Err(anyhow!("Threads tree is empty"));
            };

            return Ok((first.0, first.1.as_slice()));
        };

        self.threads
            .get_key_value(current)
            .map(|(k, v)| (k, v.as_slice()))
            .ok_or_else(|| anyhow!("Thread not found"))
    }

    pub fn next_thread(&mut self) -> Result<()> {
        match self.threads.iter().nth(self.current_thread_index()? + 1) {
            Some((next, _)) => self.current_thread.replace(*next),
            None => self.current_thread.replace(
                self.threads
                    .first_key_value()
                    .map(|(k, _)| *k)
                    .ok_or_else(|| anyhow!("threads empty"))?,
            ),
        };

        self.current_comment = None;

        Ok(())
    }

    pub fn prev_thread(&mut self) -> Result<()> {
        let prev_index = self
            .current_thread_index()?
            .checked_sub(1)
            .unwrap_or(self.threads.len() - 1);

        match self.threads.iter().nth(prev_index) {
            Some((prev, _)) => self.current_thread.replace(*prev),
            None => self.current_thread.replace(
                self.threads
                    .last_key_value()
                    .map(|(k, _)| *k)
                    .ok_or_else(|| anyhow!("threads empty"))?,
            ),
        };

        self.current_comment = None;

        Ok(())
    }

    pub fn next_comment(&mut self) -> Result<()> {
        let (_, comments) = self.current_thread()?;

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + 1) % comments.len(),
            None => 0,
        });

        Ok(())
    }

    pub fn prev_comment(&mut self) -> Result<()> {
        let (_, comments) = self.current_thread()?;

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + comments.len() - 1) % comments.len(),
            None => comments.len() - 1,
        });

        Ok(())
    }
}

impl Actor for Threads {
    fn tick(&mut self, event: &Event, _ctx: Arc<Context>) -> Result<bool> {
        // Next thread
        if let Event::Key(key) = event
            && key.code == KeyCode::Right
        {
            self.next_thread()?;
        }

        // Previous thread
        if let Event::Key(key) = event
            && key.code == KeyCode::Left
        {
            self.prev_thread()?;
        }

        // Next comment
        if let Event::Key(key) = event
            && key.code == KeyCode::Down
            && key.modifiers.contains(KeyModifiers::ALT)
        {
            self.next_comment()?;
        }

        // Previous comment
        if let Event::Key(key) = event
            && key.code == KeyCode::Up
            && key.modifiers.contains(KeyModifiers::ALT)
        {
            self.prev_comment()?;
        }

        // Reply
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('r')
        {
            let (thread_key, _) = self.current_thread()?;
            let _shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
            // TODO: Set initial contents to comment thread
            let content = suspend_for_editor(String::new())?;
            self.queue_reply(PendingComment {
                in_reply_to: thread_key.id,
                content,
            })?;
        }

        Ok(false)
    }

    fn init(&mut self, ctx: Arc<Context>) -> Result<()> {
        actor_task!(self.async_threads => async move {
            let mut page = Some(1u32);
            let mut threads: HashMap<CommentId, Vec<ThreadComment>> = HashMap::new();

            while let Some(page_number) = page {
                let res = ctx
                    .github
                    .pulls()
                    .list_comments(Some(ctx.github.pr.number))
                    .direction(Direction::Ascending)
                    .sort(Sort::Created)
                    .per_page(100)
                    .page(page_number)
                    .send()
                    .await.unwrap();

                for item in res.items {
                    threads
                        .entry(item.in_reply_to_id.unwrap_or(item.id))
                        .or_default()
                        .push(ThreadComment(item));
                }

                if res.incomplete_results.is_some_and(|v| v) {
                    page.replace(page_number + 1);
                } else {
                    page = None;
                }
            }

            let mut result: BTreeMap<ThreadKey, Vec<ThreadComment>> = Default::default();

            for (thread_id, mut comments) in threads {
                assert!(!comments.is_empty());
                comments.sort_unstable_by_key(|c| c.created_at);

                let key = ThreadKey {
                    id: thread_id,
                    ts: comments[0].created_at,
                };

                result.insert(key, comments);
            }

            result
        });
        Ok(())
    }

    fn poll_async(&mut self, _ctx: Arc<Context>) -> Result<()> {
        actor_task!(self.async_threads, |threads| {
            self.threads = threads;
        });
        Ok(())
    }

    fn dirty(&self) -> bool {
        false
    }
}

impl StatefulWidgetRef for Threads {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let Ok((_, comments)) = self.current_thread() else {
            return;
        };

        if comments.is_empty() {
            return;
        }

        let list = List::new(comments);

        list.render(area, buf);
    }
}

impl<'a> From<&'a ThreadComment> for ListItem<'a> {
    fn from(c: &'a ThreadComment) -> Self {
        let created_at = c.created_at.humanize();
        let author = c
            .user
            .as_ref()
            .map(|a| a.login.to_owned())
            .unwrap_or("(unknown)".to_owned());

        let mut content = Text::default();

        content.push_span(Span::styled(author, Style::new().cyan()));
        content.push_span(" ");
        content.push_span(Span::styled(created_at, Style::new().dark_gray()));

        let mut in_code_block = false;
        let mut current_text = String::new();

        for line in c.body.lines() {
            if line.trim_start().starts_with("```") {
                // Process accumulated text before code block
                if !current_text.is_empty() && !in_code_block {
                    for wrapped_line in textwrap::wrap(&current_text, 80) {
                        content.push_line(
                            Line::raw(wrapped_line.into_owned()).style(Style::new().gray()),
                        );
                    }
                    current_text.clear();
                }

                // Toggle code block state and add the line as-is
                in_code_block = !in_code_block;
                content.push_line(Line::raw(line).style(Style::new().dark_gray()));
            } else if in_code_block {
                // In code block: add line as-is without wrapping
                content.push_line(Line::raw(line).style(Style::new().dark_gray()));
            } else {
                // Not in code block: accumulate text for wrapping
                if !current_text.is_empty() {
                    current_text.push('\n');
                }
                current_text.push_str(line);
            }
        }

        // Process any remaining accumulated text
        if !current_text.is_empty() {
            for wrapped_line in textwrap::wrap(&current_text, 80) {
                content.push_line(Line::raw(wrapped_line.into_owned()).style(Style::new().gray()));
            }
        }

        stylize_block(&mut content, Style::new().dark_gray());

        // Spacer
        content.push_line("");

        ListItem::new(content)
    }
}
