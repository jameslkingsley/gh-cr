use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};

use crate::{
    app::{App, Tick},
    components::Component,
};

#[derive(Debug)]
pub struct CtrlC;

impl Component for CtrlC {
    fn tick(&mut self, _app: &mut App, event: &Event) -> Result<Tick> {
        if let Event::Key(key) = event
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return Ok(Tick::Exit);
        }
        Ok(Tick::Noop)
    }

    fn render(&self, _buf: &mut String) -> Result<()> {
        Ok(())
    }
}
