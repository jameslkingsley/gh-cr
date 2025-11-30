use anyhow::Result;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub content_height: usize,
    pub viewport_height: usize,
}

impl ScrollState {
    pub fn max_scroll(&self) -> usize {
        self.content_height.saturating_sub(self.viewport_height)
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.max_scroll();
        let new_offset = (self.offset + lines).min(max_scroll);
        if new_offset != self.offset {
            self.offset = new_offset;
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        let new_offset = self.offset.saturating_sub(lines);
        if new_offset != self.offset {
            self.offset = new_offset;
        }
    }

    pub fn scroll_page_down(&mut self) {
        let page = self.viewport_height.max(1);
        self.scroll_down(page);
    }

    pub fn scroll_page_up(&mut self) {
        let page = self.viewport_height.max(1);
        self.scroll_up(page);
    }

    pub fn scroll_to_top(&mut self) {
        if self.offset != 0 {
            self.offset = 0;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        let max_scroll = self.max_scroll();
        if self.offset != max_scroll {
            self.offset = max_scroll;
        }
    }

    pub fn update_scrollbar_state(&mut self, content_height: u16, viewport_height: u16) {
        self.content_height = usize::from(content_height);
        self.viewport_height = usize::from(viewport_height);

        let max_scroll = self.max_scroll();
        if self.offset > max_scroll {
            self.offset = max_scroll;
        }
    }

    pub fn tick(&mut self, event: &Event) -> Result<bool> {
        Ok(match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            }) => true,
            Event::Key(key) => {
                match key {
                    KeyEvent {
                        code: KeyCode::Down,
                        modifiers: KeyModifiers::ALT,
                        ..
                    } => {
                        self.scroll_to_bottom();
                    }
                    KeyEvent {
                        code: KeyCode::Up,
                        modifiers: KeyModifiers::ALT,
                        ..
                    } => {
                        self.scroll_to_top();
                    }
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        self.scroll_down(1);
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        self.scroll_up(1);
                    }
                    KeyEvent {
                        code: KeyCode::PageDown,
                        ..
                    } => {
                        self.scroll_page_down();
                    }
                    KeyEvent {
                        code: KeyCode::PageUp,
                        ..
                    } => {
                        self.scroll_page_up();
                    }
                    KeyEvent {
                        code: KeyCode::Home,
                        ..
                    } => {
                        self.scroll_to_top();
                    }
                    KeyEvent {
                        code: KeyCode::End, ..
                    } => {
                        self.scroll_to_bottom();
                    }
                    _ => {}
                }
                false
            }
            _ => false,
        })
    }
}
