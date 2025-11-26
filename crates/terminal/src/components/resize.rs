use anyhow::Result;
use crossterm::event::Event;

use crate::{
    app::{App, Tick},
    components::Component,
};

#[derive(Debug)]
pub struct Resize;

impl Component for Resize {
    fn tick(&mut self, _app: &App, event: &Event) -> Result<Tick> {
        if let Event::Resize(_, _) = event {
            return Ok(Tick::Render);
        }

        Ok(Tick::Noop)
    }

    fn render(&self, _buf: &mut String, _app: &App) -> Result<()> {
        Ok(())
    }
}
