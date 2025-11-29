use std::{sync::Arc, time::Duration};

use anyhow::Result;
use github::{GitHub, Initialised};
use ratatui::{
    buffer::Cell,
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read},
    prelude::*,
    widgets::StatefulWidgetRef,
};
use tokio::{task::yield_now, time::Instant};

use crate::{
    actors::{Actor, threads::Threads},
    color_scheme::ColorScheme,
};

pub async fn run_app(mut app: App) -> Result<()> {
    let mut terminal = ratatui::init();

    terminal.clear()?;

    app.init()?;

    loop {
        app.poll_async_widgets()?;

        terminal.draw(|frame| {
            frame.render_widget(&mut app, frame.area());
        })?;

        if poll(Duration::from_millis(100))? {
            let event = read()?;
            if app.dispatch_event(event)? {
                break;
            }
        }

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

#[derive(Debug)]
pub enum View {
    Threads(Threads),
}

impl StatefulWidgetRef for View {
    type State = Arc<Context>;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self {
            View::Threads(threads) => threads.render_ref(area, buf, state),
        }
    }
}

impl View {
    fn init(&mut self, ctx: Arc<Context>) -> Result<()> {
        match self {
            View::Threads(threads) => threads.init(ctx),
        }
    }

    fn tick(&mut self, _event: &Event, ctx: Arc<Context>) -> Result<bool> {
        match self {
            View::Threads(threads) => threads.tick(_event, ctx),
        }
    }

    fn poll_async(&mut self, ctx: Arc<Context>) -> Result<()> {
        match self {
            View::Threads(threads) => threads.poll_async(ctx),
        }
    }

    fn dirty(&self) -> bool {
        match self {
            View::Threads(threads) => threads.dirty(),
        }
    }

    fn content_height(&self, area: Rect) -> u16 {
        match self {
            View::Threads(threads) => threads.content_height(area),
        }
    }
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
    view: View,
    dirty: bool,
    scroll_offset: usize,
    content_height: usize,
    viewport_height: usize,
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
    pub fn new(ctx: Context) -> Self {
        Self {
            ctx: Arc::new(ctx),
            view: View::Threads(Default::default()),
            dirty: true,
            scroll_offset: 0,
            content_height: 0,
            viewport_height: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.view.dirty()
    }

    pub fn init(&mut self) -> Result<()> {
        self.view.init(self.ctx.clone())?;

        Ok(())
    }

    pub fn dispatch_event(&mut self, event: Event) -> Result<bool> {
        if self.view.tick(&event, self.ctx.clone())? {
            return Ok(true);
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
                        self.scroll_down(1);
                    }
                    KeyCode::PageDown if key.modifiers.is_empty() => {
                        self.scroll_page_down();
                    }
                    KeyCode::Up if key.modifiers.is_empty() => {
                        self.scroll_up(1);
                    }
                    KeyCode::PageUp if key.modifiers.is_empty() => {
                        self.scroll_page_up();
                    }
                    KeyCode::Home if key.modifiers.is_empty() => {
                        self.scroll_to_top();
                    }
                    KeyCode::End if key.modifiers.is_empty() => {
                        self.scroll_to_bottom();
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
        self.view.poll_async(self.ctx.clone())?;
        Ok(())
    }

    fn max_scroll(&self) -> usize {
        self.content_height.saturating_sub(self.viewport_height)
    }

    fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.max_scroll();
        let new_offset = (self.scroll_offset + lines).min(max_scroll);
        if new_offset != self.scroll_offset {
            self.scroll_offset = new_offset;
            self.dirty = true;
        }
    }

    fn scroll_up(&mut self, lines: usize) {
        let new_offset = self.scroll_offset.saturating_sub(lines);
        if new_offset != self.scroll_offset {
            self.scroll_offset = new_offset;
            self.dirty = true;
        }
    }

    fn scroll_page_down(&mut self) {
        let page = self.viewport_height.max(1);
        self.scroll_down(page);
    }

    fn scroll_page_up(&mut self) {
        let page = self.viewport_height.max(1);
        self.scroll_up(page);
    }

    fn scroll_to_top(&mut self) {
        if self.scroll_offset != 0 {
            self.scroll_offset = 0;
            self.dirty = true;
        }
    }

    fn scroll_to_bottom(&mut self) {
        let max_scroll = self.max_scroll();
        if self.scroll_offset != max_scroll {
            self.scroll_offset = max_scroll;
            self.dirty = true;
        }
    }

    fn measure_content_height(&self, area: Rect) -> u16 {
        self.view.content_height(area)
    }

    fn update_scrollbar_state(&mut self, content_height: u16, viewport_height: u16) {
        self.content_height = usize::from(content_height);
        self.viewport_height = usize::from(viewport_height);

        let max_scroll = self.max_scroll();
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }
    }

    fn blit_content(&self, src: &Buffer, viewport: Rect, dest: &mut Buffer) {
        let blank = Cell::default();
        let src_height = src.area.height as usize;

        for y in 0..viewport.height {
            let dst_y = viewport.y + y;
            let src_y = self.scroll_offset.saturating_add(y as usize);

            for x in 0..viewport.width {
                let dst_x = viewport.x + x;
                let cell = if src_y < src_height {
                    src.cell((x, src_y as u16)).cloned().unwrap_or_default()
                } else {
                    blank.clone()
                };
                dest[(dst_x, dst_y)] = cell;
            }
        }
    }

    pub fn render_to_buffer(&mut self, area: Rect, buf: &mut Buffer) {
        let [header, main, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(area);

        let [content, side] =
            Layout::horizontal([Constraint::Length(82), Constraint::Fill(1)]).areas(main);

        let content_height = self.measure_content_height(content);
        self.update_scrollbar_state(content_height, content.height);

        Line::raw("Header line").render(header, buf);
        Text::raw("\nFooter line").render(footer, buf);

        let virtual_height = content_height.max(1);
        let mut content_buf = Buffer::empty(Rect::new(0, 0, content.width, virtual_height));

        self.view
            .render_ref(content_buf.area, &mut content_buf, &mut self.ctx.clone());

        self.blit_content(&content_buf, content, buf);

        Line::raw("Side").render(side, buf);
    }
}
