#![allow(dead_code)]

use std::{
    collections::{HashSet, VecDeque},
    env,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs,
    io::{Write, stdout},
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use chrono_humanize::HumanTime;
use clap::Parser;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    style::{Color, Stylize},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode, size,
    },
};
use serde::Deserialize;
use tempfile::NamedTempFile;
use textwrap::{Options as WrapOptions, wrap};
use tokio::process::Command as TokioCommand;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let mode = if args.dump { Mode::Dump } else { Mode::Tui };
    let gh = GhCli::new();
    let repo = match gh.current_repo().await {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("Unable to determine repository: {err}");
            return Ok(());
        }
    };

    let pr_number = if let Some(num) = args.pr_number {
        num
    } else {
        match gh.current_pr_number().await {
            Ok(num) => num,
            Err(err) => {
                eprintln!("No pull request associated with the current branch: {err}");
                return Ok(());
            }
        }
    };

    let skip_store = SkipStore::load().context("failed to load skip list")?;
    let threads = gh
        .fetch_threads(&repo, pr_number)
        .await
        .context("failed to fetch review threads")?;

    let mut app = App::new(gh, repo, pr_number, skip_store, threads);
    match mode {
        Mode::Tui => app.run().await?,
        Mode::Dump => app.dump_once()?,
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Review GitHub PR threads from your terminal."
)]
struct Cli {
    /// Print the current thread content once and exit (no TUI)
    #[arg(long)]
    dump: bool,

    /// Override the inferred PR number
    #[arg(value_parser = clap::value_parser!(u64))]
    pr_number: Option<u64>,
}

#[derive(Clone, Copy)]
enum Mode {
    Tui,
    Dump,
}

const COMMENT_WRAP: usize = 80;

struct App {
    gh: GhCli,
    repo: Repo,
    pr_number: u64,
    skip_store: SkipStore,
    active_threads: Vec<Thread>,
    unresolved_threads: Vec<Thread>,
    skipped_threads: Vec<Thread>,
    current_unresolved: usize,
    current_active: usize,
    current_skipped: usize,
    view: ThreadView,
    status_line: Option<String>,
    scroll_offset: usize,
    queued_replies: VecDeque<QueuedReply>,
}

#[derive(Clone, Copy)]
enum ThreadView {
    Unresolved,
    Active,
    Skipped,
}

impl ThreadView {
    fn next(self) -> Self {
        match self {
            ThreadView::Unresolved => ThreadView::Active,
            ThreadView::Active => ThreadView::Skipped,
            ThreadView::Skipped => ThreadView::Unresolved,
        }
    }

    fn skip_action_label(self) -> &'static str {
        match self {
            ThreadView::Unresolved | ThreadView::Active => "s skip",
            ThreadView::Skipped => "s unskip",
        }
    }

    fn name(self) -> &'static str {
        match self {
            ThreadView::Unresolved => "unresolved",
            ThreadView::Active => "unskipped",
            ThreadView::Skipped => "skipped",
        }
    }
}

impl App {
    fn new(
        gh: GhCli,
        repo: Repo,
        pr_number: u64,
        skip_store: SkipStore,
        threads: Vec<Thread>,
    ) -> Self {
        let (active_threads, skipped_threads) = Self::partition_threads(&skip_store, threads);
        let unresolved_threads = Self::build_unresolved(&active_threads);
        Self {
            gh,
            repo,
            pr_number,
            skip_store,
            active_threads,
            unresolved_threads,
            skipped_threads,
            current_unresolved: 0,
            current_active: 0,
            current_skipped: 0,
            view: ThreadView::Unresolved,
            status_line: None,
            scroll_offset: 0,
            queued_replies: VecDeque::new(),
        }
    }

    fn partition_threads(
        skip_store: &SkipStore,
        threads: Vec<Thread>,
    ) -> (Vec<Thread>, Vec<Thread>) {
        let mut active = Vec::new();
        let mut skipped = Vec::new();
        for thread in threads {
            if skip_store.contains(&thread.id) {
                skipped.push(thread);
            } else {
                active.push(thread);
            }
        }
        Self::sort_threads(&mut active);
        Self::sort_threads(&mut skipped);
        (active, skipped)
    }

