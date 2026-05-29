use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub jira: JiraConfig,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraConfig {
    pub base_url: String,
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Defaults {
    pub project: Option<String>,
    #[allow(dead_code)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Legacy single-status filter; superseded by default_filter.statuses
    #[serde(default)]
    pub status_filter: Option<String>,
    #[serde(default)]
    pub assigned_to_me: bool,
    #[serde(default)]
    pub assign_on_checkout: bool,
    #[serde(default = "default_true")]
    pub hide_done: bool,
    #[serde(default)]
    pub default_filter: DefaultFilter,
    /// Only show these statuses in the filter panel (empty = show all)
    #[serde(default)]
    pub visible_statuses: Vec<String>,
    /// Only show these components in the filter panel (empty = show all)
    #[serde(default)]
    pub visible_components: Vec<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DefaultFilter {
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub sprint_active_only: bool,
    #[serde(default)]
    pub sort_by: String,
    #[serde(default)]
    pub sort_dir: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Templates {
    #[serde(default)]
    pub templates: Vec<TicketTemplate>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TicketTemplate {
    pub name: String,
    pub project: String,
    pub issue_type: String,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default, alias = "epic")]
    pub epic_link: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
}

/// Holds the shareable, team-wide defaults. Serialized as `user_defaults.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SettingsFile {
    #[serde(default)]
    pub defaults: Defaults,
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("jura")
}

pub fn user_defaults_path() -> PathBuf {
    config_dir().join("user_defaults.yaml")
}

/// Load credentials from `config.yaml`. If `user_defaults.yaml` exists its
/// `defaults` section wins; otherwise the `defaults` block inside `config.yaml`
/// is used as a backward-compatible fallback.
pub fn load_config() -> Result<Config> {
    let path = config_dir().join("config.yaml");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read config at {}", path.display()))?;
    let mut config: Config = serde_yaml::from_str(&content).context("Failed to parse config.yaml")?;

    if let Some(sf) = load_user_defaults() {
        config.defaults = sf.defaults;
    }

    Ok(config)
}

/// Load `user_defaults.yaml`. Returns `None` if the file doesn't exist yet.
pub fn load_user_defaults() -> Option<SettingsFile> {
    let path = user_defaults_path();
    if !path.exists() { return None; }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Persist only the Jira credentials to `config.yaml`.
/// Defaults live in `user_defaults.yaml` and are not written here.
pub fn save_config(config: &Config) -> Result<()> {
    #[derive(Serialize)]
    struct CredFile<'a> { jira: &'a JiraConfig }
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.yaml");
    let yaml = serde_yaml::to_string(&CredFile { jira: &config.jira })
        .context("Failed to serialize config")?;
    std::fs::write(&path, yaml)
        .with_context(|| format!("Failed to write config to {}", path.display()))
}

/// Persist defaults/preferences to `user_defaults.yaml`.
pub fn save_settings(defaults: &Defaults) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let file = SettingsFile { defaults: defaults.clone() };
    let yaml = serde_yaml::to_string(&file).context("Failed to serialize user_defaults")?;
    std::fs::write(user_defaults_path(), yaml).context("Failed to write user_defaults.yaml")
}

pub fn load_templates() -> Result<Templates> {
    let path = config_dir().join("templates.yaml");
    if !path.exists() {
        return Ok(Templates::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    serde_yaml::from_str(&content).context("Failed to parse templates.yaml")
}

pub fn write_example_config() -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;

    let config_path = dir.join("config.yaml");
    if !config_path.exists() {
        std::fs::write(
            &config_path,
            r#"jira:
  base_url: "https://jira.yourcompany.com"
  token: "your-personal-access-token"
"#,
        )?;
        eprintln!("Created example config at {}", config_path.display());
    }

    let user_defaults_path = dir.join("user_defaults.yaml");
    if !user_defaults_path.exists() {
        std::fs::write(
            &user_defaults_path,
            r#"defaults:
  # Jira project key to filter by default (e.g. "PROJ")
  project: "PROJ"

  # Maximum number of tickets to fetch
  max_results: 50

  # Hide tickets with status Done/Closed/Resolved
  hide_done: true

  # Only show tickets assigned to you by default
  assigned_to_me: false

  # Automatically assign yourself when checking out a branch
  assign_on_checkout: false

  # Restrict which statuses appear in the filter panel (empty = show all)
  visible_statuses: []

  # Restrict which components appear in the filter panel (empty = show all)
  visible_components: []

  # Default filter applied on startup
  default_filter:
    statuses: []
    component: ~
    labels: []
    team: ~
    sprint_active_only: false
    sort_by: "updated"   # updated | created | priority
    sort_dir: "desc"     # desc | asc
"#,
        )?;
        eprintln!("Created example user_defaults at {}", user_defaults_path.display());
    }

    let templates_path = dir.join("templates.yaml");
    if !templates_path.exists() {
        std::fs::write(
            &templates_path,
            r#"templates:
  - name: "Feature"
    project: "PROJ"
    issue_type: "Story"
    component: "frontend"       # optional
    epic: "PROJ-1"              # optional — links to epic via customfield_10014
    team: "42"                  # optional — customfield_10001 (plain string team id)
    assignee: "user@example.com" # optional — Jira username / email
    labels: ["frontend"]        # optional
    priority: "Medium"          # optional

  - name: "Bug Fix"
    project: "PROJ"
    issue_type: "Bug"
    priority: "High"
"#,
        )?;
        eprintln!("Created example templates at {}", templates_path.display());
    }

    Ok(())
}
