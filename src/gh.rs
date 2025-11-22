use std::ffi::{OsStr, OsString};

use anyhow::{Result, anyhow};
use serde::Deserialize;
use tokio::process::Command;

pub struct GitHub;

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl GitHub {
    async fn pr(&self) -> Result<PullRequest> {
        let output = self
            .invoke(["repo", "view", "--json", "name,owner"])
            .await?;

        let repo: PullRequest = serde_json::from_str(&output)?;

        Ok(PullRequest {
            owner: todo!(),
            repo: todo!(),
            number: todo!(),
        })
    }

    async fn current_pr_number(&self) -> Result<u64> {
        let output = self
            .run(["pr", "view", "--json", "number"])
            .await
            .context("gh pr view failed")?;
        let pr: PrResponse = serde_json::from_str(&output).context("failed to parse PR info")?;
        Ok(pr.number)
    }

    async fn invoke<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = Command::new("gh").args(args).output().await?;

        if !output.status.success() {
            return Err(anyhow!(
                "gh invocation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8(output.stdout)?;

        Ok(stdout)
    }
}
