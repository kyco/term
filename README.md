# TermAI

A multi-provider AI assistant for your terminal, written in Rust. TermAI works with Anthropic Claude, OpenAI GPT-5.x, and OpenAI Codex (via ChatGPT subscription OAuth), and keeps everything local: sessions with git-style conversation branching, reusable prompt presets, AI-powered Git integration (commits, reviews, conflict resolution), smart context discovery for your codebase, and SQLite-backed storage with privacy redaction. Chat sessions can optionally use web search/fetch tools (opt-in via `/tools`).

## Install

```bash
# Prerequisites: Rust and Cargo
git clone https://github.com/kyco/termai.git
cd termai
cargo build --release
# binary at target/release/termai (or: cargo install --path .)
```

## Quickstart

```bash
termai setup                     # Interactive setup wizard
termai auth login claude         # Or: openai, codex (ChatGPT OAuth)
termai ask "Explain this code" ./src/main.rs
termai chat                      # Interactive chat session (saved automatically)
termai sessions list             # Find a past conversation
termai sessions find "flyway"    # ...or search what was said in one
```

Chats are saved as you go: each turn is written when it completes, and your
prompt is stored *before* the request goes out, so a dropped connection costs
you nothing. Use `termai chat --temporary` for a conversation you do not want
kept.

Upgrading from an earlier version? Run `termai sessions repair` — older
versions re-inserted the whole conversation on every turn, and it will show
you (and, with `--apply`, clean up) the duplicates.

## Documentation

Full documentation lives in [docs/README.md](docs/README.md), with a command reference in [docs/COMMANDS.md](docs/COMMANDS.md) and a cheat sheet in [docs/QUICK_REFERENCE.md](docs/QUICK_REFERENCE.md).

## License

MIT — see [LICENSE](LICENSE).
