use std::time::Duration;

use anyhow::Result;
use ratatui::{
    Terminal,
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read},
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
    init(&mut ctx)?;

    loop {
        poll_async_widgets(&mut ctx)?;

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
            if dispatch_event(event, &mut ctx, &mut state)? {
                break;
            }
        }

        yield_now().await;
    }

    Ok(())
}

fn init(ctx: &mut Context) -> Result<()> {
    ctx.init()?;
    Ok(())
}

fn dispatch_event(event: Event, ctx: &mut Context, state: &mut AppState) -> Result<bool> {
    if ctx.tick(&event)? {
        return Ok(true);
    }
    handle_event(&event, state)
}

fn handle_event(event: &Event, state: &mut AppState) -> Result<bool> {
    Ok(match event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
        | Event::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        }) => true,
        Event::Key(key) => {
            match key.code {
                KeyCode::Down if key.modifiers.is_empty() => {
                    state.scroll.scroll_down(1);
                }
                KeyCode::PageDown if key.modifiers.is_empty() => {
                    state.scroll.scroll_page_down();
                }
                KeyCode::Up if key.modifiers.is_empty() => {
                    state.scroll.scroll_up(1);
                }
                KeyCode::PageUp if key.modifiers.is_empty() => {
                    state.scroll.scroll_page_up();
                }
                KeyCode::Home if key.modifiers.is_empty() => {
                    state.scroll.scroll_to_top();
                }
                KeyCode::End if key.modifiers.is_empty() => {
                    state.scroll.scroll_to_bottom();
                }
                _ => {}
            };
            false
        }
        _ => false,
    })
}

fn poll_async_widgets(ctx: &mut Context) -> Result<()> {
    ctx.poll_async()?;
    Ok(())
}