    fn sort_threads(list: &mut Vec<Thread>) {
        list.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    }

    fn build_unresolved(active: &[Thread]) -> Vec<Thread> {
        active
            .iter()
            .filter(|thread| !thread.is_resolved)
            .cloned()
            .collect()
    }

    fn threads_for_view(&self, view: ThreadView) -> &Vec<Thread> {
        match view {
            ThreadView::Unresolved => &self.unresolved_threads,
            ThreadView::Active => &self.active_threads,
            ThreadView::Skipped => &self.skipped_threads,
        }
    }

    fn threads_for_view_mut(&mut self, view: ThreadView) -> &mut Vec<Thread> {
        match view {
            ThreadView::Unresolved => &mut self.unresolved_threads,
            ThreadView::Active => &mut self.active_threads,
            ThreadView::Skipped => &mut self.skipped_threads,
        }
    }

    fn index_for_view(&self, view: ThreadView) -> usize {
        match view {
            ThreadView::Unresolved => self.current_unresolved,
            ThreadView::Active => self.current_active,
            ThreadView::Skipped => self.current_skipped,
        }
    }

    fn index_for_view_mut(&mut self, view: ThreadView) -> &mut usize {
        match view {
            ThreadView::Unresolved => &mut self.current_unresolved,
            ThreadView::Active => &mut self.current_active,
            ThreadView::Skipped => &mut self.current_skipped,
        }
    }

    fn current_threads(&self) -> &Vec<Thread> {
        self.threads_for_view(self.view)
    }

    fn current_threads_mut(&mut self) -> &mut Vec<Thread> {
        self.threads_for_view_mut(self.view)
    }

    fn current_index(&self) -> usize {
        self.index_for_view(self.view)
    }

    fn current_index_mut(&mut self) -> &mut usize {
        self.index_for_view_mut(self.view)
    }

    fn current_thread(&self) -> Option<&Thread> {
        self.current_threads().get(self.current_index())
    }

    fn clamp_index_for_view(&mut self, view: ThreadView) {
        let len = self.threads_for_view(view).len();
        let idx = self.index_for_view_mut(view);
        if len == 0 {
            *idx = 0;
        } else if *idx >= len {
            *idx = len - 1;
        }
    }

    fn clamp_current_index(&mut self) {
        self.clamp_index_for_view(self.view);
    }

    fn advance_view(&mut self) {
        self.view = self.view.next();
        self.clamp_current_index();
        self.reset_scroll();
        self.status_line = Some(format!("Showing {} threads.", self.view.name()));
    }

    fn rebuild_unresolved(&mut self, preferred: Option<String>) {
        let fallback = self
            .unresolved_threads
            .get(self.current_unresolved)
            .map(|t| t.id.clone());
        let target = preferred.or(fallback);
        self.unresolved_threads = Self::build_unresolved(&self.active_threads);
        if let Some(id) = target {
            if let Some(pos) = self.unresolved_threads.iter().position(|t| t.id == id) {
                self.current_unresolved = pos;
                return;
            }
        }
        if self.unresolved_threads.is_empty() {
            self.current_unresolved = 0;
        } else if self.current_unresolved >= self.unresolved_threads.len() {
            self.current_unresolved = self.unresolved_threads.len() - 1;
        }
    }

