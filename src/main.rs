use anyhow::{Result, anyhow};
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::DisableMouseCapture,
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use serde_json::Value;
use std::{ffi::OsStr, io::stdout, sync::Arc};
use tokio::{process::Command, sync::RwLock};

use crate::{
    app::{App, run_app},
    color_scheme::{ColorScheme, Rgb},
    github::GitHub,
};

mod app;
mod color_scheme;
mod components;
mod github;
mod review;
mod threads;
mod utils;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Review GitHub pull requests from your terminal."
)]
struct Terminal {
    /// Override the inferred PR number
    #[arg(short, long)]
    pr: Option<u64>,

    /// Override the inferred owner
    #[arg(short, long)]
    owner: Option<String>,

    /// Override the inferred repository
    #[arg(short, long)]
    repo: Option<String>,
}

impl Terminal {
    fn enter(&self) -> Result<()> {
        let mut out = stdout();

        enable_raw_mode()?;

        execute!(
            out,
            EnterAlternateScreen,
            Clear(ClearType::All),
            Hide,
            DisableMouseCapture
        )?;

        Ok(())
    }

    fn leave(&self) -> Result<()> {
        let mut out = stdout();

        disable_raw_mode().ok();

        execute!(out, DisableMouseCapture, Show, LeaveAlternateScreen)?;

        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let terminal = Terminal::parse();

    let guessed = guess_pull_request().await?;

    let github = GitHub::new()
        .owner(terminal.owner.clone().unwrap_or(guessed.owner))
        .repo(terminal.repo.clone().unwrap_or(guessed.repo))
        .pr(terminal.pr.unwrap_or(guessed.pr))
        .build()
        .await?;

    let color_scheme = ColorScheme {
        pr_title: Rgb(208, 208, 208),
        muted: Rgb(51, 53, 68),
        author: Rgb(18, 207, 192),
        comment_body: Rgb(208, 208, 208),
        border: Rgb(51, 53, 68),
        border_active: Rgb(18, 207, 192),
        diff_added: Rgb(218, 255, 166),
        diff_removed: Rgb(246, 144, 144),
        diff_unchanged: Rgb(51, 53, 68),
    };

    let app = Arc::new(RwLock::new(App::new(github, terminal, color_scheme).await?));

    run_app(app).await?;

    Ok(())
}

struct GuessedPullRequest {
    pr: u64,
    owner: String,
    repo: String,
}

async fn guess_pull_request() -> Result<GuessedPullRequest> {
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
