use anyhow::Result;
use clap::Parser;
use github::{GitHub, guess_pr::guess_pull_request};

use crate::{
    app::{App, Context, run_app},
    color_scheme::{ColorScheme, Rgb},
};

mod actors;
mod app;
mod color_scheme;
mod utils;

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Review GitHub pull requests from your terminal."
)]
struct Cli {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let guessed = guess_pull_request().await?;

    let github = GitHub::new()
        .owner(cli.owner.clone().unwrap_or(guessed.owner))
        .repo(cli.repo.clone().unwrap_or(guessed.repo))
        .pr(cli.pr.unwrap_or(guessed.pr))
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

    let ctx = Context::new(github, color_scheme);
    let app = App::new(ctx);

    run_app(app).await?;

    Ok(())
}
