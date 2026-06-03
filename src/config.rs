use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub jira: JiraConfig,
    #[serde(default)]
    pub board_id: Option<u64>,
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
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub assign_on_checkout: bool,
    /// Statuses excluded from results when no explicit status filter is active (empty = hide nothing)
    #[serde(default = "default_hidden_statuses")]
    pub hidden_statuses: Vec<String>,
    #[serde(default)]
    pub default_filter: DefaultFilter,
    /// Only show these statuses in the filter panel (empty = show all)
    #[serde(default)]
    pub visible_statuses: Vec<String>,
    /// Only show these components in the filter panel (empty = show all)
    #[serde(default)]
    pub visible_components: Vec<String>,
    /// Labels available as options in the filter panel (empty = text input not shown)
    #[serde(default)]
    pub visible_labels: Vec<String>,
    /// Teams available as options in the filter panel (empty = no team filter shown)
    #[serde(default)]
    pub visible_teams: Vec<TeamEntry>,
    /// Statuses that trigger auto-assignment to the active sprint on transition (empty = disabled)
    #[serde(default)]
    pub sprint_on_transition: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TeamEntry {
    pub id: String,
    pub name: String,
}

fn default_hidden_statuses() -> Vec<String> {
    vec!["Done".to_string(), "Closed".to_string(), "Resolved".to_string()]
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DefaultFilter {
    #[serde(default)]
    pub statuses: Vec<String>,
    #[serde(default)]
    pub hidden_statuses: Vec<String>,
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
    #[serde(default)]
    pub fix_version: Option<String>,
}

/// Holds user-specific settings. Serialized as `user_settings.yaml`.
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

pub fn user_settings_path() -> PathBuf {
    config_dir().join("user_settings.yaml")
}

/// Load credentials from `config.yaml`. If `user_settings.yaml` exists its
/// `defaults` section wins; otherwise the `defaults` block inside `config.yaml`
/// is used as a backward-compatible fallback.
pub fn load_config() -> Result<Config> {
    let path = config_dir().join("config.yaml");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Could not read config at {}", path.display()))?;
    let mut config: Config = serde_yaml::from_str(&content).context("Failed to parse config.yaml")?;

    if let Some(sf) = load_user_settings() {
        config.defaults = sf.defaults;
    }

    Ok(config)
}

/// Load `user_settings.yaml`. Returns `None` if the file doesn't exist yet.
pub fn load_user_settings() -> Option<SettingsFile> {
    let path = user_settings_path();
    if !path.exists() { return None; }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Persist only the Jira credentials to `config.yaml`.
/// Settings live in `user_settings.yaml` and are not written here.
pub fn save_config(config: &Config) -> Result<()> {
    #[derive(Serialize)]
    struct CredFile<'a> {
        jira: &'a JiraConfig,
        #[serde(skip_serializing_if = "Option::is_none")]
        board_id: Option<u64>,
    }
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.yaml");
    let yaml = serde_yaml::to_string(&CredFile { jira: &config.jira, board_id: config.board_id })
        .context("Failed to serialize config")?;
    std::fs::write(&path, yaml)
        .with_context(|| format!("Failed to write config to {}", path.display()))
}

/// Persist defaults/preferences to `user_settings.yaml`.
pub fn save_settings(defaults: &Defaults) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let file = SettingsFile { defaults: defaults.clone() };
    let yaml = serde_yaml::to_string(&file).context("Failed to serialize user_settings")?;
    std::fs::write(user_settings_path(), yaml).context("Failed to write user_settings.yaml")
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
    }

    let user_settings_path = dir.join("user_settings.yaml");
    if !user_settings_path.exists() {
        std::fs::write(
            &user_settings_path,
            r#"defaults:
  # Jira project key to filter by default (e.g. "PROJ")
  project: ~

  # Maximum number of tickets to fetch
  max_results: 50

  # Automatically assign yourself when checking out a branch
  assign_on_checkout: false

  # Statuses hidden from results when no explicit status filter is active (empty = hide nothing)
  # These also appear as toggleable options in the filter panel [3] Hidden statuses row.
  hidden_statuses: []

  # Restrict which statuses appear in the [2] Status filter row (empty = show all)
  visible_statuses: []

  # Restrict which components appear in the [4] Component filter row (empty = show all)
  visible_components: []

  # Labels shown as selectable options in the [5] Labels filter row (empty = row hidden)
  visible_labels: []

  # Teams shown as selectable options in the [6] Team filter row (empty = row hidden)
  # Each entry needs an id (the value sent to Jira) and a name (shown in the UI).
  visible_teams: []

  # Statuses that trigger auto-assignment to the active sprint when transitioning a ticket.
  # Leave empty (default) to disable. Example: ["To Do", "In Progress", "In Review"]
  sprint_on_transition: []

  # Default filter applied on startup
  default_filter:
    statuses: []
    component: ~
    labels: []
    team: ~
    sprint_active_only: false
    sort_by: "updated"   # updated | created | priority
    sort_dir: "desc"     # desc | asc

# ── Example filled configuration ─────────────────────────────────────────────
#
# defaults:
#   project: "SWISH"
#   max_results: 200
#   hidden_statuses:
#     - "Won't Do"
#     - "Done"
#   assign_on_checkout: true
#   visible_statuses:
#     - "Backlog"
#     - "To Do"
#     - "In Progress"
#     - "Review"
#     - "Done"
#   visible_components:
#     - "private-app-android"
#     - "private-app-ios"
#   visible_labels:
#     - "bug"
#     - "frontend"
#     - "backend"
#     - "tech-debt"
#   visible_teams:
#     - id: "49"
#       name: "Mobile"
#     - id: "12"
#       name: "Platform"
#   default_filter:
#     statuses: []
#     component: ~
#     labels: []
#     team: "49"
#     sprint_active_only: true
#     sort_by: "updated"
#     sort_dir: "desc"
"#,
        )?;
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
    }

    Ok(())
}
