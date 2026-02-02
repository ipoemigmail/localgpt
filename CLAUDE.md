# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Development Commands

```bash
# Build
cargo build              # Debug build
cargo build --release    # Release build (~7MB binary)

# Run
cargo run -- <subcommand>   # Run with arguments
cargo run -- chat           # Interactive chat
cargo run -- ask "question" # Single question
cargo run -- daemon start   # Start daemon with HTTP server

# Test
cargo test                  # Run all tests
cargo test <test_name>      # Run specific test
cargo test -- --nocapture   # Show test output

# Lint
cargo clippy
cargo fmt --check
```

## Architecture

LocalGPT is a local-only AI assistant with persistent markdown-based memory and optional autonomous operation via heartbeat.

### Core Modules (`src/`)

- **agent/** - LLM interaction layer
  - `providers.rs` - Trait `LLMProvider` with implementations for OpenAI, Anthropic, and Ollama. Model prefix determines provider (`gpt-*` → OpenAI, `claude-*` → Anthropic, else Ollama)
  - `session.rs` - Conversation state with automatic compaction when approaching context window limits
  - `tools.rs` - Agent tools: `bash`, `read_file`, `write_file`, `edit_file`, `memory_search`, `memory_append`, `web_fetch`

- **memory/** - Markdown-based knowledge store
  - `index.rs` - SQLite FTS5 index for fast search. Chunks files (~400 tokens with 80 token overlap)
  - `watcher.rs` - File system watcher for automatic reindexing
  - Files: `MEMORY.md` (curated knowledge), `HEARTBEAT.md` (pending tasks), `memory/YYYY-MM-DD.md` (daily logs)

- **heartbeat/** - Autonomous task runner
  - `runner.rs` - Runs on configurable interval within active hours. Reads `HEARTBEAT.md` and executes pending tasks

- **server/** - HTTP/WebSocket API
  - `http.rs` - Axum-based REST API. Note: creates new Agent per request (no session persistence via HTTP)
  - Endpoints: `/health`, `/api/status`, `/api/chat`, `/api/memory/search`, `/api/memory/stats`

- **config/** - TOML configuration at `~/.localgpt/config.toml`
  - Supports `${ENV_VAR}` expansion in API keys
  - `workspace_path()` returns expanded memory workspace path

- **cli/** - Clap-based subcommands: `chat`, `ask`, `daemon`, `memory`, `config`

### Key Patterns

- Agent is not `Send+Sync` due to SQLite connections - HTTP handler uses `spawn_blocking`
- Session compaction triggers memory flush (prompts LLM to save important context before truncating)
- Memory context automatically loaded into new sessions: `MEMORY.md`, recent daily logs, `HEARTBEAT.md`
- Tools use `shellexpand::tilde()` for path expansion

## Configuration

Default config path: `~/.localgpt/config.toml` (see `config.example.toml`)

Key settings:
- `agent.default_model` - Model name (determines provider)
- `agent.context_window` / `reserve_tokens` - Context management
- `heartbeat.interval` - Duration string (e.g., "30m", "1h")
- `heartbeat.active_hours` - Optional `{start, end}` in "HH:MM" format
- `server.port` - HTTP server port (default: 18790)
