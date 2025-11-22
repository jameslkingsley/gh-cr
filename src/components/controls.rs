use anyhow::Result;
use crossterm::event::Event;

use crate::app::{App, Tick};

use super::Component;

#[derive(Debug)]
pub struct Controls;

impl Component for Controls {
    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick> {
        Ok(Tick::Noop)
    }

    fn render(&self, _buf: &mut String) -> Result<()> {
        Ok(())
    }
}
