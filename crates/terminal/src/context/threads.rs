use std::{cmp::Ordering, collections::BTreeMap, ops::Deref};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use github::PullRequestExt;
use octocrab::models::{
    CommentId,
    pulls::{Comment, PullRequest},
};
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{
    context::tasks::{Task, Tasks},
    states::AppState,
    utils::dirty,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadKey {
    pub id: CommentId,
    pub ts: DateTime<Utc>,
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
    current_thread: Option<ThreadKey>,
    current_comment: Option<usize>,
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
        // Fixes ghost characters left by tabs.
        // Probably more of a bug in Ratatui or Crossterm
        self.body.replace("\t", "    ")
    }
}

impl Threads {
    pub fn replace_threads(&mut self, threads: BTreeMap<ThreadKey, Vec<ThreadComment>>) {
        if let Some(key) = self.current_thread.as_ref() {
            if !threads.contains_key(key) {
                self.current_thread = threads.first_key_value().map(|(k, _)| *k);
            }
        }

        self.threads = threads;
        self.current_comment = None;
        dirty();
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
        dirty();

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
        dirty();

        Ok(())
    }

    pub fn next_comment(&mut self) -> Result<()> {
        let (_, comments) = self.current_thread()?;

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + 1) % comments.len(),
            None => 0,
        });
        dirty();

        Ok(())
    }

    pub fn prev_comment(&mut self) -> Result<()> {
        let (_, comments) = self.current_thread()?;

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + comments.len() - 1) % comments.len(),
            None => comments.len() - 1,
        });
        dirty();

        Ok(())
    }

    pub fn tick(
        &mut self,
        event: &Event,
        state: &mut AppState,
        tasks: &Tasks,
        pr: Option<&PullRequest>,
    ) -> Result<bool> {
        // Toggle diffs
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('d')
        {
            state.show_diffs = !state.show_diffs;
            dirty();
        }

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
            let Some(pr) = pr else {
                return Err(anyhow!("pr is none"));
            };
            // TODO: Set initial contents to comment thread
            let content = state.suspend_for_editor(String::new())?;
            tasks.send(Task::PostReviewComment {
                owner: pr.owner().to_owned(),
                repo: pr.repo().to_owned(),
                pr_number: pr.number(),
                comment_id: thread_key.id,
                body: content,
            })?;
        }

        Ok(false)
    }
}
