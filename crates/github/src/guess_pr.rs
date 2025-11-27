use anyhow::{Result, anyhow};
use serde_json::Value;
use std::ffi::OsStr;
use tokio::process::Command;

pub struct GuessedPullRequest {
    pub pr: u64,
    pub owner: String,
    pub repo: String,
}

pub async fn guess_pull_request() -> Result<GuessedPullRequest> {
    let repo: Value =
        serde_json::from_str(&invoke_gh(["repo", "view", "--json", "name,owner"]).await?)?;

    let pr: Value = serde_json::from_str(&invoke_gh(["pr", "view", "--json", "number"]).await?)?;

    Ok(GuessedPullRequest {
        pr: pr["number"]
            .as_u64()
            .ok_or_else(|| anyhow!("invalid pr number"))?,
        owner: repo["owner"]["login"]
            .as_str()
            .ok_or_else(|| anyhow!("invalid owner"))?
            .to_string(),
        repo: repo["name"]
            .as_str()
            .ok_or_else(|| anyhow!("invalid repo"))?
            .to_string(),
    })
}

async fn invoke_gh<I, S>(args: I) -> Result<String>
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
