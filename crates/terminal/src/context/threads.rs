use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, VecDeque},
    ops::Deref,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use futures::FutureExt;
use octocrab::{
    models::{
        CommentId,
        pulls::{Comment, PullRequest},
    },
    params::{Direction, pulls::comments::Sort},
};
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use tokio::sync::oneshot::{self, Receiver};

use crate::{GH, utils::suspend_for_editor};

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
    recv_threads: Option<Receiver<BTreeMap<ThreadKey, Vec<ThreadComment>>>>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct PendingComment {
    pub in_reply_to: CommentId,
    pub content: String,
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
        // Stylize block consumes 2 columns; clamp to a sensible range.
        let available = area_width.saturating_sub(2).max(1);
        usize::from(available).min(80)
    }

    pub fn sanitised_body(&self) -> String {
        // Fixes ghost characters left by tabs
        self.body.replace("\t", "    ")
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

    pub fn tick(&mut self, event: &Event) -> Result<bool> {
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

    pub fn fetch(&mut self, pr: &PullRequest) -> Result<()> {
        let Some(owner) = pr.user.as_ref().map(|o| o.login.clone()) else {
            return Err(anyhow!("missing pr owner: {:?}", pr));
        };

        let Some(repo) = pr.repo.as_ref().map(|r| r.name.clone()) else {
            return Err(anyhow!("missing pr repo: {:?}", pr));
        };

        let pr_number = pr.number;

        let (tx, rx) = oneshot::channel();

        tokio::spawn(async move {
            let Some(github) = GH.get() else {
                unreachable!("github client not set")
            };

            let mut page = Some(1u32);
            let mut threads: HashMap<CommentId, Vec<ThreadComment>> = HashMap::new();

            while let Some(page_number) = page {
                let res = github
                    .pulls(&owner, &repo)
                    .list_comments(Some(pr_number))
                    .direction(Direction::Ascending)
                    .sort(Sort::Created)
                    .per_page(100)
                    .page(page_number)
                    .send()
                    .await
                    .unwrap();

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

            let _ = tx.send(result);
        });

        self.recv_threads = Some(rx);

        Ok(())
    }

    pub fn poll_async(&mut self) -> Result<()> {
        if let Some(rx) = &mut self.recv_threads {
            if let Some(Ok(value)) = rx.now_or_never() {
                self.recv_threads = None;
                self.threads = value;
            }
        }
        Ok(())
    }
}
