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
use tokio::{task::yield_now, time::Instant};

use crate::{
    color_scheme::ColorScheme,
    components::{Component, Quit, Resize, Scroll},
    github::{GitHub, Initialised},
    threads::Threads,
};

#[derive(Debug)]
pub struct App {
    pub github: GitHub<Initialised>,
    pub color_scheme: ColorScheme,
    timer: Instant,
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

type Components = Vec<Box<dyn Component>>;

impl App {
    pub async fn new(github: GitHub<Initialised>, color_scheme: ColorScheme) -> Result<Self> {
        Ok(Self {
            github,
            color_scheme,
            timer: Instant::now(),
            view: View::default(),
            scroll_offset: 0,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut components: Components = match self.view {
            View::Threads => vec![
                Box::new(Quit),
                Box::new(Scroll),
                Box::new(Resize),
                Box::new(Threads::default()),
            ],
            View::Review => vec![Box::new(Quit), Box::new(Scroll), Box::new(Resize)],
        };

        let mut render = true;

        'outer: loop {
            for component in components.iter_mut() {
                component.tick_async(self).await?;
            }

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
                    component.render(&mut buf, self)?;
                }

                self.render(buf)?;

                self.timer = Instant::now();
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

    /// Delta time since last render
    pub fn delta(&self) -> Duration {
        self.timer.elapsed()
    }

    fn render(&mut self, buf: String) -> Result<()> {
        let mut out = stdout();

        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;

        let mut lines: Vec<&str> = buf.lines().collect();

        // Add top and bottom padding
        lines.insert(0, "");
        lines.push("");

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
