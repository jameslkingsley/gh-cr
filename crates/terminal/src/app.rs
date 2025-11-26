use std::{
    env, fs,
    io::{Write, stdout},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow};
use crossterm::{
    cursor::MoveTo,
    event::{poll, read},
    execute,
    terminal::{Clear, ClearType, size},
};
use github::{GitHub, Initialised};
use tempfile::NamedTempFile;
use tokio::{sync::RwLock, task::yield_now, time::Instant};

use crate::{
    Terminal,
    widgets::{Example, Widget},
    // color_scheme::ColorScheme,
    // components::{Component, Header, Quit, Resize, Scroll},
    // github::{GitHub, Initialised},
    // threads::Threads,
};

// TODO Use ratatui
//      Compare by commits option

pub async fn run_app(mut app: App) -> Result<()> {
    app.terminal.enter()?;

    // let background_tick = tokio::spawn(async move {
    //     loop {
    //         let app_guard = app_ptr.read().await;

    //         if app_guard.do_async_tick.load(Ordering::Relaxed) {
    //             for component in app_guard.components.load().iter() {
    //                 let mut guard = component.write().await;
    //                 guard.tick_async(&*app_guard).await.unwrap();
    //             }

    //             app_guard.do_async_tick.swap(false, Ordering::Relaxed);
    //         }

    //         drop(app_guard);

    //         tokio::time::sleep(Duration::from_millis(100)).await;
    //     }
    // });

    'main: loop {
        if poll(Duration::from_millis(100))? {
            let event = read()?;

            for widget in &mut app.widgets {
                if widget.tick(&event)? {
                    break 'main;
                }
            }
        }

        let buf = {
            let mut buf = String::with_capacity(1024);
            for widget in &app.widgets {
                widget.render(&mut buf)?;
            }
            buf
        };

        app.render(buf)?;
        app.timer = Instant::now();

        yield_now().await;
    }

    app.terminal.leave()?;

    Ok(())
}

#[derive(Debug)]
pub struct App {
    pub github: GitHub<Initialised>,
    pub terminal: Terminal,
    pub widgets: Vec<Box<dyn Widget>>,
    timer: Instant,
    view: View,
    scroll_offset: AtomicUsize,
    render: AtomicBool,
    do_async_tick: AtomicBool,
}

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Threads,
}

pub enum Tick {
    Exit,
    Render,
    Noop,
}

impl App {
    pub async fn new(github: GitHub<Initialised>, terminal: Terminal) -> Result<Self> {
        Ok(Self {
            github,
            terminal,
            widgets: vec![Box::new(Example::default())],
            timer: Instant::now(),
            view: View::default(),
            scroll_offset: AtomicUsize::new(0),
            render: AtomicBool::new(true),
            do_async_tick: AtomicBool::new(true),
        })
    }

    pub fn suspend_for_editor(&self, initial_contents: String) -> Result<String> {
        self.terminal.leave()?;

        let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

        let mut tempfile = NamedTempFile::new()?;
        tempfile.write_all(initial_contents.as_bytes())?;
        tempfile.flush()?;

        let status = Command::new(&editor).arg(tempfile.path()).status()?;
        if !status.success() {
            return Err(anyhow!("editor exited with {}", status));
        }

        let body = fs::read_to_string(tempfile.path())?;

        self.terminal.enter()?;

        Ok(body)
    }

    pub fn scroll(&self, step: isize) -> Tick {
        if step == 0 {
            return Tick::Noop;
        }

        self.scroll_offset
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |offset| {
                Some(offset.saturating_add_signed(step))
            })
            .unwrap();

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

        let mut scroll_offset = self.scroll_offset.load(Ordering::Relaxed);
        let max_offset = lines.len().saturating_sub(viewport);

        if scroll_offset > max_offset {
            scroll_offset = max_offset;
        }

        for (row, line) in lines.iter().skip(scroll_offset).take(viewport).enumerate() {
            let y = row as u16;
            execute!(out, MoveTo(0, y))?;
            // TODO Config option for indent
            out.write_all("  ".as_bytes())?;
            out.write_all(line.as_bytes())?;
        }

        out.flush()?;

        Ok(())
    }
}
