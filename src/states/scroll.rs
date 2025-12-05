use std::ops::Div;

use anyhow::Result;
use ratatui::{
    crossterm::event::{Event, KeyCode, KeyEvent},
    layout::Rect,
    widgets::ScrollbarState,
};

use crate::utils::dirty;

#[derive(Debug, Default)]
pub struct ScrollState {
    pub inner: ScrollbarState,
    pub content_length: usize,
    pub viewport: Rect,
    pub vertical: usize,
}

impl ScrollState {
    pub fn content_length(&mut self, new_length: usize) {
        self.content_length = new_length;
        self.inner = self.inner.content_length(self.content_length);
        self.vertical = self.vertical.min(self.content_length);
    }

    pub fn viewport(&mut self, area: Rect) {
        self.viewport = area;
    }

    pub fn reset(&mut self) {
        self.vertical = 0;
        self.inner = self.inner.position(self.vertical);
        dirty();
    }

    pub fn scroll(&mut self, delta: isize) {
        let clamp = self
            .content_length
            .saturating_sub(self.viewport.height.div(2) as usize);

        self.vertical = self.vertical.saturating_add_signed(delta).max(0).min(clamp);

        self.inner = self.inner.position(self.vertical);

        dirty();
    }

    pub fn tick(&mut self, event: &Event) -> Result<bool> {
        match event {
            Event::Key(key) => match key {
                KeyEvent {
                    code: KeyCode::Down,
                    ..
                } => {
                    self.scroll(1);
                }
                KeyEvent {
                    code: KeyCode::Up, ..
                } => {
                    self.scroll(-1);
                }
                KeyEvent {
                    code: KeyCode::Home,
                    ..
                } => {
                    self.scroll(self.content_length as isize);
                }
                KeyEvent {
                    code: KeyCode::End, ..
                } => {
                    self.scroll(-(self.content_length as isize));
                }
                _ => {}
            },
            _ => {}
        }

        Ok(false)
    }
}
