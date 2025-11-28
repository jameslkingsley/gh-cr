pub mod guess_pr;

mod service;

use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use octocrab::{
    AuthState, Octocrab, OctocrabBuilder,
    models::{Repository, pulls::PullRequest},
    pulls::PullRequestHandler,
};

use service::GitHubCLI;

pub fn build_octocrab_client() -> Result<Octocrab, Infallible> {
    OctocrabBuilder::new_empty()
        .with_service(GitHubCLI)
        .with_auth(AuthState::None)
        .build()
}

#[derive(Debug)]
pub struct GitHub<State = Pending> {
    state: State,
}

#[derive(Debug)]
pub struct Pending {
    owner: String,
    repo: String,
    pr: u64,
}

#[derive(Debug)]
pub struct Initialised {
    pub repo: Repository,
    pub pr: PullRequest,
    client: Octocrab,
}

impl<T> Deref for GitHub<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl<T> DerefMut for GitHub<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl Default for GitHub<Pending> {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHub<Pending> {
    pub fn new() -> Self {
        Self {
            state: Pending {
                owner: String::new(),
                repo: String::new(),
                pr: 0,
            },
        }
    }

    pub fn owner(mut self, owner: String) -> Self {
        self.owner = owner;
        self
    }

    pub fn repo(mut self, repo: String) -> Self {
        self.repo = repo;
        self
    }

    pub fn pr(mut self, pr: u64) -> Self {
        self.pr = pr;
        self
    }

    pub async fn build(self) -> Result<GitHub<Initialised>> {
        let client = build_octocrab_client().expect("octocrab builder infallible");

        let repo = client.repos(&self.owner, &self.repo).get().await?;
        let pr = client.pulls(&self.owner, &self.repo).get(self.pr).await?;

        Ok(GitHub {
            state: Initialised { repo, pr, client },
        })
    }
}

impl GitHub<Initialised> {
    pub fn pulls(&self) -> PullRequestHandler<'_> {
        self.client.pulls(
            self.repo
                .owner
                .as_ref()
                .expect("invalid repo")
                .login
                .as_str(),
            self.repo.name.as_str(),
        )
    }
}
