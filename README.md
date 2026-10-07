# jura

Terminal Jira client with git and AI integration.

## Installation

Requires a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo install --git https://github.com/palmensimon/jura.git
```

## Configuration

Run `jura init`:

```sh
jura init
```

It walks you through your Jira base URL and token, tests the connection, lets you pick a
default project from your real Jira projects (or type one), and offers to install the AI
skill. Running bare `jura` with no config yet launches the same wizard automatically.

Config is stored in the platform default location:

- **Linux:** `~/.config/jura/`
- **macOS:** `~/Library/Application Support/jura/`
- **Windows:** `%APPDATA%\jura\`

| File | Purpose | Edit via |
|---|---|---|
| `config.yaml` | Jira credentials and default project (`base_url`, `token`, `project`) | `jura init`, TUI `s` → Settings, or directly |
| `user_settings.yaml` | Preferences (filters, behaviour) | TUI `s` → Settings or `Ctrl+D`, or directly |
| `templates.yaml` | Create-ticket templates | TUI `s` → `Ctrl+T`, or directly |

## Usage

| Command | Description |
|---|---|
| `jura` | Open the TUI |
| `jura tickets` | List assigned tickets (JSON, reads local cache) |
| `jura ticket <KEY>` | Full details for a ticket |
| `jura current` | Full details for the ticket linked to the current git branch |
| `jura init` | Interactive wizard to configure credentials and defaults |
| `jura install-skill [--path <file>]` | Write the cli AI skill file |
| `jura help [command]` | Show CLI help |

Inside the TUI, press `?` for keybindings and `q` to quit.
