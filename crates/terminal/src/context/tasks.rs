use std::collections::{BTreeMap, HashMap};

use anyhow::{Context as AnyhowContext, Result};
use octocrab::{
    models::CommentId,
    params::{Direction, pulls::comments::Sort},
};
use tokio::sync::{mpsc, watch};

use crate::{
    GH,
    context::threads::{ThreadComment, ThreadKey},
};

/// Jobs sent to the queue to be processed
#[derive(Debug)]
pub enum Task {
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
                    .reply_to_comment(pr_number, comment_id, body)
                    .await?;

                let map = fetch_comment_threads(&owner, &repo, pr_number).await?;
                let _ = self.signal_tx.send(Signal::CommentThreads { map }).await;

                Ok(())
            }
            Task::FetchCommentThreads {
                owner,
                repo,
                pr_number,
            } => {
                let map = fetch_comment_threads(&owner, &repo, pr_number).await?;
                let _ = self.signal_tx.send(Signal::CommentThreads { map }).await;
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

async fn fetch_comment_threads(
    owner: &str,
    repo: &str,
    pr_number: u64,
) -> Result<BTreeMap<ThreadKey, Vec<ThreadComment>>> {
    let Some(github) = GH.get() else {
        unreachable!("github client not set")
    };

    let mut page = Some(1u32);
    let mut threads: HashMap<CommentId, Vec<ThreadComment>> = HashMap::new();

    // TODO:
    // - Manual GraphQL to get all comments, their resolved status (if review), and any review bodies
    // - Abstraction for threads, so octocrab is only in github crate, allow for other services to be added

    while let Some(page_number) = page {
        let res = github
            .pulls(owner, repo)
            .list_comments(Some(pr_number))
            .direction(Direction::Ascending)
            .sort(Sort::Created)
            .per_page(100)
            .page(page_number)
            .send()
            .await
            .context(format!("prs for {owner}/{repo}/{pr_number}"))?;

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
        if comments.is_empty() {
            continue;
        }

        comments.sort_unstable_by_key(|c| c.created_at);

        let key = ThreadKey {
            id: thread_id,
            ts: comments[0].created_at,
        };

        result.insert(key, comments);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use github::GitHub;
    use serde_json::{Value, json};
    use testresult::TestResult;

    #[tokio::test]
    async fn test_github_graphql() -> TestResult {
        let gh = GitHub::new();

        let res: Value = gh
            .graphql(&json!({
                "query": format!(r#"
                query {{
                    repository(owner: "jameslkingsley", name: "gh-cr") {{
                        name
                        owner {{
                            login
                        }}
                        pullRequest(number: 2) {{
                            number
                            author {{
                                login
                            }}
                            comments(first: 100) {{
                                nodes {{
                                    author {{
                                        login
                                    }}
                                    body
                                    createdAt
                                    fullDatabaseId
                                    id
                                }}
                                pageInfo {{
                                    endCursor
                                    startCursor
                                    hasNextPage
                                    hasPreviousPage
                                }}
                            }}
                            reviewThreads(first: 100) {{
                                nodes {{
                                    id
                                    isResolved
                                    path
                                    comments(first: 100) {{
                                        nodes {{
                                            author {{
                                                login
                                            }}
                                            body
                                            createdAt
                                            diffHunk
                                            fullDatabaseId
                                            id
                                            line
                                            originalLine
                                            originalStartLine
                                            outdated
                                            path
                                            repository {{
                                                name
                                                owner {{
                                                    login
                                                }}
                                            }}
                                            startLine
                                            subjectType
                                        }}
                                    }}
                                }}
                                pageInfo {{
                                    endCursor
                                    startCursor
                                    hasNextPage
                                    hasPreviousPage
                                }}
                            }}
                        }}
                    }}
                }}"#)
            }))
            .await?;

        dbg!(res);

        Ok(())
    }
}
