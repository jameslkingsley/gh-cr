use std::ffi::OsStr;

use anyhow::{Result, anyhow};
use futures::future::try_join;
use serde_json::Value;
use tokio::process::Command;

pub struct GuessedPullRequest {
    pub pr: Option<u64>,
    pub owner: String,
    pub repo: String,
}

pub async fn guess_pull_request() -> Result<GuessedPullRequest> {
    let (repo, pr) = try_join(
        invoke_gh(["repo", "view", "--json", "name,owner"]),
        invoke_gh(["pr", "view", "--json", "number"]),
    )
    .await?;

    let repo = repo.ok_or(anyhow!("Could not determine repo"))?;

    let repo: Value = serde_json::from_str(&repo)?;
    let pr = pr
        .map(|pr| {
            serde_json::from_str(&pr)
                .ok()
                .and_then(|j: Value| j["number"].as_u64())
        })
        .flatten();

    Ok(GuessedPullRequest {
        pr,
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

async fn invoke_gh<I, S>(args: I) -> Result<Option<String>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("gh").args(args).output().await?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8(output.stdout)?;

    Ok(Some(stdout))
}
