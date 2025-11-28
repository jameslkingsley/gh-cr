use std::{sync::Arc, time::Duration};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::{FutureExt, StreamExt};
use futures_timer::Delay;
use github::{GitHub, Initialised};
use ratatui::prelude::*;
use tokio::{select, task::yield_now, time::Instant};

use crate::{actors::Actor, color_scheme::ColorScheme};

pub async fn run_app(mut app: App) -> Result<()> {
    let mut terminal = ratatui::init();

    terminal.clear()?;

    let mut event_stream = EventStream::new();

    app.init()?;

    loop {
        let delay = Delay::new(Duration::from_millis(1_000)).fuse();
        let event = event_stream.next().fuse();

        app.poll_async_widgets()?;

        terminal.draw(|frame| {
            frame.render_widget(&mut app, frame.area());
        })?;

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

        yield_now().await;
    }

    ratatui::restore();

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
    ctx: Arc<Context>,
    actors: Vec<Box<dyn Actor>>,
    dirty: bool,
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        self.render_to_buffer(area, buf);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

impl App {
    pub fn new(ctx: Context, widgets: Vec<Box<dyn Actor>>) -> Self {
        Self {
            ctx: Arc::new(ctx),
            actors: widgets,
            dirty: true,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.actors.iter().any(|w| w.dirty())
    }

    pub fn init(&mut self) -> Result<()> {
        for widget in &mut self.actors {
            widget.init(self.ctx.clone())?;
        }

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: Event) -> Result<bool> {
        for widget in &mut self.actors {
            if widget.tick(&event, self.ctx.clone())? {
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
                    KeyCode::Down if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_down();
                        self.dirty = true;
                    }
                    KeyCode::PageDown if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_page_down();
                        self.dirty = true;
                    }
                    KeyCode::Up if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_up();
                        self.dirty = true;
                    }
                    KeyCode::PageUp if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_page_up();
                        self.dirty = true;
                    }
                    KeyCode::Home if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_to_top();
                        self.dirty = true;
                    }
                    KeyCode::End if key.modifiers.is_empty() => {
                        // TODO: self.scroll_state.scroll_to_bottom();
                        self.dirty = true;
                    }
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

    pub fn poll_async_widgets(&mut self) -> Result<()> {
        for widget in &mut self.actors {
            widget.poll_async(self.ctx.clone())?;
        }
        Ok(())
    }

    pub fn render_to_buffer(&mut self, area: Rect, buf: &mut Buffer) {
        let [header, main, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let [content, side] =
            Layout::horizontal([Constraint::Length(80), Constraint::Fill(1)]).areas(main);

        Line::raw("Header line").render(header, buf);
        Line::raw("Footer line").render(footer, buf);
        Line::raw("Side").render(side, buf);

        for actor in &self.actors {
            actor.render_ref(content, buf, &mut self.ctx.clone());
        }
    }
}
