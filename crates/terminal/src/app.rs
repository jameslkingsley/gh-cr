use std::time::{Duration, Instant};

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
        state.maybe_force_redraw(terminal)?;

        state.throbber.calc_next();

        ctx.poll_async()?;

        terminal.draw(|frame| {
            let render_time = Instant::now();

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

            state.render_time = render_time.elapsed();
        })?;

        if poll(Duration::from_millis(100))? {
            let event = read()?;
            if state.tick(&event)? || ctx.tick(&event, &mut state)? {
                break;
            }
        }

        yield_now().await;
    }

    Ok(())
}
