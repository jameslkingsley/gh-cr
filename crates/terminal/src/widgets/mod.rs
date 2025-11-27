use anyhow::Result;
use crossterm::event::Event;

use crate::app::Context;

pub mod comment;
pub mod header;
pub mod threads;

#[macro_export]
macro_rules! widget_task {
    ($slot:expr => $body:expr) => {{
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send($body.await);
        });
        $slot = Some(rx);
    }};
    ($slot:expr, |$val:ident| $on_done:block) => {{
        if let Some(rx) = &mut $slot {
            if let Some(Ok($val)) = rx.now_or_never() {
                $slot = None;
                $on_done
            }
        }
    }};
}

pub trait Widget: std::fmt::Debug {
    fn init(&mut self, _app: &Context) -> Result<()> {
        Ok(())
    }

    fn tick(&mut self, _event: &Event, _app: &Context) -> Result<bool> {
        Ok(false)
    }

    fn poll_async(&mut self, _app: &Context) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, _buf: &mut String, _app: &Context) -> Result<()> {
        Ok(())
    }

    fn dirty(&self) -> bool {
        false
    }
}
