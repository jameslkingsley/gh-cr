pub mod guess_pr;

mod service;

use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
};

use anyhow::Result;
use octocrab::{AuthState, Octocrab, OctocrabBuilder, models::pulls::PullRequest};
use service::GitHubCLI;

pub fn build_octocrab_client() -> Result<Octocrab, Infallible> {
    OctocrabBuilder::new_empty()
        .with_service(GitHubCLI)
        .with_auth(AuthState::None)
        .build()
}

#[derive(Debug, Clone)]
pub struct GitHub {
    client: Octocrab,
}

impl GitHub {
    pub fn new() -> Self {
        let client = build_octocrab_client().expect("octocrab builder infallible");
        Self { client }
    }
}

impl Deref for GitHub {
    type Target = Octocrab;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for GitHub {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

pub trait PullRequestExt {
    fn owner(&self) -> &str;
    fn repo(&self) -> &str;
    fn number(&self) -> u64;
}

impl PullRequestExt for PullRequest {
    fn owner(&self) -> &str {
        self.repo
            .as_ref()
            .map(|r| r.owner.as_ref().unwrap().login.as_str())
            .unwrap()
    }

    fn repo(&self) -> &str {
        self.repo.as_ref().map(|r| r.name.as_str()).unwrap()
    }

    fn number(&self) -> u64 {
        self.number
    }
}
