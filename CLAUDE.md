# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project: GitHub Kanban Native App (Minik)

A minimalist cross-platform native application that displays GitHub project Kanbans with a macOS Stickies-inspired design, built with Tauri and Rust. The app provides quick visibility into project status while staying unobtrusive and always accessible.

## Setup and Build Commands

```bash
# Install prerequisites
cargo install tauri-cli

# Development
cargo tauri dev

# Build for production
cargo tauri build

# Run tests
cargo test
cargo tauri test

# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings
```

## Architecture Overview

### Technology Stack
- **Backend**: Rust with Tauri for system integration and GitHub API handling
- **Frontend**: HTML/CSS/JavaScript rendered in Tauri's web view
- **GitHub Integration**: Uses existing `gh` CLI authentication via `gh auth token`

### Key Components

1. **GitHub API Integration**
   - Leverages GitHub GraphQL API for Projects v2
   - Polls for updates every 5 minutes
   - No local caching - always fetches fresh data
   - Handles both Issues and Pull Requests

2. **UI States**
   - **Minimized View**: Compact horizontal strip showing column summaries
   - **Expanded View**: Full Kanban board with cards
   - Always-on-top window behavior

3. **Platform Features**
   - **macOS**: Native menubar integration
   - **Linux**: System tray support
   - Window state persistence (position, size, selected project)

## GitHub API Endpoints

- Organizations: `GET /user/orgs`
- Project data: GraphQL `organization.projectsV2` and `projectV2` queries
- Authentication: Via `gh auth token` command output

## UI Specifications

### Minimized View (Default)
- Always on top window behavior
- Format: `[ Column1 - 4 | Column2 - 2 ][ Column3 - 0 | Column4 - 3 ]`
- Semi-transparent rounded rectangle with subtle drop shadow
- SF Pro typography on macOS
- Double-click to expand

### Expanded View
- Ultra-compact layout with maximum information density
- Column colors: Yellow, Blue, Green, Pink, Orange, Purple (cycle as needed)
- Card details: Issue/PR titles, assignees, labels
- Click cards to open in GitHub
- ESC or click outside to minimize

### Menubar Features
- Hierarchical organization/project structure
- "Hide/Show Columns" submenu (per-project settings)
- "Show Only My Items" toggle filter
- Manual refresh option

## Development Guidelines

- Keep the UI ultra-compact and minimalist
- Prioritize information density in the expanded view
- Maintain Stickies-inspired visual aesthetics (rounded corners, subtle shadows, distinct column colors)
- Ensure smooth transitions between minimized/expanded states
- Handle GitHub API rate limits gracefully
- No offline mode - require network connectivity
- Window state persistence in JSON (position, size, selected project)
- Multi-monitor drag support

## Implementation Phases

### Phase 1 (MVP)
- Basic GitHub authentication via gh CLI
- Single project Kanban display
- Minimized/expanded view toggle
- Menubar project selection

### Phase 2
- Column visibility preferences
- User assignment filtering
- Click-through links to GitHub

### Phase 3
- Multi-monitor drag support
- Window state persistence
- Performance optimizations