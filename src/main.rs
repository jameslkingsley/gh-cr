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
use tokio::{
    process::Command as TokioCommand,
    time::{Duration, sleep},
};

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

    let pr_number = match gh.current_pr_number().await {
        Ok(num) => num,
        Err(err) => {
            eprintln!("No pull request associated with the current branch: {err}");
            return Ok(());
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
    threads: Vec<Thread>,
    current: usize,
    status_line: Option<String>,
    scroll_offset: usize,
    queued_replies: VecDeque<QueuedReply>,
}

impl App {
    fn new(
        gh: GhCli,
        repo: Repo,
        pr_number: u64,
        skip_store: SkipStore,
        threads: Vec<Thread>,
    ) -> Self {
        let mut app = Self {
            gh,
            repo,
            pr_number,
            skip_store,
            threads,
            current: 0,
            status_line: None,
            scroll_offset: 0,
            queued_replies: VecDeque::new(),
        };
        app.apply_skips();
        app
    }

    async fn run(&mut self) -> Result<()> {
        let mut terminal = TerminalSession::enter()?;
        loop {
            self.render()?;
            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    match key.code {
                        KeyCode::Char('q') if key.modifiers.is_empty() => break,
                        KeyCode::Char('j') if key.modifiers.is_empty() => self.next_thread(),
                        KeyCode::Char('k') if key.modifiers.is_empty() => self.prev_thread(),
                        KeyCode::Right if key.modifiers.is_empty() => self.next_thread(),
                        KeyCode::Left if key.modifiers.is_empty() => self.prev_thread(),
                        KeyCode::Down if key.modifiers.is_empty() => self.scroll_down(1),
                        KeyCode::Up if key.modifiers.is_empty() => self.scroll_up(1),
                        KeyCode::Char('s') if key.modifiers.is_empty() => {
                            if let Err(err) = self.skip_current() {
                                self.status_line = Some(format!("Failed to skip thread: {err}"));
                            }
                        }
                        KeyCode::Char('c') if key.modifiers.is_empty() => {
                            if let Err(err) = self.reply_to_current(&mut terminal).await {
                                self.status_line = Some(format!("Failed to post reply: {err}"));
                            }
                        }
                        KeyCode::Char('p') if key.modifiers.is_empty() => {
                            if let Err(err) = self.publish_queue().await {
                                self.status_line =
                                    Some(format!("Failed to publish replies: {err}"));
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(me) => match me.kind {
                    MouseEventKind::ScrollUp => self.scroll_up(3),
                    MouseEventKind::ScrollDown => self.scroll_down(3),
                    _ => {}
                },
                Event::Resize(_, _) => {
                    // Re-render on resize.
                }
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
        if self.threads.is_empty() {
            writeln!(
                buf,
                "{}",
                format!("PR #{} – No review threads to display.", self.pr_number).bold()
            )?;
            writeln!(buf, "{}", "Press q to exit.".with(Color::DarkGrey))?;
        } else {
            let thread = &self.threads[self.current];
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
                format!("Thread {}/{}", self.current + 1, self.threads.len())
                    .with(muted)
                    .bold(),
                format!("PR #{}", self.pr_number).with(muted)
            )?;
            writeln!(
                buf,
                "{}   {}   {}",
                thread.path.as_str().with(accent),
                if thread.is_resolved {
                    "resolved".with(muted)
                } else {
                    "open".with(Color::DarkYellow)
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
                            Some('@') => line.with(Color::DarkYellow),
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
                    writeln!(buf)?;
                }
                writeln!(
                    buf,
                    "{}  {}",
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
                )?;
                let mut body_lines = Vec::new();
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
                .with(Color::DarkYellow)
            )?;
        }
        writeln!(
            buf,
            "{}",
            "←/→ thread  ↑/↓ scroll  c reply  p publish  s skip  q quit".with(Color::DarkGrey)
        )?;
        if let Some(message) = &self.status_line {
            writeln!(buf, "{}", message.as_str().with(Color::Yellow))?;
        }
        Ok(())
    }

    fn dump_once(&self) -> Result<()> {
        print!("{}", self.render_view());
        Ok(())
    }

    fn next_thread(&mut self) {
        if self.threads.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.threads.len();
        self.reset_scroll();
    }

    fn prev_thread(&mut self) {
        if self.threads.is_empty() {
            return;
        }
        if self.current == 0 {
            self.current = self.threads.len() - 1;
        } else {
            self.current -= 1;
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
        let Some(thread) = self.threads.get(self.current) else {
            self.status_line = Some("No thread to skip.".into());
            return Ok(());
        };
        self.skip_store
            .add(thread.id.clone())
            .context("failed to persist skip state")?;
        self.threads.remove(self.current);
        self.clamp_index();
        self.reset_scroll();
        self.status_line = Some("Thread skipped.".into());
        Ok(())
    }

    async fn reply_to_current(&mut self, terminal: &mut TerminalSession) -> Result<()> {
        let Some(thread) = self.threads.get(self.current) else {
            self.status_line = Some("No thread selected.".into());
            return Ok(());
        };
        let Some(target_comment) = thread.comments.last() else {
            self.status_line = Some("Thread has no comments.".into());
            return Ok(());
        };

        let reply_body = match terminal.suspend_for_editor()? {
            Some(body) => body,
            None => {
                self.status_line = Some("Reply cancelled.".into());
                return Ok(());
            }
        };

        self.queued_replies.push_back(QueuedReply {
            comment_id: target_comment.id.clone(),
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
            self.status_line = Some("No queued replies to publish.".into());
            return Ok(());
        }
        let total = self.queued_replies.len();
        let spinner = ['|', '/', '-', '\\'];
        let mut index = 0;
        while let Some(reply) = self.queued_replies.front().cloned() {
            for &frame in &spinner {
                self.status_line = Some(format!(
                    "Publishing reply {}/{} {}",
                    index + 1,
                    total,
                    frame
                ));
                self.render()?;
                sleep(Duration::from_millis(80)).await;
            }
            self.gh
                .post_reply(&reply.comment_id, &reply.body)
                .await
                .context("failed to publish reply")?;
            self.queued_replies.pop_front();
            index += 1;
        }
        self.status_line = Some(format!("Published {} replies ✓", total));
        self.refresh_threads().await?;
        Ok(())
    }

    fn apply_skips(&mut self) {
        self.threads
            .retain(|thread| !self.skip_store.contains(&thread.id));
        self.clamp_index();
        self.reset_scroll();
    }

    async fn refresh_threads(&mut self) -> Result<()> {
        let current_id = self.threads.get(self.current).map(|t| t.id.clone());
        let updated = self
            .gh
            .fetch_threads(&self.repo, self.pr_number)
            .await
            .context("failed to refresh threads")?;
        self.threads = updated
            .into_iter()
            .filter(|t| !self.skip_store.contains(&t.id))
            .collect();
        if let Some(id) = current_id {
            if let Some(index) = self.threads.iter().position(|t| t.id == id) {
                self.current = index;
            } else {
                self.clamp_index();
                self.reset_scroll();
            }
        } else {
            self.clamp_index();
            self.reset_scroll();
        }
        Ok(())
    }

    fn clamp_index(&mut self) {
        if self.threads.is_empty() {
            self.current = 0;
            self.scroll_offset = 0;
        } else if self.current >= self.threads.len() {
            self.current = self.threads.len() - 1;
        }
    }
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
    author: String,
    body: String,
    diff_hunk: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Clone)]
struct QueuedReply {
    comment_id: String,
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

    async fn post_reply(&self, comment_id: &str, body: &str) -> Result<()> {
        let mutation = r#"mutation($commentId: ID!, $body: String!) {
            addPullRequestReviewComment(input: {commentId: $commentId, body: $body}) {
                comment { id }
            }
        }"#;
        let args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={}", mutation),
            "-F".to_string(),
            format!("commentId={}", comment_id),
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
        Ok(Self {
            id: raw.id,
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
    for line in lines {
        if line.is_empty() {
            writeln!(buf, "{}", "│".with(Color::DarkGrey))?;
        } else {
            writeln!(buf, "{} {}", "│".with(Color::DarkGrey), line)?;
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

    fn suspend_for_editor(&mut self) -> Result<Option<String>> {
        self.deactivate()?;
        let result = launch_editor();
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

fn launch_editor() -> Result<Option<String>> {
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".into());
    println!("Opening editor: {editor}");
    let mut tempfile = NamedTempFile::new().context("unable to create temp file")?;
    tempfile.flush()?;
    let status = StdCommand::new(&editor)
        .arg(tempfile.path())
        .status()
        .with_context(|| format!("failed to launch editor: {editor}"))?;
    if !status.success() {
        return Err(anyhow!("editor exited with {}", status));
    }
    let body = fs::read_to_string(tempfile.path()).context("failed to read editor contents")?;
    if body.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(body))
}
