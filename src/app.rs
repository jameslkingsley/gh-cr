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
use arc_swap::ArcSwap;
use crossterm::{
    cursor::MoveTo,
    event::{poll, read},
    execute,
    terminal::{Clear, ClearType, size},
};
use tempfile::NamedTempFile;
use tokio::{sync::RwLock, task::yield_now, time::Instant};

use crate::{
    Terminal,
    color_scheme::ColorScheme,
    components::{Component, Header, Quit, Resize, Scroll},
    github::{GitHub, Initialised},
    threads::Threads,
};

// TODO Use ratatui

pub async fn run_app(app: Arc<RwLock<App>>) -> Result<()> {
    {
        app.read().await.terminal.enter()?;
    }

    // TODO: Toggle view
    // let mut components: Components = match self.view {
    //     View::Threads => vec![
    //         Box::new(Quit),
    //         Box::new(Scroll),
    //         Box::new(Resize),
    //         Box::new(Header),
    //         Box::new(Threads::default()),
    //     ],
    // };

    let app_ptr = app.clone();
    let background_tick = tokio::spawn(async move {
        loop {
            let app_guard = app_ptr.read().await;

            if app_guard.do_async_tick.load(Ordering::Relaxed) {
                for component in app_guard.components.load().iter() {
                    let mut guard = component.write().await;
                    guard.tick_async(&*app_guard).await.unwrap();
                }

                app_guard.do_async_tick.swap(false, Ordering::Relaxed);
            }

            drop(app_guard);

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    'outer: loop {
        // for component in components.iter_mut() {
        //     component.tick_async(app).await?;
        // }

        if poll(Duration::from_millis(100))? {
            let event = read()?;
            let app_guard = app.read().await;

            for component in app_guard.components.load().iter() {
                let mut guard = component.write().await;
                match guard.tick(&*app_guard, &event)? {
                    Tick::Exit => break 'outer,
                    Tick::Render => {
                        app_guard.render.swap(true, Ordering::Relaxed);
                    }
                    Tick::Noop => {}
                }
            }
        }

        if app.read().await.render.load(Ordering::Relaxed) {
            let buf = {
                let mut buf = String::with_capacity(1024);
                let app_guard = app.read().await;

                for component in app_guard.components.load().iter() {
                    let guard = component.read().await;
                    guard.render(&mut buf, &*app_guard)?;
                }
                buf
            };

            let mut app_guard = app.write().await;

            app_guard.render(buf)?;
            app_guard.timer = Instant::now();
        }

        app.read().await.render.swap(false, Ordering::Relaxed);

        yield_now().await;
    }

    background_tick.abort();

    {
        app.read().await.terminal.leave()?;
    }

    Ok(())
}

#[derive(Debug)]
pub struct App {
    pub github: GitHub<Initialised>,
    pub terminal: Terminal,
    pub color_scheme: ColorScheme,
    components: Components,
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

type Components = ArcSwap<Vec<Arc<RwLock<dyn Component>>>>;

impl App {
    pub async fn new(
        github: GitHub<Initialised>,
        terminal: Terminal,
        color_scheme: ColorScheme,
    ) -> Result<Self> {
        Ok(Self {
            github,
            terminal,
            color_scheme,
            components: ArcSwap::new(Arc::new(vec![
                Arc::new(RwLock::new(Quit)),
                Arc::new(RwLock::new(Scroll)),
                Arc::new(RwLock::new(Resize)),
                Arc::new(RwLock::new(Header)),
                Arc::new(RwLock::new(Threads::default())),
            ])),
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
