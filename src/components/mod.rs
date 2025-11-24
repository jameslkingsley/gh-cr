mod comment;
mod header;
mod quit;
mod resize;
mod scroll;

use std::{fmt::Debug, pin::Pin};

use anyhow::Result;
use crossterm::event::Event;

pub use comment::*;
pub use header::*;
pub use quit::*;
pub use resize::*;
pub use scroll::*;

use crate::app::{App, Tick};

pub trait Render {
    type Context;

    fn render(&self, buf: &mut String, app: &App, ctx: Self::Context) -> Result<()>;
}

pub trait Component: Debug + Send + Sync {
    fn tick_async<'a>(
        &'a mut self,
        _app: &'a App,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn tick(&mut self, app: &App, event: &Event) -> Result<Tick>;

    fn render(&self, buf: &mut String, app: &App) -> Result<()>;
}
