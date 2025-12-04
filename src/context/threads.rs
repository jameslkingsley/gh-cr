use anyhow::{Result, anyhow};
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{
    context::tasks::{Task, Tasks},
    github::threads::{PullRequestThreadsResponse, ReviewThread},
    states::AppState,
    utils::dirty,
};

#[derive(Debug, Default)]
pub struct Threads {
    data: Option<PullRequestThreadsResponse>,
    current_thread: usize,
    current_comment: Option<usize>,
}

impl Threads {
    pub fn replace_threads(&mut self, data: PullRequestThreadsResponse) {
        if !data
            .pull_request
            .review_threads
            .nodes
            .get(self.current_thread)
            .is_none()
        {
            self.current_thread = 0;
        }

        self.data = Some(data);
        self.current_comment = None;
        dirty();
    }

    pub fn thread_len(&self) -> usize {
        self.data
            .as_ref()
            .map(|d| d.pull_request.review_threads.nodes.len())
            .unwrap_or(0)
    }

    pub fn current_thread_index(&self) -> usize {
        match self.data.as_ref() {
            Some(data) => {
                assert!(self.current_thread < data.pull_request.review_threads.nodes.len());
                self.current_thread
            }
            None => {
                assert_eq!(0, self.current_thread);
                self.current_thread
            }
        }
    }

    pub fn current_thread(&self) -> Option<&ReviewThread> {
        self.data
            .as_ref()
            .map(|d| &d.pull_request.review_threads.nodes[self.current_thread_index()])
    }

    pub fn next_thread(&mut self) -> Result<()> {
        if self.current_thread >= self.thread_len() - 1 {
            self.current_thread = 0;
        } else {
            self.current_thread += 1;
        }

        self.current_comment = None;
        dirty();

        Ok(())
    }

    pub fn prev_thread(&mut self) -> Result<()> {
        if self.current_thread == 0 {
            self.current_thread = self.thread_len() - 1;
        } else {
            self.current_thread -= 1;
        }

        self.current_comment = None;
        dirty();

        Ok(())
    }

    pub fn next_comment(&mut self) -> Result<()> {
        let Some(thread) = self.current_thread() else {
            return Ok(());
        };

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + 1) % thread.comments.nodes.len(),
            None => 0,
        });
        dirty();

        Ok(())
    }

    pub fn prev_comment(&mut self) -> Result<()> {
        let Some(thread) = self.current_thread() else {
            return Ok(());
        };

        self.current_comment = Some(match self.current_comment {
            Some(i) => (i + thread.comments.nodes.len() - 1) % thread.comments.nodes.len(),
            None => thread.comments.nodes.len() - 1,
        });
        dirty();

        Ok(())
    }

    pub fn tick(&mut self, event: &Event, state: &mut AppState, tasks: &Tasks) -> Result<bool> {
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
            if let Some(thread) = self.current_thread() {
                let Some(data) = self.data.as_ref() else {
                    return Err(anyhow!("threads not set"));
                };
                // TODO: Set initial contents to comment thread
                let content = state.suspend_for_editor(String::new())?;
                tasks.send(Task::PostReviewComment {
                    owner: data.owner.login.to_owned(),
                    repo: data.name.to_owned(),
                    pr_number: data.pull_request.number,
                    comment_id: thread
                        .comments
                        .nodes
                        .iter()
                        .max()
                        .map(|c| c.full_database_id)
                        .expect("missing comment"),
                    body: content,
                })?;
            }
        }

        Ok(false)
    }
}
