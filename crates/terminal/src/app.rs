use std::time::Duration;

use anyhow::Result;
use ratatui::{
    Terminal,
    crossterm::event::{poll, read},
    prelude::Backend,
};
use tokio::task::yield_now;

use crate::{
    context::Context,
    states::{AppState, View},
    views::{loading::LoadingView, threads::ThreadsView},
};

pub async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut state: AppState,
    mut ctx: Context,
) -> Result<()> {
    ctx.init()?;

    loop {
        state.throbber.calc_next();

        ctx.poll_async()?;

        terminal.draw(|frame| {
            if ctx.is_loading() {
                frame.render_stateful_widget_ref(LoadingView, frame.area(), &mut state);
                return;
            }

            match state.view {
                View::Threads => frame.render_stateful_widget_ref(
                    ThreadsView { ctx: &ctx },
                    frame.area(),
                    &mut state,
                ),
            }
        })?;

        if poll(Duration::from_millis(100))? {
            let event = read()?;
            if state.tick(&event)? || ctx.tick(&event)? {
                break;
            }
        }

        yield_now().await;
    }

    Ok(())
}
