use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use std::io::stdout;

use crate::app::App;

mod app;
mod components;
mod gh;
mod review;
mod threads;

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
            EnableMouseCapture
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
    let mut app = App::default();

    terminal.enter()?;

    app.run().await?;

    terminal.leave()?;

    Ok(())
}
