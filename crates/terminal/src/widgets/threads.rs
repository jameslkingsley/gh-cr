use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, VecDeque},
    fmt::Write,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use crossterm::{
    event::{Event, KeyCode, KeyModifiers},
    style::Stylize,
};
use octocrab::{
    models::{CommentId, pulls::Comment},
    params::{Direction, pulls::comments::Sort},
};
use tokio::sync::oneshot::Receiver;

use crate::{
    app::Context,
    utils::{suspend_for_editor, write_stylized_block},
    widget_task,
    widgets::Widget,
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

#[derive(Debug)]
pub struct Threads {
    threads: BTreeMap<ThreadKey, Vec<Comment>>,
    comment_queue: VecDeque<PendingComment>,
    current_thread: Option<ThreadKey>,
    current_comment: Option<usize>,
    async_threads: Option<Receiver<BTreeMap<ThreadKey, Vec<Comment>>>>,
}

#[derive(Debug)]
pub struct PendingComment {
    pub in_reply_to: CommentId,
    pub content: String,
}

impl Default for Threads {
    fn default() -> Self {
        Self {
            threads: BTreeMap::new(),
            comment_queue: VecDeque::new(),
            current_thread: None,
            current_comment: None,
            async_threads: None,
        }
    }
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

    pub fn current_thread(&self) -> Result<(&ThreadKey, &[Comment])> {
        let Some(current) = &self.current_thread else {
            return Err(anyhow!("Current thread is None"));
        };

        self.threads
            .get_key_value(&current)
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

impl Widget for Threads {
    fn tick(&mut self, event: &Event, app: &Context) -> Result<bool> {
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

    fn render(&mut self, buf: &mut String, app: &Context) -> Result<()> {
        let (_, comments) = self.current_thread()?;
        let mut comments = comments.to_owned();
        let first = &comments[0];

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

        for (_index, comment) in comments.iter_mut().enumerate() {
            comment.render(buf, app)?;
            writeln!(buf)?;
        }

        let mut controls = String::new();

        write!(controls, "  ←/→ thread")?;
        write!(controls, "  (r)eply")?;
        write!(controls, "  (q)uit")?;

        let thread_count = self.threads.len();
        let current_thread = self.current_thread_index()?;
        write!(
            controls,
            "  {}/{} thread{}",
            current_thread + 1,
            thread_count,
            if thread_count > 1 { "s" } else { "" }
        )?;

        writeln!(buf, "{}", controls.with(app.color_scheme.muted.into()))?;

        for pending_comment in self.comment_queue.iter() {
            writeln!(
                buf,
                "- {} -> {}",
                pending_comment.in_reply_to, pending_comment.content
            )?;
        }

        Ok(())
    }

    fn init(&mut self, app: &Context) -> Result<()> {
        widget_task!(self.async_threads => async move {
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
                    .await.unwrap();

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

            let mut result: BTreeMap<ThreadKey, Vec<Comment>> = Default::default();

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

    fn poll_async(&mut self, _app: &Context) -> Result<()> {
        widget_task!(self.async_threads, |threads| {
            self.threads = threads;
        });
        Ok(())
    }

    fn dirty(&self) -> bool {
        true
    }
}
