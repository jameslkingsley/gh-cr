<p align="center">
    <img src=".github/demo.gif" alt="gh-threads demo" width="480">
</p>

Read and reply to GitHub pull request review threads directly from your terminal.

**Status:**

- Early development. The roadmap for `v0.1` is [tracked here](https://github.com/jameslkingsley/gh-threads/issues/3)
- Currently only supports GitHub

## Requirements

- [GitHub CLI](https://cli.github.com) (`gh`) installed and authenticated

## Install

```bash
gh extension install jameslkingsley/gh-threads
```

## Usage

Just run `gh threads` in your repository; it will infer the pull request from the current branch. For more options run:

```bash
gh threads --help
```

## Controls

- Left/Right: previous/next thread
- Up/Down or mouse wheel: scroll
- Tab: switch between conversation / reviews (unresolved) / reviews (all)
- d: toggle diff hunk / description
- r: post a reply (opens `$EDITOR`)
- q: quit

## Contributing

Contributions are welcome! The long-term goal of this tool is to provide a seamless experience for responding to and conducting code reviews directly from the terminal.
