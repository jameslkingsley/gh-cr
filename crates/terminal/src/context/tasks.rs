use std::collections::BTreeMap;

use octocrab::models::CommentId;
use tokio::sync::{mpsc, watch};

use crate::context::threads::{ThreadComment, ThreadKey};

/// Jobs sent to the queue to be processed
#[derive(Debug)]
pub enum Job {
    /// Posts a review comment on a pull request
    PostReviewComment {
        owner: String,
        repo: String,
        pr_number: u64,
        comment_id: CommentId,
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
    CommentThreads {
        map: BTreeMap<ThreadKey, Vec<ThreadComment>>,
    },
}

/// Status of the job queue
#[derive(Debug)]
pub enum Status {
    /// Indicates that the queue is idle
    Idle,

    /// Indicates that the queue is working
    Working,
}

#[derive(Debug)]
pub struct Queue {
    job_rx: mpsc::Receiver<Job>,
    signal_tx: mpsc::Sender<Signal>,
    status_tx: watch::Sender<Status>,
}
