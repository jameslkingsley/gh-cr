use std::isize;

use anyhow::Result;
use crossterm::{
    event::{Event, KeyCode, MouseEventKind},
    terminal::size,
};

use crate::{
    app::{App, Tick},
    components::Component,
};

#[derive(Debug)]
pub struct Scroll;

impl Component for Scroll {
    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick> {
        Ok(match event {
            Event::Key(key) => match key.code {
                KeyCode::Down if key.modifiers.is_empty() => app.scroll(1),
                KeyCode::PageDown if key.modifiers.is_empty() => app.scroll(page_step()),
                KeyCode::Up if key.modifiers.is_empty() => app.scroll(-1),
                KeyCode::PageUp if key.modifiers.is_empty() => app.scroll(-page_step()),
                KeyCode::Home if key.modifiers.is_empty() => app.scroll(isize::MIN),
                KeyCode::End if key.modifiers.is_empty() => app.scroll(isize::MAX),
                _ => Tick::Noop,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => app.scroll(-3),
                MouseEventKind::ScrollDown => app.scroll(3),
                _ => Tick::Noop,
            },
            _ => Tick::Noop,
        })
    }

    fn render(&self, _buf: &mut String, _app: &App) -> Result<()> {
        Ok(())
    }
}

fn page_step() -> isize {
    match size() {
        Ok((_, height)) => height.saturating_sub(1) as isize,
        Err(_) => 0,
    }
}
