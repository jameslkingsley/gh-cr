use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use std::{error::Error, io};
use textwrap::wrap;

#[derive(Clone, Copy)]
enum DiffKind {
    Add,
    Remove,
    Context,
}

struct DiffLine<'a> {
    kind: DiffKind,
    text: &'a str,
}

struct Comment<'a> {
    author: &'a str,
    age: &'a str,
    body: &'a str, // plain text; we'll wrap it with textwrap
}

struct Thread<'a> {
    idx: usize,
    total_threads: usize,
    status: &'a str,
    pr_title: &'a str,
    path: &'a str,
    line: u32,
    unresolved: bool,
    age: &'a str,
    diff: Vec<DiffLine<'a>>,
    comments: Vec<Comment<'a>>,
}

/// Widget that renders a single thread as a big block of styled text,
/// and lets Paragraph handle scrolling.
struct ThreadWidget<'a> {
    thread: &'a Thread<'a>,
    scroll: u16,
}

impl<'a> ThreadWidget<'a> {
    fn build_text(&self) -> Text<'a> {
        let mut lines: Vec<Line<'a>> = Vec::new();

        // ── header line 1: "Thread 1/5 (unresolved)   PR #2"
        let status_color = if self.thread.unresolved {
            Color::Red
        } else {
            Color::Green
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("Thread {}/{} ", self.thread.idx, self.thread.total_threads),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if self.thread.unresolved {
                    "(unresolved) "
                } else {
                    "(resolved) "
                },
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(self.thread.pr_title, Style::default().fg(Color::Yellow)),
        ]));

        // header line 2: "src/components/mod.rs:33   unresolved   4 days ago"
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}:{}", self.thread.path, self.thread.line),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("    "),
            Span::styled(self.thread.status, Style::default().fg(status_color)),
            Span::raw("    "),
            Span::styled(self.thread.age, Style::default().fg(Color::DarkGray)),
        ]));

        lines.push(Line::default()); // blank

        // diff block
        for d in &self.thread.diff {
            let (prefix, color) = match d.kind {
                DiffKind::Add => ("+ ", Color::Green),
                DiffKind::Remove => ("- ", Color::Red),
                DiffKind::Context => ("  ", Color::DarkGray),
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::styled(d.text, Style::default().fg(color)),
            ]));
        }

        lines.push(Line::default()); // blank before comments

        // comments
        for c in &self.thread.comments {
            // author line
            lines.push(Line::from(vec![
                Span::styled(
                    c.author,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(c.age, Style::default().fg(Color::DarkGray)),
            ]));

            lines.push(Line::default());

            // body lines, wrapped to 80 cols with textwrap
            for wrapped in wrap(c.body, 80) {
                lines.push(Line::from(Span::raw(wrapped.into_owned())));
            }

            lines.push(Line::default()); // space between comments
        }

        Text::from(lines)
    }
}

impl<'a> Widget for ThreadWidget<'a> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let text = self.build_text();

        Paragraph::new(text)
            .block(Block::default().borders(Borders::NONE))
            .scroll((self.scroll, 0))
            .render(area, buf);
    }
}

struct App<'a> {
    thread: Thread<'a>,
    scroll: u16,
}

impl<'a> App<'a> {
    fn new(thread: Thread<'a>) -> Self {
        Self { thread, scroll: 0 }
    }

    fn on_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn on_down(&mut self) {
        // You can clamp to max scroll by precomputing text.height()
        self.scroll = self.scroll.saturating_add(1);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // sample data -------------------------------------------------------------
    let diff = vec![
        DiffLine {
            kind: DiffKind::Add,
            text: "mod controls;",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "mod ctrl_c;",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "mod scroll;",
        },
        DiffLine {
            kind: DiffKind::Context,
            text: "",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "pub trait Component: Debug {",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "    fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick>;",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "    fn render(&self, buf: &mut String) -> Result<()>;",
        },
        DiffLine {
            kind: DiffKind::Add,
            text: "}",
        },
    ];

    let comment_body = "Ne refugitque exsistit *amas se*. Molior conlapsosque haec et ego, iussum \
                        silva, nostras ut nefasque capit dixerat pollice os vitibus aduncis bis.\n\n\
                        – Est valet non Argos exsultat conditus sacra\n\
                        – A Circes iuvenum in redeat sistitur se\n\
                        – Adest Iovi non sed protinus parte aperite\n\
                        – Nec virorum pudore\n\
                        – Rapidum malus";

    let comments = vec![
        Comment {
            author: "jameslkingsley",
            age: "4 days ago",
            body: comment_body,
        },
        Comment {
            author: "jameslkingsley",
            age: "4 days ago",
            body: "Suggestion:\n\npub trait Component: Debug {\n    \
                   fn tick(&mut self, app: &mut App, event: &Event) -> Result<Tick>;\n    \
                   fn render(&self, buf: &mut String) -> Result<()>;\n}",
        },
    ];

    let thread = Thread {
        idx: 1,
        total_threads: 5,
        status: "unresolved",
        pr_title: "PR #2",
        path: "src/components/mod.rs",
        line: 33,
        unresolved: true,
        age: "4 days ago",
        diff,
        comments,
    };

    let mut app = App::new(thread);

    // terminal setup ----------------------------------------------------------
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.on_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.on_down(),
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // main + status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(size);

    let main = chunks[0];
    let status = chunks[1];

    // thread view
    let widget = ThreadWidget {
        thread: &app.thread,
        scroll: app.scroll,
    };
    f.render_widget(widget, main);

    // bottom status line
    let status_text = Line::from("↑/k ↓/j scroll    q quit");
    let status_paragraph = Paragraph::new(status_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default());
    f.render_widget(status_paragraph, status);
}
