mod cli;
mod config;
mod git;
mod jira;
mod cache;
mod markdown;
mod init;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jura", about = "Jira terminal client with git and AI integration")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Interactively configure Jira credentials, default project, and optional extras
    Init,
    /// List all Jira tickets assigned to me (from local cache)
    Tickets,
    /// Show full details for a specific ticket key (e.g. PROJ-123)
    Ticket {
        key: String,
    },
    /// Show full details for the ticket linked to the current git branch
    Current,
    /// Write the jura-cli.skill file for use with your AI agent
    InstallSkill {
        /// Parent directory to create jura-cli/ in (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init) => {
            let existing = config::load_config().ok();
            match init::run_wizard(existing).await? {
                Some(_) => {
                    println!();
                    println!("Run `jura` to open the TUI.");
                }
                None => println!("Init cancelled — no changes made."),
            }
        }
        Some(Command::Tickets) => {
            cli::cmd_tickets();
        }
        Some(Command::Ticket { key }) => {
            cli::cmd_ticket(&key);
        }
        Some(Command::Current) => {
            cli::cmd_current();
        }
        Some(Command::InstallSkill { path }) => {
            cli::cmd_install_skill(path.as_deref());
        }
        None => {
            let config_path = config::config_dir().join("config.yaml");
            let cfg = if !config_path.exists() {
                match init::run_wizard(None).await? {
                    Some(cfg) => cfg,
                    None => {
                        eprintln!("Init cancelled — run `jura init` to try again.");
                        std::process::exit(1);
                    }
                }
            } else {
                match config::load_config() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Config error: {e}");
                        eprintln!("Run `jura init` to reconfigure.");
                        std::process::exit(1);
                    }
                }
            };
            let templates = config::load_templates().unwrap_or_default();
            let client = jira::JiraClient::new(&cfg.jira)?;
            tui::run_tui(cfg, templates, client).await?;
        }
    }

    Ok(())
}
