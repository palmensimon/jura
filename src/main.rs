mod cli;
mod config;
mod git;
mod jira;
mod mcp;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jura", about = "Jira terminal client with git and Claude integration")]
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
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init) => {
            config::write_example_config()?;
            let config_path = config::config_dir().join("config.yaml");
            println!("Edit {} with your Jira credentials, then run jura.", config_path.display());
        }
        Some(Command::Tickets) => {
            cli::cmd_tickets();
        }
        Some(Command::Ticket { key }) => {
            cli::cmd_ticket(&key);
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
