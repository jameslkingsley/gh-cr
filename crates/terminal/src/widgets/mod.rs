#![allow(dead_code, unused_variables)]

use anyhow::Result;
use crossterm::event::{Event, KeyCode};
use std::fmt::Write;

pub trait Widget: std::fmt::Debug {
    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn tick(&mut self, event: &Event) -> Result<bool>;

    fn render(&mut self, buf: &mut String) -> Result<()> {
        Ok(())
    }

    fn dirty(&self) -> bool {
        false
    }
}

#[derive(Debug, Default)]
pub struct Example {
    pub lines: Vec<String>,
    pub dirty: bool,
}

impl Example {
    pub fn add_line(&mut self) {
        self.lines.push("Here is a line".to_string());
        self.dirty = true;
    }

    pub fn remove_line(&mut self) {
        self.lines.pop();
        self.dirty = true;
    }
}

impl Widget for Example {
    fn tick(&mut self, event: &Event) -> Result<bool> {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('a') => self.add_line(),
                KeyCode::Char('r') => self.remove_line(),
                KeyCode::Char('q') => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
    }

    fn render(&mut self, buf: &mut String) -> Result<()> {
        for line in &self.lines {
            writeln!(buf, "{}", line)?;
        }

        self.dirty = false;

        Ok(())
    }

    fn dirty(&self) -> bool {
        self.dirty
    }
}
