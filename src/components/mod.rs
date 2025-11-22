mod controls;
mod ctrl_c;
mod scroll;

use std::fmt::Debug;

use anyhow::Result;
use crossterm::event::Event;

pub use controls::*;
pub use ctrl_c::*;
pub use scroll::*;

use crate::app::{App, Tick};

pub trait Component: Debug {
    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick>;

    fn render(&self, buf: &mut String) -> Result<()>;
}
