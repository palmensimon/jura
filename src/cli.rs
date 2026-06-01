use std::path::PathBuf;
use serde::Serialize;
use crate::git;
use crate::jira::models::Issue;
use crate::cache::storage::{load_mine_cache, load_issue_cache};

static SKILL_MD: &str = include_str!("../SKILL.md");

#[derive(Serialize)]
struct TicketSummary<'a> {
    key: &'a str,
    summary: &'a str,
    status: &'a str,
    #[serde(rename = "type")]
    issue_type: &'a str,
    priority: &'a str,
    assignee: &'a str,
    checked_out: bool,
}

#[derive(Serialize)]
struct TicketDetail<'a> {
    key: &'a str,
    summary: &'a str,
    description: Option<&'a str>,
    status: &'a str,
    #[serde(rename = "type")]
    issue_type: &'a str,
    priority: &'a str,
    assignee: &'a str,
    components: Vec<&'a str>,
    labels: &'a [String],
    parent: Option<ParentOutput<'a>>,
    sprint: Option<String>,
}

#[derive(Serialize)]
struct ParentOutput<'a> {
    key: &'a str,
    summary: &'a str,
}

fn error_exit(msg: &str) -> ! {
    let obj = serde_json::json!({ "error": msg });
    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    std::process::exit(1);
}

fn print_json<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}

pub fn cmd_tickets() {
    let cache = load_mine_cache()
        .filter(|c| !c.issues.is_empty())
        .unwrap_or_else(|| {
            error_exit("No tickets cached. Open jura TUI to load your assigned tickets.")
        });

    let current_key = git::current_branch().ok()
        .and_then(|b| git::extract_ticket_key(&b));

    let summaries: Vec<TicketSummary> = cache.issues.iter().map(|i| TicketSummary {
        key: &i.key,
        summary: i.summary(),
        status: i.status(),
        issue_type: i.issue_type(),
        priority: i.priority(),
        assignee: i.assignee(),
        checked_out: current_key.as_deref() == Some(i.key.as_str()),
    }).collect();

    print_json(&summaries);
}

pub fn cmd_current() {
    let branch = git::current_branch().unwrap_or_else(|e| {
        error_exit(&format!("Could not determine current branch: {e}"))
    });

    let key = git::extract_ticket_key(&branch).unwrap_or_else(|| {
        error_exit(&format!(
            "No Jira ticket key found in branch name '{branch}'."
        ))
    });

    cmd_ticket(&key);
}

pub fn cmd_ticket(key: &str) {
    let key = key.to_uppercase();

    let issue = find_in_cache(&key).unwrap_or_else(|| {
        error_exit(&format!(
            "Ticket '{key}' not found in local cache. Open jura TUI and load your tickets first."
        ))
    });

    let parent = issue.fields.parent.as_ref().map(|p| ParentOutput {
        key: &p.key,
        summary: p.fields.as_ref()
            .and_then(|f| f.summary.as_deref())
            .unwrap_or(""),
    });

    let detail = TicketDetail {
        key: &issue.key,
        summary: issue.summary(),
        description: issue.description_text(),
        status: issue.status(),
        issue_type: issue.issue_type(),
        priority: issue.priority(),
        assignee: issue.assignee(),
        components: issue.component_names(),
        labels: &issue.fields.labels,
        parent,
        sprint: issue.sprint_name(),
    };

    print_json(&detail);
}

pub fn cmd_install_skill(path: Option<&str>) {
    let parent: PathBuf = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };
    let dir = parent.join("jura-cli");
    let dest = dir.join("SKILL.md");

    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Failed to create directory {}: {e}", dir.display());
        std::process::exit(1);
    }

    if let Err(e) = std::fs::write(&dest, SKILL_MD) {
        eprintln!("Failed to write skill file: {e}");
        std::process::exit(1);
    }

    println!("Skill written to {}", dest.display());
}

fn find_in_cache(key: &str) -> Option<Issue> {
    if let Some(cache) = load_mine_cache() {
        if let Some(issue) = cache.issues.into_iter().find(|i| i.key == key) {
            return Some(issue);
        }
    }
    if let Some(cache) = load_issue_cache() {
        if let Some(issue) = cache.issues.into_iter().find(|i| i.key == key) {
            return Some(issue);
        }
    }
    None
}
