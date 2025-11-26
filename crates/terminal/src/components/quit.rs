use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{
    app::{App, Tick},
    components::Component,
};

#[derive(Debug)]
pub struct Quit;

impl Component for Quit {
    fn tick(&mut self, _app: &App, event: &Event) -> Result<Tick> {
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Ok(Tick::Exit);
        }

        if let Event::Key(key) = event
            && key.code == KeyCode::Char('q')
        {
            return Ok(Tick::Exit);
        }

        Ok(Tick::Noop)
    }

    fn render(&self, _buf: &mut String, _app: &App) -> Result<()> {
        Ok(())
    }
}
