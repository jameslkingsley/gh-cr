pub mod threads;

use anyhow::{Error, Result};
use futures::{FutureExt, future::try_join};
use github::guess_pr::guess_pull_request;
use octocrab::models::{Repository, pulls::PullRequest};
use ratatui::crossterm::event::Event;
use tokio::sync::oneshot::{self, Receiver};

use crate::{Cli, GH, context::threads::Threads};

#[derive(Debug)]
pub struct Context {
    pub args: Cli,
    pub repo: Option<Repository>,
    pub pr: Option<PullRequest>,
    pub threads: Threads,
    recv: Option<Receiver<(Repository, PullRequest)>>,
}

impl Context {
    pub fn new(args: Cli) -> Self {
        Context {
            args,
            repo: None,
            pr: None,
            threads: Threads::default(),
            recv: None,
        }
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
                        let guessed = guess_pull_request().await?;
                        (
                            args.owner.unwrap_or(guessed.owner),
                            args.repo.unwrap_or(guessed.repo),
                            args.pr.unwrap_or(guessed.pr),
                        )
                    } else {
                        (args.owner.unwrap(), args.repo.unwrap(), args.pr.unwrap())
                    };

                let (repo, mut pr) = try_join(
                    github.repos(&owner_str, &repo_str).get(),
                    github.pulls(&owner_str, &repo_str).get(pr_num),
                )
                .await?;

                pr.repo = Some(Box::new(repo.clone()));

                let _ = tx.send((repo, pr));

                Ok::<(), Error>(())
            }
        });

        self.recv = Some(rx);

        Ok(())
    }

    pub fn tick(&mut self, event: &Event) -> Result<bool> {
        if self.threads.tick(event)? {
            return Ok(true);
        }
        Ok(false)
    }

    pub fn poll_async(&mut self) -> Result<()> {
        if let Some(rx) = &mut self.recv {
            if let Some(Ok((repo, pr))) = rx.now_or_never() {
                // Repo or pull request has changed, so fetch threads
                self.threads.fetch(&pr)?;
                self.recv = None;
                self.repo = Some(repo);
                self.pr = Some(pr);
            }
        }

        self.threads.poll_async()?;

        Ok(())
    }
}
