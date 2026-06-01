# jura

Terminal Jira client with git and AI integration.

## Installation

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo install --git https://github.com/palmensimon/jura.git
```

## Configuration

```sh
jura init
```

Config is stored in the platform default location:

- **Linux:** `~/.config/jura/`
- **macOS:** `~/Library/Application Support/jura/`
- **Windows:** `%APPDATA%\jura\`

Two files are generated:

| File | Purpose | Edit via |
|---|---|---|
| `config.yaml` | Jira credentials (`base_url`, `token`) | TUI `s` → Settings, or directly |
| `user_settings.yaml` | Preferences (`project`, filters, behaviour) | TUI `s` → Settings or `Ctrl+D`, or directly |
| `templates.yaml` | Create-ticket templates | TUI `s` → `Ctrl+T`, or directly |

## Usage

| Command | Description |
|---|---|
| `jura` | Open the TUI |
| `jura tickets` | List assigned tickets (JSON, reads local cache) |
| `jura ticket <KEY>` | Full details for a ticket |
| `jura current` | Full details for the ticket linked to the current git branch |
| `jura init` | Write example config files |
| `jura install-skill [--path <file>]` | Write the AI skill file |

The local cache is populated when you open the Mine tab in the TUI.

## AI Integration

Run `jura install-skill` to create a `jura-cli/` directory containing `SKILL.md` (or `--path <dir>` to place it elsewhere), then install it in your AI agent.

This gives your agent read-only access to your cached Jira tickets via the CLI commands above.