    async fn run(&mut self) -> Result<()> {
        let mut terminal = TerminalSession::enter()?;
        let mut needs_render = true;
        loop {
            if needs_render {
                self.render()?;
                needs_render = false;
            }
            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    match key.code {
                        KeyCode::Char('q') if key.modifiers.is_empty() => break,
                        KeyCode::Char('j') if key.modifiers.is_empty() => {
                            self.next_thread();
                            needs_render = true;
                        }
                        KeyCode::Char('k') if key.modifiers.is_empty() => {
                            self.prev_thread();
                            needs_render = true;
                        }
                        KeyCode::Right if key.modifiers.is_empty() => {
                            self.next_thread();
                            needs_render = true;
                        }
                        KeyCode::Left if key.modifiers.is_empty() => {
                            self.prev_thread();
                            needs_render = true;
                        }
                        KeyCode::Down if key.modifiers.is_empty() => {
                            self.scroll_down(1);
                            needs_render = true;
                        }
                        KeyCode::Up if key.modifiers.is_empty() => {
                            self.scroll_up(1);
                            needs_render = true;
                        }
                        KeyCode::Tab if key.modifiers.is_empty() => {
                            self.advance_view();
                            needs_render = true;
                        }
                        KeyCode::Char('s') if key.modifiers.is_empty() => {
                            let action = match self.view {
                                ThreadView::Unresolved | ThreadView::Active => "skip",
                                ThreadView::Skipped => "unskip",
                            };
                            if let Err(err) = self.skip_current() {
                                self.status_line =
                                    Some(format!("Failed to {action} thread: {err}"));
                            }
                            needs_render = true;
                        }
                        KeyCode::Char('r') if key.modifiers.is_empty() => {
                            if let Err(err) = self.reply_to_current(&mut terminal).await {
                                self.status_line = Some(format!("Failed to post reply: {err}"));
                            }
                            needs_render = true;
                        }
                        KeyCode::Char('p') if key.modifiers.is_empty() => {
                            if let Err(err) = self.publish_queue().await {
                                self.status_line =
                                    Some(format!("Failed to publish replies: {err}"));
                            }
                            needs_render = true;
                        }
                        _ => {}
                    }
                }
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::ScrollUp => {
                        self.scroll_up(3);
                        needs_render = true;
                    }
                    MouseEventKind::ScrollDown => {
                        self.scroll_down(3);
                        needs_render = true;
                    }
                    _ => {}
                },
                Event::Resize(_, _) => needs_render = true,
                _ => {}
            }
        }
        Ok(())
    }

    fn render(&mut self) -> Result<()> {
        let mut out = stdout();
        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;
        let view = self.render_view();
        let lines: Vec<&str> = view.lines().collect();
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

    fn render_view(&self) -> String {
        let mut buf = String::new();
        self.write_view(&mut buf)
            .expect("writing to a string should not fail");
        buf
    }

    fn write_view(&self, buf: &mut String) -> std::fmt::Result {
        let now = Utc::now();
        let threads = self.current_threads();
        if threads.is_empty() {
            writeln!(
                buf,
                "{}",
                format!(
                    "PR #{} – No {} threads to display.",
                    self.pr_number,
                    self.view.name()
                )
                .bold()
            )?;
            let hint = match self.view {
                ThreadView::Unresolved => {
                    "Press tab to view all unskipped threads or skipped threads. Press q to exit."
                }
                ThreadView::Active => "Press tab to view skipped threads or q to exit.",
                ThreadView::Skipped => "Press tab to return to unresolved threads or q to exit.",
            };
            writeln!(buf, "{}", hint.with(Color::DarkGrey))?;
        } else {
            let current_index = self.current_index();
            let thread = &threads[current_index];
            let muted = Color::Rgb {
                r: 110,
                g: 110,
                b: 110,
            };
            let accent = Color::Rgb {
                r: 180,
                g: 180,
                b: 180,
            };
            writeln!(
                buf,
                "{}   {}",
                format!(
                    "Thread {}/{} ({})",
                    current_index + 1,
                    threads.len(),
                    self.view.name()
                )
                .with(muted)
                .bold(),
                format!("PR #{}", self.pr_number).with(muted)
            )?;
            writeln!(
                buf,
                "{}  {}  {}",
                thread.path.as_str().with(accent),
                if thread.is_resolved {
                    "resolved".with(Color::DarkGreen)
                } else {
                    "unresolved".with(Color::DarkYellow)
                },
                humanize_relative(now, thread.created_at).with(muted)
            )?;
            writeln!(buf)?;
            if let Some(diff) = &thread.diff_hunk {
                let diff_lines: Vec<String> = diff
                    .lines()
                    .map(|line| {
                        let styled_line = match line.chars().next() {
                            Some('+') => line.with(Color::DarkGreen),
                            Some('-') => line.with(Color::DarkRed),
                            Some('@') => line.with(Color::DarkGrey),
                            _ => line.with(Color::Grey),
                        };
                        styled_line.to_string()
                    })
                    .collect();
                render_block(buf, &diff_lines)?;
                writeln!(buf)?;
            }
            let wrap_opts = WrapOptions::new(COMMENT_WRAP).break_words(false);
            for (idx, comment) in thread.comments.iter().enumerate() {
                if idx > 0 {
                    // writeln!(buf)?;
                }
                // writeln!(
                //     buf,
                //     "{} {} {}",
                //     "╭".dark_grey(),
                //     comment
                //         .author
                //         .as_str()
                //         .with(Color::Rgb {
                //             r: 120,
                //             g: 200,
                //             b: 220
                //         })
                //         .bold(),
                //     humanize_relative(now, comment.created_at).with(muted)
                // )?;
                let mut body_lines = Vec::new();
                body_lines.push(format!(
                    "{} {}",
                    comment
                        .author
                        .as_str()
                        .with(Color::Rgb {
                            r: 120,
                            g: 200,
                            b: 220
                        })
                        .bold(),
                    humanize_relative(now, comment.created_at).with(muted)
                ));
                for line in comment.body.lines() {
                    if line.trim().is_empty() {
                        body_lines.push(String::new());
                    } else {
                        for chunk in wrap(line, wrap_opts.clone()) {
                            body_lines.push(chunk.into_owned());
                        }
                    }
                }
                render_block(buf, &body_lines)?;
                writeln!(buf)?;
            }
        }
        if !self.queued_replies.is_empty() {
            writeln!(
                buf,
                "{}",
                format!(
                    "{} replies queued – press p to publish",
                    self.queued_replies.len()
                )
                .with(Color::DarkMagenta)
            )?;
        }
        writeln!(
            buf,
            "{}",
            format!(
                "←/→ thread  ↑/↓ scroll  tab switch view  r reply  p publish  {}  q quit",
                self.view.skip_action_label()
            )
            .with(Color::DarkGrey)
        )?;
        if let Some(message) = &self.status_line {
            writeln!(buf, "{}", message.as_str().with(Color::DarkGrey))?;
        }
        Ok(())
    }

    fn dump_once(&self) -> Result<()> {
        print!("{}", self.render_view());
        Ok(())
    }

    fn next_thread(&mut self) {
        let len = self.current_threads().len();
        if len == 0 {
            return;
        }
        let index = self.current_index_mut();
        *index = (*index + 1) % len;
        self.reset_scroll();
    }

    fn prev_thread(&mut self) {
        let len = self.current_threads().len();
        if len == 0 {
            return;
        }
        let index = self.current_index_mut();
        if *index == 0 {
            *index = len - 1;
        } else {
            *index -= 1;
        }
        self.reset_scroll();
    }

    fn scroll_up(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    fn skip_current(&mut self) -> Result<()> {
        match self.view {
            ThreadView::Unresolved | ThreadView::Active => self.skip_selected_thread(),
            ThreadView::Skipped => self.unskip_selected_thread(),
        }
    }

    fn skip_selected_thread(&mut self) -> Result<()> {
        let thread = match self.view {
            ThreadView::Active => {
                self.clamp_index_for_view(ThreadView::Active);
                if self.active_threads.is_empty() {
                    self.status_line = Some("No thread to skip.".into());
                    return Ok(());
                }
                self.active_threads.remove(self.current_active)
            }
            ThreadView::Unresolved => {
                self.clamp_index_for_view(ThreadView::Unresolved);
                if self.unresolved_threads.is_empty() {
                    self.status_line = Some("No unresolved thread to skip.".into());
                    return Ok(());
                }
                let thread = self.unresolved_threads.remove(self.current_unresolved);
                if let Some(pos) = self
                    .active_threads
                    .iter()
                    .position(|candidate| candidate.id == thread.id)
                {
                    self.active_threads.remove(pos);
                    self.clamp_index_for_view(ThreadView::Active);
                }
                thread
            }
            ThreadView::Skipped => {
                self.status_line = Some("Cannot skip thread in skipped view.".into());
                return Ok(());
            }
        };
        self.skip_store
            .add(thread.id.clone())
            .context("failed to persist skip state")?;
        self.skipped_threads.push(thread);
        Self::sort_threads(&mut self.skipped_threads);
        self.rebuild_unresolved(None);
        self.clamp_index_for_view(self.view);
        self.reset_scroll();
        self.status_line = Some("Thread skipped.".into());
        Ok(())
    }

    fn unskip_selected_thread(&mut self) -> Result<()> {
        self.clamp_index_for_view(ThreadView::Skipped);
        if self.skipped_threads.is_empty() {
            self.status_line = Some("No skipped thread to unskip.".into());
            return Ok(());
        }
        let thread = self.skipped_threads.remove(self.current_skipped);
        self.skip_store
            .remove(&thread.id)
            .context("failed to persist skip state")?;
        self.active_threads.push(thread.clone());
        Self::sort_threads(&mut self.active_threads);
        let preferred = if thread.is_resolved {
            None
        } else {
            Some(thread.id.clone())
        };
        self.rebuild_unresolved(preferred);
        self.clamp_index_for_view(ThreadView::Skipped);
        self.reset_scroll();
        self.status_line = Some("Thread unskipped.".into());
        Ok(())
    }

    async fn reply_to_current(&mut self, terminal: &mut TerminalSession) -> Result<()> {
        let Some(thread) = self.current_thread() else {
            self.status_line = Some("No thread selected.".into());
            return Ok(());
        };
        let Some(target_comment) = thread.comments.last() else {
            self.status_line = Some("Thread has no comments.".into());
            return Ok(());
        };

        let editor_template = build_reply_editor_template(thread);
        let reply_body = match terminal.suspend_for_editor(&editor_template)? {
            Some(body) => body,
            None => {
                self.status_line = Some("Reply cancelled.".into());
                return Ok(());
            }
        };

        self.queued_replies.push_back(QueuedReply {
            comment_database_id: target_comment.database_id,
            body: reply_body,
        });
        self.status_line = Some(format!(
            "Reply queued ({} pending).",
            self.queued_replies.len()
        ));
        Ok(())
    }

    async fn publish_queue(&mut self) -> Result<()> {
        if self.queued_replies.is_empty() {
            self.status_line = None;
            return Ok(());
        }
        let total = self.queued_replies.len();
        let mut index = 0;
        while let Some(reply) = self.queued_replies.front().cloned() {
            self.status_line = Some(format!("Publishing reply {}/{}", index + 1, total,));

            self.render()?;

            self.gh
                .post_reply(
                    &self.repo,
                    self.pr_number,
                    reply.comment_database_id,
                    &reply.body,
                )
                .await?;
            self.queued_replies.pop_front();
            index += 1;
        }
        self.status_line = Some(format!("Published {} replies ✓", total));
        self.refresh_threads().await?;
        Ok(())
    }

    async fn refresh_threads(&mut self) -> Result<()> {
        let current_unresolved_id = self
            .unresolved_threads
            .get(self.current_unresolved)
            .map(|t| t.id.clone());
        let current_active_id = self
            .active_threads
            .get(self.current_active)
            .map(|t| t.id.clone());
        let current_skipped_id = self
            .skipped_threads
            .get(self.current_skipped)
            .map(|t| t.id.clone());
        let updated = self
            .gh
            .fetch_threads(&self.repo, self.pr_number)
            .await
            .context("failed to refresh threads")?;
        let (active, skipped) = Self::partition_threads(&self.skip_store, updated);
        self.active_threads = active;
        self.skipped_threads = skipped;
        self.restore_selection(ThreadView::Active, current_active_id);
        self.restore_selection(ThreadView::Skipped, current_skipped_id);
        self.rebuild_unresolved(current_unresolved_id);
        self.clamp_current_index();
        self.reset_scroll();
        Ok(())
    }

    fn restore_selection(&mut self, view: ThreadView, target: Option<String>) {
        let target_pos =
            target.and_then(|id| self.threads_for_view(view).iter().position(|t| t.id == id));
        let len = self.threads_for_view(view).len();
        let index = self.index_for_view_mut(view);
        if let Some(found) = target_pos {
            *index = found;
        } else if len == 0 {
            *index = 0;
        } else if *index >= len {
            *index = len - 1;
        }
    }
}

fn build_reply_editor_template(thread: &Thread) -> String {
    let mut buf = String::from("\n\n");
    let now = Utc::now();
    let status = if thread.is_resolved {
        "resolved"
    } else {
        "open"
    };
    let _ = writeln!(
        buf,
        "# Reply to the thread on {} ({} status).",
        thread.path, status
    );
    let _ = writeln!(
        buf,
        "# Lines starting with '# ' are ignored when submitting the reply."
    );
    let _ = writeln!(buf, "#");
    let _ = writeln!(buf, "# --- Thread comments ---");
    let context_width = COMMENT_WRAP.saturating_sub(2).max(10);
    let context_wrap = WrapOptions::new(context_width).break_words(false);
    for comment in &thread.comments {
        let _ = writeln!(
            buf,
            "# {} ({})",
            comment.author,
            humanize_relative(now, comment.created_at)
        );
        if comment.body.trim().is_empty() {
            let _ = writeln!(buf, "#");
            continue;
        }
        for line in comment.body.lines() {
            if line.trim().is_empty() {
                let _ = writeln!(buf, "#");
            } else {
                for chunk in wrap(line, context_wrap.clone()) {
                    let _ = writeln!(buf, "# {}", chunk);
                }
            }
        }
        let _ = writeln!(buf, "#");
    }
    buf
}

#[derive(Clone)]
struct Thread {
    id: String,
    path: String,
    diff_hunk: Option<String>,
    is_resolved: bool,
    created_at: DateTime<Utc>,
    comments: Vec<Comment>,
}

impl Thread {
    fn sort_key(&self) -> (&bool, &DateTime<Utc>) {
        (&self.is_resolved, &self.created_at)
    }
}

#[derive(Clone)]
struct Comment {
    id: String,
    database_id: u64,
    author: String,
    body: String,
    diff_hunk: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Clone)]
