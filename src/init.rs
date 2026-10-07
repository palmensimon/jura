//! Implements `jura init`: collects Jira credentials and a default project, verifies them
//! against the Jira API, writes the config files, and optionally installs the AI skill.
//! Also auto-launched by a bare `jura` run when no config exists yet.

use std::io::{self, Write};

use anyhow::{Context, Result};

use crate::cli;
use crate::config::{self, Config, JiraConfig};
use crate::git;
use crate::jira::{JiraClient, ProjectSummary};

const MAX_LISTED_PROJECTS: usize = 30;

/// Runs `jura init`. Returns `Some(config)` if it completed and the config was written to
/// disk, or `None` if the user cancelled/aborted (nothing was written).
pub async fn run_wizard(existing: Option<Config>) -> Result<Option<Config>> {
    println!("── jura init ──────────────────────────────────────────────");
    println!();

    if let Some(cfg) = &existing {
        println!("Existing config found at {}", config::config_dir().join("config.yaml").display());
        println!("  base_url: {}", cfg.jira.base_url);
        println!("  token:    {}", mask(&cfg.jira.token));
        println!("  project:  {}", cfg.project.as_deref().unwrap_or("(none)"));
        println!();
        if !prompt_yes_no("Reconfigure?", false)? {
            return Ok(None);
        }
        println!();
    }

    let existing_base_url = existing.as_ref().map(|c| c.jira.base_url.clone());
    let existing_token = existing.as_ref().map(|c| c.jira.token.clone());
    let existing_project = existing.as_ref().and_then(|c| c.project.clone());

    let (base_url, token, client) = loop {
        let base_url = prompt_required("Jira base URL", existing_base_url.as_deref())?;
        let base_url = base_url.trim_end_matches('/').to_string();

        let pat_url = format!("{base_url}/secure/ViewProfile.jspa");
        println!("Personal access tokens are managed at: {pat_url}");
        if prompt_yes_no("Open it in your browser now?", true)? {
            if let Err(e) = git::open_url(&pat_url, None) {
                println!("Could not open browser: {e:#}");
            }
        }
        let token = prompt_masked_required("Token / PAT", existing_token.as_deref())?;

        let client = JiraClient::new(&JiraConfig { base_url: base_url.clone(), token: token.clone() })?;
        print!("Testing connection... ");
        io::stdout().flush().ok();
        match client.get_myself().await {
            Ok(user) => {
                let name = user.display_name.or(user.name).unwrap_or_else(|| "unknown user".to_string());
                println!("connected as {name}");
                break (base_url, token, client);
            }
            Err(e) => {
                println!("failed: {e:#}");
                match prompt_choice("[r]etry / [s]kip / [a]bort", &['r', 's', 'a'], 'r')? {
                    'r' => continue,
                    's' => break (base_url, token, client),
                    _ => return Ok(None),
                }
            }
        }
    };

    println!();
    let project = pick_project(&client, existing_project.as_deref()).await?;

    println!();
    let config_path = config::config_dir().join("config.yaml");
    println!("Will write to {}", config_path.display());
    println!("  base_url: {base_url}");
    println!("  token:    {}", mask(&token));
    println!("  project:  {}", project.as_deref().unwrap_or("(none)"));
    println!();
    if !prompt_yes_no("Write config?", true)? {
        return Ok(None);
    }

    let defaults = existing.map(|c| c.defaults).unwrap_or_default();
    let cfg = Config { jira: JiraConfig { base_url, token }, project, defaults };
    config::save_config(&cfg)?;
    config::write_default_settings_file()?;
    config::write_default_templates_file()?;
    println!();
    println!("Config saved.");

    println!();
    if prompt_yes_no("Install the AI skill into the current directory now?", true)? {
        let cwd = std::env::current_dir().context("Could not determine current directory")?;
        match cli::install_skill(&cwd) {
            Ok(path) => println!("Skill written to {}", path.display()),
            Err(e) => println!("Could not install skill: {e:#}"),
        }
    }

    Ok(Some(cfg))
}

// ── Project picker ───────────────────────────────────────────────────────────

async fn pick_project(client: &JiraClient, default: Option<&str>) -> Result<Option<String>> {
    let fetched = client.get_projects().await.ok().filter(|p| !p.is_empty());

    let Some(mut projects) = fetched else {
        let value = prompt_line("Default project key (optional)", default)?;
        return Ok(if value.is_empty() { None } else { Some(value) });
    };
    projects.sort_by(|a, b| a.key.cmp(&b.key));

    loop {
        print_project_list(&projects);
        let input = prompt_line("Project: number, key, or search text (blank to skip)", default)?;
        if input.is_empty() {
            return Ok(None);
        }

        if let Ok(idx) = input.parse::<usize>() {
            if idx >= 1 && idx <= projects.len().min(MAX_LISTED_PROJECTS) {
                return Ok(Some(projects[idx - 1].key.clone()));
            }
        }

        if let Some(p) = projects.iter().find(|p| p.key.eq_ignore_ascii_case(&input)) {
            return Ok(Some(p.key.clone()));
        }

        let needle = input.to_lowercase();
        let matches: Vec<&ProjectSummary> = projects
            .iter()
            .filter(|p| p.key.to_lowercase().contains(&needle) || p.name.to_lowercase().contains(&needle))
            .collect();

        match matches.len() {
            0 => {
                let question = format!("No match for '{input}'. Use it as a custom project key anyway?");
                if prompt_yes_no(&question, false)? {
                    return Ok(Some(input));
                }
            }
            1 => return Ok(Some(matches[0].key.clone())),
            _ => {
                let narrowed: Vec<ProjectSummary> = matches.into_iter().cloned().collect();
                projects = narrowed;
            }
        }
    }
}

