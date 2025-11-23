mod service;

use std::{convert::Infallible, marker::PhantomData, mem};

use anyhow::Result;
use octocrab::{AuthState, Octocrab, OctocrabBuilder, pulls::PullRequestHandler};

use service::GitHubCLI;

pub fn build_octocrab_client() -> Result<Octocrab, Infallible> {
    OctocrabBuilder::new_empty()
        .with_service(GitHubCLI)
        .with_auth(AuthState::None)
        .build()
}

#[derive(Debug)]
pub struct GitHub<State = Pending> {
    owner: String,
    repo: String,
    pr: i64,
    client: Octocrab,
    _state: PhantomData<State>,
}

#[derive(Debug)]
pub struct Pending;

#[derive(Debug)]
pub struct Initialised;

impl GitHub<Pending> {
    pub fn new() -> Self {
        Self {
            owner: String::new(),
            repo: String::new(),
            pr: 0,
            client: build_octocrab_client().expect("octocrab builder infallible"),
            _state: PhantomData,
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

    pub fn pr(mut self, pr: i64) -> Self {
        self.pr = pr;
        self
    }

    pub fn build(self) -> GitHub<Initialised> {
        unsafe { mem::transmute(self) }
    }
}

impl GitHub<Initialised> {
    pub fn pr(&self) -> u64 {
        self.pr as u64
    }

    pub fn pulls(&self) -> PullRequestHandler<'_> {
        self.client.pulls(&self.owner, &self.repo)
    }
}
