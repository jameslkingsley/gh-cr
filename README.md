Read and reply to GitHub pull request reviews and comments from your terminal.

## Requirements

- GitHub CLI (`gh`) authenticated to the repo you want to review.
- A local checkout of the PR branch (or pass a PR number explicitly).

## Install

```
gh extension install jameslkingsley/gh-threads
```

## Usage

- Attach to the current branch’s PR:
  ```
  gh threads
  ```
- Specify a PR number explicitly:
  ```
  gh threads <pr-number>
  ```

## Controls

- Left/Right: previous/next thread
- Up/Down or mouse wheel: scroll
- Tab: switch between unresolved / unskipped / skipped
- d: toggle diff hunk
- s: skip/unskip thread
- r: write a reply (opens $EDITOR)
- p: publish queued replies
- q: quit

## Roadmap

- Comment on diff hunks
- Add comment reactions
- Mark comments as resolved