struct QueuedReply {
    comment_database_id: u64,
    body: String,
}

struct GhCli;

impl GhCli {
    fn new() -> Self {
        Self
    }

    async fn current_repo(&self) -> Result<Repo> {
        let output = self
            .run(["repo", "view", "--json", "name,owner"])
            .await
            .context("gh repo view failed")?;
        let repo: RepoResponse =
            serde_json::from_str(&output).context("failed to parse repo info")?;
        Ok(Repo {
            owner: repo.owner.login,
            name: repo.name,
        })
    }

    async fn current_pr_number(&self) -> Result<u64> {
        let output = self
            .run(["pr", "view", "--json", "number"])
            .await
            .context("gh pr view failed")?;
        let pr: PrResponse = serde_json::from_str(&output).context("failed to parse PR info")?;
        Ok(pr.number)
    }

    async fn fetch_threads(&self, repo: &Repo, pr_number: u64) -> Result<Vec<Thread>> {
        let query = r#"query($owner: String!, $name: String!, $number: Int!) {
            repository(owner: $owner, name: $name) {
                pullRequest(number: $number) {
                    reviewThreads(first: 100) {
                        nodes {
                            id
                            isResolved
                            path
                            comments(first: 100) {
                                nodes {
                            id
                            databaseId
                            body
                            diffHunk
                            createdAt
                                    author {
                                        login
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }"#;
        let args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={}", query),
            "-F".to_string(),
            format!("owner={}", repo.owner),
            "-F".to_string(),
            format!("name={}", repo.name),
            "-F".to_string(),
            format!("number={}", pr_number),
        ];
        let output = self
            .run(args)
            .await
            .context("failed to query review threads")?;
        let response: ThreadsResponse =
            serde_json::from_str(&output).context("failed to parse thread response")?;
        let raw_threads = response
            .data
            .repository
            .ok_or_else(|| anyhow!("repository missing from response"))?
            .pull_request
            .ok_or_else(|| anyhow!("pull request missing from response"))?
            .review_threads
            .nodes;
        let mut threads: Vec<Thread> = raw_threads
            .into_iter()
            .map(Thread::try_from)
            .collect::<Result<Vec<_>>>()?;
        threads.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        Ok(threads)
    }

    async fn post_reply(
        &self,
        repo: &Repo,
        pr_number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<()> {
        let endpoint = format!(
            "repos/{}/{}/pulls/{}/comments/{}/replies",
            repo.owner, repo.name, pr_number, comment_id
        );
        let args = vec![
            "api".to_string(),
            endpoint,
            "-X".to_string(),
            "POST".to_string(),
            "-f".to_string(),
            format!("body={}", body),
        ];
        self.run(args).await?;
        Ok(())
    }

    async fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args_vec: Vec<OsString> = args
            .into_iter()
            .map(|arg| arg.as_ref().to_os_string())
            .collect();
        let output = TokioCommand::new("gh")
            .args(&args_vec)
            .output()
            .await
            .context("failed to spawn gh")?;
        if !output.status.success() {
            let rendered: Vec<String> = args_vec
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            return Err(anyhow!(
                "gh {:?} failed: {}",
                rendered,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let stdout = String::from_utf8(output.stdout).context("invalid utf-8 from gh")?;
        Ok(stdout)
    }
}

struct Repo {
    owner: String,
    name: String,
}

#[derive(Deserialize)]
struct RepoResponse {
    name: String,
    owner: RepoOwner,
}

#[derive(Deserialize)]
struct RepoOwner {
    login: String,
}

#[derive(Deserialize)]
struct PrResponse {
    number: u64,
}

#[derive(Deserialize)]
struct ThreadsResponse {
    data: ThreadData,
}

#[derive(Deserialize)]
struct ThreadData {
    repository: Option<ThreadRepo>,
}

#[derive(Deserialize)]
struct ThreadRepo {
    #[serde(rename = "pullRequest")]
    pull_request: Option<PullRequest>,
}

#[derive(Deserialize)]
struct PullRequest {
    #[serde(rename = "reviewThreads")]
    review_threads: ThreadConnection,
}

#[derive(Deserialize)]
struct ThreadConnection {
    nodes: Vec<RawThread>,
}

#[derive(Deserialize)]
struct RawThread {
    id: String,
    #[serde(rename = "isResolved")]
    is_resolved: bool,
    path: Option<String>,
    comments: RawCommentConnection,
}

#[derive(Deserialize)]
struct RawCommentConnection {
    nodes: Vec<RawComment>,
}

#[derive(Deserialize)]
struct RawComment {
    id: String,
    #[serde(rename = "databaseId")]
    database_id: Option<u64>,
    body: String,
    #[serde(rename = "diffHunk")]
    diff_hunk: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    author: Option<RawAuthor>,
}

#[derive(Deserialize)]
struct RawAuthor {
    login: String,
}

impl TryFrom<RawThread> for Thread {
    type Error = anyhow::Error;

    fn try_from(raw: RawThread) -> Result<Self> {
        if raw.comments.nodes.is_empty() {
            return Err(anyhow!("thread missing comments"));
        }
        let comments: Vec<Comment> = raw
            .comments
            .nodes
            .into_iter()
            .map(Comment::try_from)
            .collect::<Result<_>>()?;
        let created_at = comments
            .first()
            .map(|c| c.created_at.clone())
            .ok_or_else(|| anyhow!("thread missing creation time"))?;
        let diff_hunk = comments.iter().find_map(|c| c.diff_hunk.clone());
        Ok(Thread {
            id: raw.id,
            path: raw.path.unwrap_or_else(|| "unknown".into()),
            diff_hunk,
            is_resolved: raw.is_resolved,
            created_at,
            comments,
        })
    }
}

impl TryFrom<RawComment> for Comment {
    type Error = anyhow::Error;

    fn try_from(raw: RawComment) -> Result<Self> {
        let created_at = parse_timestamp(&raw.created_at)?;
        let database_id = raw
            .database_id
            .ok_or_else(|| anyhow!("comment missing databaseId"))?;
        Ok(Self {
            id: raw.id,
            database_id,
            author: raw
                .author
                .map(|a| a.login)
                .unwrap_or_else(|| "unknown".into()),
            body: raw.body,
            diff_hunk: raw.diff_hunk,
            created_at,
        })
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    let dt = DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid timestamp: {value}"))?;
    Ok(dt.with_timezone(&Utc))
}

fn humanize_relative(now: DateTime<Utc>, then: DateTime<Utc>) -> String {
    let delta = now.signed_duration_since(then);
    let text = HumanTime::from(delta).to_string();
    if let Some(rest) = text.strip_prefix("in ") {
        format!("{} ago", rest.trim())
    } else {
        text
    }
}

fn render_block(buf: &mut String, lines: &[String]) -> std::fmt::Result {
    if lines.is_empty() {
        writeln!(buf, "{}", "│".with(Color::DarkGrey))?;
        return Ok(());
    }
    for (i, line) in lines.iter().enumerate() {
        let block = match i {
            0 if lines.len() == 1 => "",
            0 if lines.len() > 1 => "╭",
            _ if i + 1 == lines.len() => "╰",
            _ => "│",
        };

        if line.is_empty() {
            writeln!(buf, "{}", block.with(Color::DarkGrey))?;
        } else {
            writeln!(buf, "{} {}", block.with(Color::DarkGrey), line)?;
        }
    }
    Ok(())
}

struct SkipStore {
    path: PathBuf,
    skipped: HashSet<String>,
}

impl SkipStore {
    fn load() -> Result<Self> {
        let path = Self::default_path()?;
        let skipped = match fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str::<Vec<String>>(&raw)
                .map(|items| items.into_iter().collect())
                .unwrap_or_default(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => HashSet::new(),
            Err(err) => return Err(err.into()),
        };
        Ok(Self { path, skipped })
    }

    fn contains(&self, id: &str) -> bool {
        self.skipped.contains(id)
    }

    fn add(&mut self, id: String) -> Result<()> {
        if self.skipped.insert(id) {
            self.persist()?;
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Result<()> {
        if self.skipped.remove(id) {
            self.persist()?;
        }
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let payload: Vec<&String> = self.skipped.iter().collect();
        let data = serde_json::to_vec_pretty(&payload)?;
        fs::write(&self.path, data)?;
        Ok(())
    }

    fn default_path() -> Result<PathBuf> {
        if let Ok(dir) = env::var("XDG_STATE_HOME") {
            if !dir.is_empty() {
                return Ok(Path::new(&dir).join("gh-cr").join("skipped.json"));
            }
        }
        let mut path = dirs_next::home_dir().ok_or_else(|| anyhow!("HOME not set"))?;
        path.push(".local");
        path.push("state");
        path.push("gh-cr");
        path.push("skipped.json");
        Ok(path)
    }
}

struct TerminalSession {
    active: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self> {
        let mut out = stdout();
        enable_raw_mode().context("enable raw mode failed")?;
        execute!(
            out,
            EnterAlternateScreen,
            Clear(ClearType::All),
            Hide,
            EnableMouseCapture
        )
        .context("failed to configure terminal")?;
        Ok(Self { active: true })
    }

    fn suspend_for_editor(&mut self, initial_contents: &str) -> Result<Option<String>> {
        self.deactivate()?;
        let result = launch_editor(initial_contents);
        self.activate()?;
        result
    }

    fn activate(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        let mut out = stdout();
        enable_raw_mode().context("enable raw mode failed")?;
        execute!(
            out,
            EnterAlternateScreen,
            Clear(ClearType::All),
            Hide,
            EnableMouseCapture
        )?;
        self.active = true;
        Ok(())
    }

    fn deactivate(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let mut out = stdout();
        disable_raw_mode().ok();
        execute!(out, DisableMouseCapture, Show, LeaveAlternateScreen)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.deactivate();
    }
}

fn launch_editor(initial_contents: &str) -> Result<Option<String>> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());
    println!("Opening editor: {editor}");
    let mut tempfile = NamedTempFile::new().context("unable to create temp file")?;
    tempfile
        .write_all(initial_contents.as_bytes())
        .context("failed to prime editor template")?;
    tempfile.flush()?;
    let status = StdCommand::new(&editor)
        .arg(tempfile.path())
        .status()
        .with_context(|| format!("failed to launch editor: {editor}"))?;
    if !status.success() {
        return Err(anyhow!("editor exited with {}", status));
    }
    let body = fs::read_to_string(tempfile.path()).context("failed to read editor contents")?;
    Ok(sanitize_editor_contents(&body))
}

fn sanitize_editor_contents(raw: &str) -> Option<String> {
    let filtered_lines: Vec<&str> = raw
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    let filtered = filtered_lines.join("\n");
    let trimmed = filtered.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
