use std::sync::OnceLock;

use anyhow::Result;
use clap::Parser;

use crate::{app::run_app, context::Context, github::GitHub, states::AppState};

mod app;
mod context;
mod github;
mod prelude;
mod states;
mod utils;
mod views;
mod widgets;

pub static GH: OnceLock<GitHub> = OnceLock::new();

#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version,
    about = "Review GitHub pull requests from your terminal."
)]
pub struct Cli {
    /// Override the inferred PR number
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

    let _ = GH.set(GitHub::new());

    let state = AppState::default();
    let ctx = Context::new(cli);

    let mut terminal = ratatui::init();

    let result = run_app(&mut terminal, state, ctx).await;

    ratatui::restore();

    result
}
