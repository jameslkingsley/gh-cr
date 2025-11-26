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
    event::{Event, EventStream, poll, read},
    execute,
    terminal::{Clear, ClearType, size},
};
use futures::{FutureExt, StreamExt};
use futures_timer::Delay;
use github::{GitHub, Initialised};
use tempfile::NamedTempFile;
use tokio::{
    select,
    sync::{RwLock, mpsc::unbounded_channel},
    task::yield_now,
    time::Instant,
};

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

#[derive(Debug)]
pub struct Context {
    //
}

pub async fn run_app(mut app: App) -> Result<()> {
    app.terminal.enter()?;

    // let (async_tx, async_rx) = unbounded_channel();

    // let _worker = tokio::spawn(async move {});

    let mut event_stream = EventStream::new();

    'main: loop {
        let delay = Delay::new(Duration::from_millis(1_000)).fuse();
        let event = event_stream.next().fuse();

        select! {
            _ = delay => {},
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if app.dispatch_event(event)? {
                            break 'main;
                        }
                    }
                    Some(Err(e)) => println!("Error: {e:?}\r"),
                    None => break,
                }
            }
        };

        if app.is_dirty() {
            app.render()?;
        }

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
    render_count: usize,
}

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Threads,
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
            render_count: 0,
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.widgets.iter().any(|w| w.dirty())
    }

    pub fn dispatch_event(&mut self, event: Event) -> Result<bool> {
        for widget in &mut self.widgets {
            if widget.tick(&event)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    // pub fn suspend_for_editor(&self, initial_contents: String) -> Result<String> {
    //     self.terminal.leave()?;

    //     let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());

    //     let mut tempfile = NamedTempFile::new()?;
    //     tempfile.write_all(initial_contents.as_bytes())?;
    //     tempfile.flush()?;

    //     let status = Command::new(&editor).arg(tempfile.path()).status()?;
    //     if !status.success() {
    //         return Err(anyhow!("editor exited with {}", status));
    //     }

    //     let body = fs::read_to_string(tempfile.path())?;

    //     self.terminal.enter()?;

    //     Ok(body)
    // }

    // pub fn scroll(&self, step: isize) -> Tick {
    //     if step == 0 {
    //         return Tick::Noop;
    //     }

    //     self.scroll_offset
    //         .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |offset| {
    //             Some(offset.saturating_add_signed(step))
    //         })
    //         .unwrap();

    //     Tick::Render
    // }

    // /// Delta time since last render
    // pub fn delta(&self) -> Duration {
    //     self.timer.elapsed()
    // }

    pub fn render(&mut self) -> Result<()> {
        self.render_count += 1;

        let mut buf = String::with_capacity(1024);

        for widget in &mut self.widgets {
            widget.render(&mut buf)?;
        }

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

        writeln!(out, "\nRenders: {}", self.render_count)?;

        out.flush()?;

        Ok(())
    }
}
