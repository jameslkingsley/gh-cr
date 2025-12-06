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
    utils::{clean, is_dirty},
    views::{convo::ConversationsView, loading::LoadingView, reviews::ReviewsView},
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

        if let Err(err) = ctx.poll_async() {
            return Err(err);
        }

        if is_dirty() || ctx.is_loading() || ctx.is_working() {
            terminal.draw(|frame| {
                state.scroll.viewport(frame.area());

                if ctx.is_loading() {
                    frame.render_stateful_widget_ref(LoadingView, frame.area(), &mut state);
                    return;
                }

                let render_time = Instant::now();

                match state.view {
                    View::ReviewsAll => frame.render_stateful_widget_ref(
                        ReviewsView { ctx: &ctx },
                        frame.area(),
                        &mut state,
                    ),
                    View::ReviewsUnresolved => frame.render_stateful_widget_ref(
                        ReviewsView { ctx: &ctx },
                        frame.area(),
                        &mut state,
                    ),
                    View::Convo => frame.render_stateful_widget_ref(
                        ConversationsView { ctx: &ctx },
                        frame.area(),
                        &mut state,
                    ),
                }

                state.render_time = render_time.elapsed();
            })?;

            clean();
        }

        if poll(Duration::from_millis(100))? {
            let event = read()?;
            if state.tick(&event, &mut ctx)? || ctx.tick(&event, &mut state)? {
                break;
            }
        }

        yield_now().await;
    }

    Ok(())
}
