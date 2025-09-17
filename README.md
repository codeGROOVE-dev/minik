# minik

Native GitHub Kanban boards but tiny. Like _really_ tiny.

![minik screenshot](media/minik.png)

## What

Inspired by the macOS Stickies app, it's a floating Kanban for your GitHub project boards.

- **Minimalist**: Collapses to a tiny strip when you don't need it
- **Always on top**: Stay on task
- **Native**: Built with Rust & Tauri

It's only been tested on macOS, but it should work anywhere.

## Features

- ✅ Minimized/expanded views
- ✅ Column visibility toggles
- ✅ Drag & drop items between columns
- ✅ Click through to GitHub issues/PRs
- ✅ Uses your existing `gh` CLI auth

## Quick Start

```bash
# Prerequisites: Rust + GitHub CLI
gh auth login
cargo install tauri-cli

# Run it
git clone https://github.com/tstromberg/minik
cd minik
cargo tauri dev
```

## Why "minik"?

Mini + kanban = minik.

I get distracted easily so I needed something to keep me on task.

## Status

Works better than expected, breaks less than feared. Still rough around the edges but genuinely useful for daily project tracking.

## License

Apache
