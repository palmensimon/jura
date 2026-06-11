use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{
    config::{Config, TicketTemplate},
    jira::{Issue, JiraClient, Transition},
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Tab {
    #[default]
    All,
    Mine,
}

impl Tab {
    pub fn next(self) -> Self {
        match self {
            Tab::All => Tab::Mine,
            Tab::Mine => Tab::All,
        }
    }
    pub fn prev(self) -> Self {
        self.next()
    }
    pub fn label(self) -> &'static str {
        match self {
            Tab::All => "All",
            Tab::Mine => "Mine",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TabState {
    pub issues: Vec<Issue>,
    pub loading: bool,
    pub local_search: String,
    pub local_search_active: bool,
    pub selected_row: usize,
}

impl TabState {
    pub fn visible_issues(&self) -> Vec<&Issue> {
        if self.local_search.is_empty() {
            self.issues.iter().collect()
        } else {
            let q = self.local_search.to_lowercase();
            self.issues.iter().filter(|i| issue_matches(i, &q)).collect()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SortBy {
    #[default]
    Updated,
    Created,
    Priority,
}

impl SortBy {
    pub fn as_str(self) -> &'static str {
        match self {
            SortBy::Updated => "updated",
            SortBy::Created => "created",
            SortBy::Priority => "priority",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "created" => SortBy::Created,
            "priority" => SortBy::Priority,
            _ => SortBy::Updated,
        }
    }
    pub fn next(self) -> Self {
        match self {
            SortBy::Updated => SortBy::Created,
            SortBy::Created => SortBy::Priority,
            SortBy::Priority => SortBy::Updated,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SortBy::Updated => "Updated",
            SortBy::Created => "Created",
            SortBy::Priority => "Priority",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SortDir {
    #[default]
    Desc,
    Asc,
}

impl SortDir {
    pub fn as_str(self) -> &'static str {
        match self {
            SortDir::Desc => "desc",
            SortDir::Asc => "asc",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "asc" => SortDir::Asc,
            _ => SortDir::Desc,
        }
    }
    pub fn next(self) -> Self {
        match self {
            SortDir::Desc => SortDir::Asc,
            SortDir::Asc => SortDir::Desc,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SortDir::Desc => "Desc ▾",
            SortDir::Asc => "Asc ▴",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppView {
    TicketList,
    TicketDetail { issue: Box<Issue> },
    TransitionPicker { issue: Box<Issue> },
    TemplatesPanel,
    CreateTicket,
    Settings,
    FilterPanel,
    TicketSearch,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    IssuesLoaded(Result<Vec<Issue>, String>, String, Tab),
    BranchCreated(String),
    TicketCreated(String),
    ConfigSaved(Config),
    FilterSaved(Config),
    FilterOptions { statuses: Vec<String>, components: Vec<String> },
    TransitionsLoaded(Vec<Transition>, String),
    TransitionApplied(String),
    IssueReloaded(Issue),
    UserLoaded(String),
    AssignmentChanged(Issue),
    TicketFound(Issue),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct FilterState {
    pub project: Option<String>,
    pub component: Option<String>,
    pub selected_statuses: Vec<String>,
    pub hidden_statuses: Vec<String>,
    pub text_search: String,
    pub labels: Vec<String>,
    pub team: Option<String>,
    pub epic: Option<String>,
    pub sprint_active_only: bool,
    pub assigned_to_me: bool,
    pub sort_by: SortBy,
    pub sort_dir: SortDir,
}

impl FilterState {
    pub fn build_jql(&self, config: &Config) -> String {
        let sort_col = self.sort_by.as_str();
        let sort_dir = match self.sort_dir {
            SortDir::Desc => "DESC",
            SortDir::Asc => "ASC",
        };
        let order = format!("ORDER BY {sort_col} {sort_dir}");
        let mut conditions = vec![];

        if let Some(proj) = self.project.as_deref().or(config.project.as_deref()) {
            conditions.push(format!("project = {proj}"));
        }
        if let Some(comp) = &self.component {
            if !comp.is_empty() {
                conditions.push(format!("component = \"{comp}\""));
            }
        }
        if !self.selected_statuses.is_empty() {
            let joined = self.selected_statuses
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",");
            conditions.push(format!("status in ({joined})"));
        } else if !self.hidden_statuses.is_empty() {
            let joined = self.hidden_statuses
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",");
            conditions.push(format!("status not in ({joined})"));
        }
        if self.assigned_to_me {
            conditions.push("assignee = currentUser()".to_string());
        }
        if self.sprint_active_only {
            conditions.push("sprint in openSprints()".to_string());
        }
        if let Some(team) = &self.team {
            if !team.is_empty() {
                if team.chars().all(|c| c.is_ascii_digit()) {
                    conditions.push(format!("Team = {team}"));
                } else {
                    conditions.push(format!("Team = \"{team}\""));
                }
            }
        }
        if let Some(epic) = &self.epic {
            if !epic.is_empty() {
                conditions.push(format!("\"Epic Link\" = \"{epic}\""));
            }
        }
        if !self.labels.is_empty() {
            let joined = self.labels
                .iter()
                .map(|l| format!("\"{l}\""))
                .collect::<Vec<_>>()
                .join(",");
            conditions.push(format!("labels in ({joined})"));
        }
        if !self.text_search.is_empty() {
            conditions.push(format!("summary ~ \"{}\"", self.text_search));
        }

        if conditions.is_empty() {
            order
        } else {
            format!("{} {order}", conditions.join(" AND "))
        }
    }

    pub fn from_config(config: &Config) -> Self {
        let df = &config.defaults.default_filter;
        let selected_statuses = df.statuses.clone();

        let hidden_statuses = if !df.hidden_statuses.is_empty() {
            df.hidden_statuses.clone()
        } else {
            config.defaults.hidden_statuses.clone()
        };

        Self {
            project: config.project.clone(),
            component: df.component.clone(),
            selected_statuses,
            hidden_statuses,
            text_search: String::new(),
            labels: df.labels.clone(),
            team: df.team.clone(),
            epic: df.epic.clone(),
            sprint_active_only: df.sprint_active_only,
            assigned_to_me: false,
            sort_by: SortBy::from_str(&df.sort_by),
            sort_dir: SortDir::from_str(&df.sort_dir),
        }
    }
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            project: None,
            component: None,
            selected_statuses: vec![],
            hidden_statuses: vec!["Done".to_string(), "Closed".to_string(), "Resolved".to_string()],
            text_search: String::new(),
            labels: vec![],
            team: None,
            epic: None,
            sprint_active_only: false,
            assigned_to_me: false,
            sort_by: SortBy::default(),
            sort_dir: SortDir::default(),
        }
    }
}

pub struct App {
    pub view: AppView,
    pub tab: Tab,
    pub all: TabState,
    pub mine: TabState,
    pub error: Option<String>,
    pub status_msg: Option<String>,
    pub filter: FilterState,
    pub current_branch_key: Option<String>,
    pub current_branch_name: Option<String>,
    pub current_user_name: Option<String>,
    pub config: Arc<Config>,
    pub templates: Vec<TicketTemplate>,
    pub client: Arc<JiraClient>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub available_statuses: Vec<String>,
    pub all_statuses: Vec<String>,
    pub hidden_status_options: Vec<String>,
    pub available_components: Vec<String>,
    pub available_transitions: Vec<Transition>,
    pub show_help: bool,
    pub help_scroll: u16,
}

impl App {
    pub fn new(
        config: Config,
        templates: Vec<TicketTemplate>,
        client: JiraClient,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        let filter = FilterState::from_config(&config);
        let current_branch_name = crate::git::current_branch().ok();
        let current_branch_key = current_branch_name.as_deref()
            .and_then(crate::git::extract_ticket_key);

        Self {
            view: AppView::TicketList,
            tab: Tab::default(),
            all: TabState::default(),
            mine: TabState::default(),
            error: None,
            status_msg: None,
            filter,
            current_branch_key,
            current_branch_name,
            current_user_name: None,
            config: Arc::new(config),
            templates,
            client: Arc::new(client),
            event_tx,
            available_statuses: vec![],
            all_statuses: vec![],
            hidden_status_options: vec![],
            available_components: vec![],
            available_transitions: vec![],
            show_help: false,
            help_scroll: 0,
        }
    }

    pub fn active_tab(&self) -> &TabState {
        match self.tab {
            Tab::All => &self.all,
            Tab::Mine => &self.mine,
        }
    }

    pub fn active_tab_mut(&mut self) -> &mut TabState {
        match self.tab {
            Tab::All => &mut self.all,
            Tab::Mine => &mut self.mine,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::IssuesLoaded(Ok(issues), jql, tab) => {
                let ts = match tab {
                    Tab::All => {
                        crate::cache::storage::save_issue_cache(&jql, &issues);
                        &mut self.all
                    }
                    Tab::Mine => {
                        crate::cache::storage::save_mine_cache(&jql, &issues);
                        &mut self.mine
                    }
                };
                ts.loading = false;
                ts.issues = issues;
                ts.selected_row = 0;
            }
            AppEvent::IssuesLoaded(Err(e), _, _) => {
                self.all.loading = false;
                self.mine.loading = false;
                self.error = Some(e);
            }
            AppEvent::BranchCreated(branch) => {
                self.current_branch_key = crate::git::extract_ticket_key(&branch);
                self.current_branch_name = Some(branch.clone());
                self.status_msg = Some(format!("Switched to '{branch}'"));
            }
            AppEvent::TicketCreated(key) => {
                self.status_msg = Some(format!("Created {key}"));
                self.view = AppView::TicketList;
                self.trigger_load();
            }
            AppEvent::ConfigSaved(new_cfg) => {
                match JiraClient::new(&new_cfg.jira) {
                    Ok(client) => {
                        self.client = Arc::new(client);
                        self.filter = FilterState::from_config(&new_cfg);
                        self.config = Arc::new(new_cfg);
                        self.view = AppView::TicketList;
                        self.status_msg = Some("Settings saved".to_string());
                        self.trigger_load_tab(Tab::All);
                        self.trigger_load_tab(Tab::Mine);
                    }
                    Err(e) => {
                        self.error = Some(format!("Bad credentials: {e:#}"));
                    }
                }
            }
            AppEvent::FilterSaved(new_cfg) => {
                self.config = Arc::new(new_cfg);
                self.status_msg = Some("Filter saved".to_string());
            }
            AppEvent::FilterOptions { statuses, components } => {
                let vs = &self.config.defaults.visible_statuses;
                self.available_statuses = if vs.is_empty() {
                    statuses.clone()
                } else {
                    statuses.iter().filter(|s| vs.contains(s)).cloned().collect()
                };
                self.hidden_status_options = statuses.iter()
                    .filter(|s| self.config.defaults.hidden_statuses.contains(s))
                    .cloned()
                    .collect();
                self.all_statuses = statuses;
                let vc = &self.config.defaults.visible_components;
                self.available_components = if vc.is_empty() {
                    components
                } else {
                    components.into_iter().filter(|c| vc.contains(c)).collect()
                };
            }
            AppEvent::TransitionsLoaded(transitions, issue_key) => {
                crate::cache::storage::save_transition_cache(&issue_key, &transitions);
                self.available_transitions = transitions;
            }
            AppEvent::TransitionApplied(_key) => {
                self.status_msg = Some("Status updated".to_string());
                self.trigger_load();
                // View change is handled by mod.rs based on transition_state.return_to_list
            }
            AppEvent::IssueReloaded(issue) => {
                if matches!(self.view, AppView::TicketDetail { .. }) {
                    self.view = AppView::TicketDetail { issue: Box::new(issue) };
                }
            }
            AppEvent::UserLoaded(name) => {
                self.current_user_name = Some(name);
            }
            AppEvent::AssignmentChanged(issue) => {
                let assigned_to_me = self.current_user_name.as_deref()
                    .map(|me| issue.fields.assignee.as_ref()
                        .and_then(|u| u.name.as_deref())
                        .map(|n| n == me)
                        .unwrap_or(false))
                    .unwrap_or(false);
                self.status_msg = Some(if assigned_to_me {
                    "Assigned to you".to_string()
                } else {
                    "Unassigned".to_string()
                });
                if matches!(self.view, AppView::TicketDetail { .. }) {
                    self.view = AppView::TicketDetail { issue: Box::new(issue) };
                }
                self.trigger_load();
            }
            AppEvent::TicketFound(issue) => {
                self.view = AppView::TicketDetail { issue: Box::new(issue) };
            }
            AppEvent::Error(e) => {
                self.error = Some(e);
            }
        }
    }

    pub fn mine_jql(&self) -> String {
        let mut conditions = vec!["assignee = currentUser()".to_string()];
        if !self.filter.hidden_statuses.is_empty() {
            let joined = self.filter.hidden_statuses
                .iter()
                .map(|s| format!("\"{s}\""))
                .collect::<Vec<_>>()
                .join(",");
            conditions.push(format!("status not in ({joined})"));
        }
        format!("{} ORDER BY updated DESC", conditions.join(" AND "))
    }

    pub fn trigger_load(&mut self) {
        let branch = crate::git::current_branch().ok();
        self.current_branch_key = branch.as_deref().and_then(crate::git::extract_ticket_key);
        self.current_branch_name = branch;
        self.trigger_load_tab(self.tab);
    }

    pub fn trigger_load_tab(&mut self, tab: Tab) {
        let jql = match tab {
            Tab::All => self.filter.build_jql(&self.config),
            Tab::Mine => self.mine_jql(),
        };

        {
            let ts = match tab {
                Tab::All => &mut self.all,
                Tab::Mine => &mut self.mine,
            };
            ts.loading = true;
            ts.local_search.clear();
            ts.local_search_active = false;
        }
        self.error = None;

        // Pre-populate from cache
        let cached = match tab {
            Tab::All => crate::cache::storage::load_issue_cache(),
            Tab::Mine => crate::cache::storage::load_mine_cache(),
        };
        if let Some(cache) = cached {
            if cache.jql == jql && !cache.issues.is_empty() {
                let ts = match tab {
                    Tab::All => &mut self.all,
                    Tab::Mine => &mut self.mine,
                };
                ts.issues = cache.issues;
                ts.selected_row = 0;
            }
        }

        let client = self.client.clone();
        let max = self.config.defaults.max_results.unwrap_or(50);
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            let result = client.search_issues(&jql, max).await;
            let _ = tx
                .send(match result {
                    Ok(r) => AppEvent::IssuesLoaded(Ok(r.issues), jql, tab),
                    Err(e) => AppEvent::IssuesLoaded(Err(format!("{e:#}")), jql, tab),
                })
                .await;
        });
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        if self.tab == tab { return; }
        self.tab = tab;
        if tab == Tab::Mine && self.mine.issues.is_empty() && !self.mine.loading {
            self.trigger_load_tab(Tab::Mine);
        }
    }

    pub fn selected_issue(&self) -> Option<&Issue> {
        let ts = self.active_tab();
        ts.visible_issues().get(ts.selected_row).copied()
    }

    pub fn spawn_checkout(&self, branch: String, issue: &Issue) {
        let should_assign = self.config.defaults.assign_on_checkout
            && self.current_user_name.is_some()
            && issue.fields.assignee.as_ref()
                .and_then(|u| u.name.as_deref())
                != self.current_user_name.as_deref();
        let current_user = self.current_user_name.clone();
        let key_str = issue.key.clone();
        let client = self.client.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            let b = branch.clone();
            match tokio::task::spawn_blocking(move || crate::git::checkout_branch(&b))
                .await
                .unwrap_or_else(|e| Err(anyhow::anyhow!("{e}")))
            {
                Err(e) => { let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await; }
                Ok(()) => {
                    let _ = tx.send(AppEvent::BranchCreated(branch)).await;
                    if should_assign {
                        if let Some(name) = current_user {
                            if let Ok(()) = client.assign_issue(&key_str, Some(&name)).await {
                                if let Ok(reloaded) = client.get_issue(&key_str).await {
                                    let _ = tx.send(AppEvent::AssignmentChanged(reloaded)).await;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn reload_issue(&mut self, key: String) {
        self.status_msg = Some("Refreshing…".to_string());
        self.error = None;
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            match client.get_issue(&key).await {
                Ok(issue) => { let _ = tx.send(AppEvent::IssueReloaded(issue)).await; }
                Err(e) => { let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await; }
            }
        });
    }

    pub fn toggle_assignment(&mut self, issue: &Issue) {
        let Some(me) = self.current_user_name.clone() else {
            self.error = Some("Current user not loaded yet".to_string());
            return;
        };
        let is_assigned = issue.fields.assignee.as_ref()
            .and_then(|u| u.name.as_deref())
            .map(|n| n == me.as_str())
            .unwrap_or(false);
        let username: Option<String> = if is_assigned { None } else { Some(me) };
        self.status_msg = Some(if username.is_some() { "Assigning…".to_string() } else { "Unassigning…".to_string() });
        let key_str = issue.key.clone();
        let client = self.client.clone();
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            match client.assign_issue(&key_str, username.as_deref()).await {
                Err(e) => { let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await; }
                Ok(()) => match client.get_issue(&key_str).await {
                    Ok(issue) => { let _ = tx.send(AppEvent::AssignmentChanged(issue)).await; }
                    Err(e) => { let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await; }
                }
            }
        });
    }

    pub fn move_selection_up(&mut self) {
        let ts = self.active_tab_mut();
        if ts.selected_row > 0 {
            ts.selected_row -= 1;
        }
    }

    pub fn move_selection_down(&mut self) {
        let ts = self.active_tab_mut();
        let len = ts.visible_issues().len();
        if ts.selected_row + 1 < len {
            ts.selected_row += 1;
        }
    }
}

fn issue_matches(issue: &Issue, q: &str) -> bool {
    let key = issue.key.to_lowercase();
    let summary = issue.summary().to_lowercase();
    let issue_type = issue.issue_type().to_lowercase();
    let assignee = issue.assignee().to_lowercase();
    let status = issue.status().to_lowercase();
    let components: Vec<String> = issue.component_names().iter().map(|c| c.to_lowercase()).collect();

    q.split_whitespace().all(|token| {
        key.contains(token)
            || summary.contains(token)
            || issue_type.contains(token)
            || assignee.contains(token)
            || status.contains(token)
            || components.iter().any(|c| c.contains(token))
    })
}
