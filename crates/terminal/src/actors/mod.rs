use std::sync::Arc;

use anyhow::Result;
use crossterm::event::Event;
use ratatui::widgets::StatefulWidgetRef;

use crate::app::Context;

pub mod comment;
pub mod header;
pub mod threads;

#[macro_export]
macro_rules! actor_task {
    ($slot:expr => $body:expr) => {{
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let _ = tx.send($body.await);
        });
        $slot = Some(rx);
    }};
    ($slot:expr, |$val:ident| $on_done:block) => {{
        if let Some(rx) = &mut $slot {
            if let Some(Ok($val)) = futures::FutureExt::now_or_never(rx) {
                $slot = None;
                $on_done
            }
        }
    }};
}

pub trait Actor: std::fmt::Debug + StatefulWidgetRef<State = Arc<Context>> {
    fn init(&mut self, _ctx: Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn tick(&mut self, _event: &Event, _ctx: Arc<Context>) -> Result<bool> {
        Ok(false)
    }

    fn poll_async(&mut self, _ctx: Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn dirty(&self) -> bool {
        false
    }
}
