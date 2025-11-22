use std::{
    io::{Write, stdout},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{poll, read},
    execute,
    terminal::{Clear, ClearType, size},
};
use tokio::task::yield_now;

use crate::components::{Component, CtrlC, Scroll};

#[derive(Debug, Default)]
pub struct App {
    view: View,
    scroll_offset: usize,
}

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Threads,
    Review,
}

pub enum Tick {
    Exit,
    Render,
    Noop,
}

impl App {
    pub async fn run(&mut self) -> Result<()> {
        let mut components: Vec<Box<dyn Component>> = match self.view {
            View::Threads => vec![Box::new(CtrlC), Box::new(Scroll)],
            View::Review => vec![Box::new(CtrlC), Box::new(Scroll)],
        };

        let mut render = true;

        'outer: loop {
            if poll(Duration::from_millis(100))? {
                let event = read()?;

                for component in components.iter_mut() {
                    match component.tick(self, &event)? {
                        Tick::Exit => break 'outer,
                        Tick::Render => render = true,
                        Tick::Noop => {}
                    }
                }
            }

            if render {
                let mut buf = String::with_capacity(1024);

                for component in &components {
                    component.render(&mut buf)?;
                }

                self.render(buf)?;
            }

            render = false;

            yield_now().await;
        }

        Ok(())
    }

    pub fn scroll(&mut self, step: isize) -> Tick {
        if step == 0 {
            return Tick::Noop;
        }

        self.scroll_offset = self.scroll_offset.saturating_add_signed(step);

        Tick::Render
    }

    fn render(&mut self, buf: String) -> Result<()> {
        let mut out = stdout();

        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;

        let lines: Vec<&str> = buf.lines().collect();
        let (_, height) = size()?;
        let viewport = height as usize;

        if viewport == 0 {
            return Ok(());
        }

        let max_offset = lines.len().saturating_sub(viewport);

        if self.scroll_offset > max_offset {
            self.scroll_offset = max_offset;
        }

        for (row, line) in lines
            .iter()
            .skip(self.scroll_offset)
            .take(viewport)
            .enumerate()
        {
            let y = row as u16;
            execute!(out, MoveTo(0, y))?;
            out.write_all(line.as_bytes())?;
        }

        out.flush()?;

        Ok(())
    }
}
