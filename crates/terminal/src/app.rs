use std::{
    io::{Write, stdout},
    time::Duration,
};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers, MouseEventKind},
    execute,
    terminal::{Clear, ClearType, size},
};
use futures::{FutureExt, StreamExt};
use futures_timer::Delay;
use github::{GitHub, Initialised};
use tokio::{select, task::yield_now, time::Instant};

use crate::{
    color_scheme::ColorScheme,
    utils::{enter_terminal, leave_terminal},
    widgets::Widget,
};

pub async fn run_app(mut app: App) -> Result<()> {
    enter_terminal()?;

    let mut event_stream = EventStream::new();

    app.init()?;

    loop {
        let delay = Delay::new(Duration::from_millis(1_000)).fuse();
        let event = event_stream.next().fuse();

        select! {
            _ = delay => {},
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        if app.dispatch_event(event)? {
                            break;
                        }
                    }
                    Some(Err(e)) => panic!("Error: {e:?}\r"),
                    None => break,
                }
            }
        };

        app.poll_async_widgets()?;

        if app.is_dirty() {
            app.render()?;
        }

        yield_now().await;
    }

    leave_terminal()?;

    Ok(())
}

#[derive(Debug)]
pub struct Context {
    pub github: GitHub<Initialised>,
    pub color_scheme: ColorScheme,
    timer: Instant,
}

#[derive(Debug, Default)]
pub enum View {
    #[default]
    Threads,
}

impl Context {
    pub fn new(github: GitHub<Initialised>, color_scheme: ColorScheme) -> Self {
        Self {
            github,
            color_scheme,
            timer: Instant::now(),
        }
    }

    pub fn delta(&self) -> Duration {
        self.timer.elapsed()
    }
}

#[derive(Debug)]
pub struct App {
    ctx: Context,
    widgets: Vec<Box<dyn Widget>>,
    scroll_offset: usize,
    dirty: bool,
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = leave_terminal();
    }
}

impl App {
    pub fn new(ctx: Context, widgets: Vec<Box<dyn Widget>>) -> Self {
        Self {
            ctx,
            widgets,
            scroll_offset: 0,
            dirty: true,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.widgets.iter().any(|w| w.dirty())
    }

    pub fn init(&mut self) -> Result<()> {
        let app = &self.ctx;
        for widget in &mut self.widgets {
            widget.init(app)?;
        }

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: Event) -> Result<bool> {
        let app = &self.ctx;
        for widget in &mut self.widgets {
            if widget.tick(&event, app)? {
                return Ok(true);
            }
        }
        self.handle_event(&event)
    }

    fn handle_event(&mut self, event: &Event) -> Result<bool> {
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
                match key.code {
                    KeyCode::Down if key.modifiers.is_empty() => self.scroll(1),
                    KeyCode::PageDown if key.modifiers.is_empty() => self.scroll(page_step()),
                    KeyCode::Up if key.modifiers.is_empty() => self.scroll(-1),
                    KeyCode::PageUp if key.modifiers.is_empty() => self.scroll(-page_step()),
                    KeyCode::Home if key.modifiers.is_empty() => self.scroll(isize::MIN),
                    KeyCode::End if key.modifiers.is_empty() => self.scroll(isize::MAX),
                    _ => {}
                };
                false
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp => self.scroll(-3),
                    MouseEventKind::ScrollDown => self.scroll(3),
                    _ => {}
                };
                false
            }
            Event::Resize(_, _) => {
                self.dirty = true;
                false
            }
            _ => false,
        })
    }

    fn scroll(&mut self, step: isize) {
        self.scroll_offset = self.scroll_offset.saturating_add_signed(step);
        self.dirty = true;
    }

    pub fn poll_async_widgets(&mut self) -> Result<()> {
        let app = &self.ctx;
        for widget in &mut self.widgets {
            widget.poll_async(app)?;
        }
        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        self.dirty = false;

        let app = &self.ctx;
        let mut buf = String::new();

        for widget in &mut self.widgets {
            widget.render(&mut buf, app)?;
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

        let mut scroll_offset = self.scroll_offset;
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

fn page_step() -> isize {
    match size() {
        Ok((_, height)) => height.saturating_sub(1) as isize,
        Err(_) => 0,
    }
}
