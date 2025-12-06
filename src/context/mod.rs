pub mod tasks;
pub mod threads;

use anyhow::{Error, Result, anyhow};
use futures::{FutureExt, future::try_join};
use octocrab::models::{Repository, pulls::PullRequest};
use ratatui::crossterm::event::Event;
use tokio::sync::oneshot::{self, Receiver};

use crate::{
    Cli, GH,
    context::{
        tasks::{Signal, Status, Task, Tasks, Worker},
        threads::Threads,
    },
    github::guess_pr::guess_pull_request,
    states::AppState,
    utils::dirty,
};

#[derive(Debug)]
pub struct Context {
    pub args: Cli,
    pub repo: Option<Repository>,
    pub pr: Option<PullRequest>,
    pub threads: Threads,
    tasks: Tasks,
    task_status: Status,
    recv: Option<Receiver<Result<(Repository, PullRequest)>>>,
}

impl Context {
    pub fn new(args: Cli) -> Self {
        Context {
            args,
            repo: None,
            pr: None,
            threads: Threads::default(),
            tasks: Worker::spawn(),
            task_status: Status::Idle,
            recv: None,
        }
    }

    pub fn is_working(&self) -> bool {
        self.task_status == Status::Working
    }

    pub fn is_loading(&self) -> bool {
        self.repo.is_none() || self.pr.is_none() || self.recv.is_some()
    }

    pub fn init(&mut self) -> Result<()> {
        let (tx, rx) = oneshot::channel();

        tokio::spawn({
            let args = self.args.clone();
            async move {
                let Some(github) = GH.get() else {
                    unreachable!("github client not set")
                };

                let (owner_str, repo_str, pr_num) =
                    if args.owner.is_none() || args.repo.is_none() || args.pr.is_none() {
                        match guess_pull_request().await {
                            Ok(guessed) => {
                                if guessed.pr.is_none() && args.pr.is_none() {
                                    let _ = tx.send(Err(anyhow!("No pull request specified")));
                                    return Ok(());
                                }

                                (
                                    args.owner.unwrap_or(guessed.owner),
                                    args.repo.unwrap_or(guessed.repo),
                                    args.pr.unwrap_or_else(|| guessed.pr.unwrap()),
                                )
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e));
                                return Ok(());
                            }
                        }
                    } else {
                        (args.owner.unwrap(), args.repo.unwrap(), args.pr.unwrap())
                    };

                let (repo, mut pr) = try_join(
                    github.repos(&owner_str, &repo_str).get(),
                    github.pulls(&owner_str, &repo_str).get(pr_num),
                )
                .await?;

                pr.repo = Some(Box::new(repo.clone()));

                let _ = tx.send(Ok((repo, pr)));

                Ok::<(), Error>(())
            }
        });

        self.recv = Some(rx);

        Ok(())
    }

    pub fn tick(&mut self, event: &Event, state: &mut AppState) -> Result<bool> {
        if self.threads.tick(event, state, &self.tasks)? {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn poll_async(&mut self) -> Result<()> {
        if let Some(rx) = &mut self.recv {
            if let Some(Ok(data)) = rx.now_or_never() {
                match data {
                    Ok((repo, pr)) => {
                        // Repo or pull request has changed, so fetch threads
                        self.enqueue_thread_fetch(&pr)?;
                        self.recv = None;
                        self.repo = Some(repo);
                        self.pr = Some(pr);
                        dirty();
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        while let Ok(signal) = self.tasks.signals().try_recv() {
            match signal {
                Signal::CommentThreads { data } => self.threads.replace_threads(data),
            }
        }

        {
            let status = self.tasks.status();
            if let Ok(true) = status.has_changed() {
                self.task_status = *status.borrow_and_update();
                dirty();
            }
        }

        Ok(())
    }

    fn enqueue_thread_fetch(&mut self, pr: &PullRequest) -> Result<()> {
        let Some(owner) = pr
            .repo
            .as_ref()
            .and_then(|r| Some(r.owner.as_ref()?.login.clone()))
        else {
            return Err(anyhow!("missing pr owner: {:?}", pr));
        };

        let Some(repo) = pr.repo.as_ref().map(|r| r.name.clone()) else {
            return Err(anyhow!("missing pr repo: {:?}", pr));
        };

        self.tasks.send(Task::FetchCommentThreads {
            owner,
            repo,
            pr_number: pr.number,
        })?;

        Ok(())
    }
}
