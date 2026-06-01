mod cli;
mod config;
mod git;
mod jira;
mod cache;
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
    /// Write example config files to ~/.config/jura/
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
            config::write_example_config()?;
            let dir = config::config_dir();
            let home = dirs::home_dir().unwrap_or_default();
            let display = dir.strip_prefix(&home)
                .map(|p| format!("~/{}", p.display()))
                .unwrap_or_else(|_| dir.display().to_string());
            println!("Config directory: {display}\n");
            println!("  config.yaml          your Jira credentials (edit this first)");
            println!("  user_defaults.yaml   preferences and filters");
            println!("  templates.yaml       create-ticket templates\n");
            println!("Next steps:");
            println!("  1. Edit {display}/config.yaml with your base_url and token");
            println!("  2. Run `jura` to open the TUI");
            println!("  3. Run `jura install-skill` to set up the AI skill");
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
            let cfg = match config::load_config() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Config error: {e}");
                    eprintln!("Run `jura init` to create example config files.");
                    std::process::exit(1);
                }
            };
            let templates = config::load_templates().unwrap_or_default();
            let client = jira::JiraClient::new(&cfg.jira)?;
            tui::run_tui(cfg, templates, client).await?;
        }
    }

    Ok(())
}