fn print_project_list(projects: &[ProjectSummary]) {
    println!("Available projects:");
    let shown = projects.len().min(MAX_LISTED_PROJECTS);
    for (i, p) in projects.iter().take(shown).enumerate() {
        println!("  {:>2}) {} — {}", i + 1, p.key, p.name);
    }
    if projects.len() > shown {
        println!("  ...and {} more — type to search", projects.len() - shown);
    }
}

// ── Prompt helpers ───────────────────────────────────────────────────────────

fn mask(token: &str) -> String {
    if token.is_empty() {
        return String::new();
    }
    if token.len() <= 4 {
        "*".repeat(token.len())
    } else {
        format!("{}{}", "*".repeat(token.len() - 4), &token[token.len() - 4..])
    }
}

/// Reads one line from stdin, trimmed. Errors (rather than looping forever) if stdin has hit
/// EOF — `read_line` reports that as `Ok(0)`, not an `Err`, so callers must check for it.
fn read_line_trimmed() -> Result<String> {
    let mut line = String::new();
    let n = io::stdin().read_line(&mut line).context("Failed to read input")?;
    if n == 0 {
        anyhow::bail!("Input closed — aborting init.");
    }
    Ok(line.trim().to_string())
}

fn prompt_line(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(d) if !d.is_empty() => print!("{label} [{d}]: "),
        _ => print!("{label}: "),
    }
    io::stdout().flush().ok();
    let line = read_line_trimmed()?;
    if line.is_empty() { Ok(default.unwrap_or("").to_string()) } else { Ok(line) }
}

fn prompt_required(label: &str, default: Option<&str>) -> Result<String> {
    loop {
        let value = prompt_line(label, default)?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("  (required)");
    }
}

fn prompt_yes_no(label: &str, default_yes: bool) -> Result<bool> {
    let hint = if default_yes { "Y/n" } else { "y/N" };
    print!("{label} [{hint}]: ");
    io::stdout().flush().ok();
    let line = read_line_trimmed()?;
    Ok(match line.to_lowercase().as_str() {
        "" => default_yes,
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default_yes,
    })
}

fn prompt_choice(label: &str, choices: &[char], default: char) -> Result<char> {
    loop {
        print!("{label}: ");
        io::stdout().flush().ok();
        let line = read_line_trimmed()?.to_lowercase();
        if line.is_empty() {
            return Ok(default);
        }
        if let Some(c) = line.chars().next() {
            if choices.contains(&c) {
                return Ok(c);
            }
        }
        println!("  please enter one of: {}", choices.iter().collect::<String>());
    }
}

/// Reads a token without echoing it to the terminal (so it doesn't end up in scrollback/history).
/// Falls back to `default` on an empty Enter.
fn prompt_masked_required(label: &str, default: Option<&str>) -> Result<String> {
    loop {
        let value = prompt_masked(label, default)?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("  (required)");
    }
}

fn prompt_masked(label: &str, default: Option<&str>) -> Result<String> {
    use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
    use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::IsTerminal;

    // Raw-mode masking needs a real terminal — piped/scripted input (CI, dotfiles bootstrap,
    // etc.) falls back to a plain, visible prompt instead of hard-failing the whole wizard.
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return prompt_line(label, default);
    }

    let hint = if default.is_some() { " [keep existing]" } else { "" };
    print!("{label}{hint}: ");
    io::stdout().flush().ok();

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut value = String::new();
    let outcome: Result<()> = loop {
        match read() {
            Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Enter => break Ok(()),
                KeyCode::Backspace => {
                    if value.pop().is_some() {
                        print!("\u{8} \u{8}");
                        io::stdout().flush().ok();
                    }
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    break Err(anyhow::anyhow!("Cancelled"));
                }
                KeyCode::Char(c) => {
                    value.push(c);
                    print!("*");
                    io::stdout().flush().ok();
                }
                _ => {}
            },
            Ok(_) => {}
            Err(e) => break Err(e.into()),
        }
    };
    disable_raw_mode().ok();
    println!();
    outcome?;

    if value.is_empty() { Ok(default.unwrap_or("").to_string()) } else { Ok(value) }
}
