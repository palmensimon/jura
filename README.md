# fira

Terminal Jira client with git and Claude Code integration.

## Installation

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo install --git https://github.com/palmensimon/fira.git
```

## Configuration

```sh
fira init
```

Edit the generated `config.yaml` with your Jira credentials (`base_url`, `token`). Config is stored in the platform default location:

- **Linux:** `~/.config/fira/`
- **macOS:** `~/Library/Application Support/fira/`
- **Windows:** `%APPDATA%\fira\`

## Usage

| Command | Description |
|---|---|
| `fira` | Open the TUI |
| `fira tickets` | List assigned tickets (JSON, reads local cache) |
| `fira ticket <KEY>` | Full details for a ticket |
| `fira init` | Write example config files |

The local cache is populated when you open the Mine tab in the TUI.

## AI Integration

Install `jira-mcp.skill` to give your AI agent access to your Jira tickets via the CLI commands above.
