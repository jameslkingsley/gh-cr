use anyhow::Result;
use octocrab::models::CommentId;
use tokio::sync::{mpsc, watch};

use crate::GH;
use crate::github::threads::PullRequestThreadsResponse;

/// Jobs sent to the queue to be processed
#[derive(Debug)]
pub enum Task {
    /// Posts a review comment on a pull request
    PostReviewComment {
        owner: String,
        repo: String,
        pr_number: u64,
        comment_id: u64,
        body: String,
    },

    /// Fetches the comment threads for a pull request
    FetchCommentThreads {
        owner: String,
        repo: String,
        pr_number: u64,
    },
}

/// Signals sent from the worker during job execution
#[derive(Debug)]
pub enum Signal {
    /// Indicates that the comment threads have been fetched
    CommentThreads { data: PullRequestThreadsResponse },
}

/// Status of the job queue
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Indicates that the queue is idle
    Idle,

    /// Indicates that the queue is working
    Working,
}

#[derive(Debug)]
pub struct Worker {
    job_rx: mpsc::Receiver<Task>,
    signal_tx: mpsc::Sender<Signal>,
    status_tx: watch::Sender<Status>,
}

impl Worker {
    pub fn spawn() -> Tasks {
        let (job_tx, job_rx) = mpsc::channel(8);
        let (signal_tx, signal_rx) = mpsc::channel(8);
        let (status_tx, status_rx) = watch::channel(Status::Idle);

        tokio::spawn(
            Worker {
                job_rx,
                signal_tx,
                status_tx,
            }
            .run(),
        );

        Tasks {
            job_tx,
            signal_rx,
            status_rx,
        }
    }

    async fn run(mut self) {
        while let Some(job) = self.job_rx.recv().await {
            let _ = self.status_tx.send(Status::Working);

            if let Err(err) = self.handle_job(job).await {
                eprintln!("task queue error: {err:?}");
            }

            if self.job_rx.is_empty() {
                let _ = self.status_tx.send(Status::Idle);
            }
        }
    }

    async fn handle_job(&self, job: Task) -> Result<()> {
        match job {
            Task::PostReviewComment {
                owner,
                repo,
                pr_number,
                comment_id,
                body,
            } => {
                let Some(github) = GH.get() else {
                    unreachable!("github client not set")
                };

                github
                    .pulls(&owner, &repo)
                    .reply_to_comment(pr_number, CommentId(comment_id), body)
                    .await?;

                let data = github.fetch_threads(&owner, &repo, pr_number).await?;
                let _ = self.signal_tx.send(Signal::CommentThreads { data }).await;

                Ok(())
            }
            Task::FetchCommentThreads {
                owner,
                repo,
                pr_number,
            } => {
                let Some(github) = GH.get() else {
                    unreachable!("github client not set")
                };
                let data = github.fetch_threads(&owner, &repo, pr_number).await?;
                let _ = self.signal_tx.send(Signal::CommentThreads { data }).await;
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
pub struct Tasks {
    job_tx: mpsc::Sender<Task>,
    signal_rx: mpsc::Receiver<Signal>,
    status_rx: watch::Receiver<Status>,
}

impl Tasks {
    pub fn send(&self, job: Task) -> Result<()> {
        self.job_tx.try_send(job).map_err(|e| e.into())
    }

    pub fn signals(&mut self) -> &mut mpsc::Receiver<Signal> {
        &mut self.signal_rx
    }

    pub fn status(&mut self) -> &mut watch::Receiver<Status> {
        &mut self.status_rx
    }
}
