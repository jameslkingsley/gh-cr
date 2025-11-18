# Codex Prompt — Build a `gh cr` Code Review TUI (Rust + Crossterm)

Create a GitHub CLI extension using `gh extension create` and implement the extension in Rust. The final binary must run as `gh cr` and open an interactive terminal UI (using only crossterm) that allows navigating and responding to pull-request review threads for the PR associated with the current Git branch.

## High-Level Requirements

### 1. GitHub CLI extension structure
- Use `gh extension create <name>`.
- Entrypoint script should call the Rust binary.
- Must run as `gh cr`.

### 2. Auto-detect PR
- Use `gh pr view --json number` or equivalent.
- Fail gracefully if no PR is found.

### 3. Fetch review comments
- Use `gh api` to fetch review threads, including:
  - comment bodies
  - authors
  - replies
  - file paths
  - diff hunks

### 4. Interactive REPL UI with crossterm
- Alternate screen + raw mode.
- Arrow keys or j/k to navigate threads.
- Display:
  - File path
  - Diff hunk
  - Original comment (author + body)
  - Replies

### 5. Actions
- **c**: Open $EDITOR (vim) to write a reply, then POST via gh api.
- **s**: Skip thread and persist skip state.
- **q**: Quit.

## Skipped Thread Persistence
- Persist skipped thread IDs in:
  - $XDG_STATE_HOME/gh-cr/skipped.json
  - fallback: $HOME/.local/state/gh-cr/skipped.json
- On startup, load file and ignore skipped threads.

## Thread Ordering
- Show first unresolved/unskipped thread.
- Others in chronological order.

## Implementation Notes for Codex
- Use RAII to clean up terminal state.
- Editor logic:
  ```
  let editor = std::env::var("EDITOR").unwrap_or("vim".into());
  ```
- Minimal dependencies: serde, serde_json, tempfile, crossterm, dirs-next, etc.
- Whole project should compile on stable Rust.
