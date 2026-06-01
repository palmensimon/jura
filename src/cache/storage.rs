use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::jira::{Issue, Transition};
use crate::config::config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCache {
    pub jql: String,
    pub issues: Vec<Issue>,
}

pub fn issue_cache_path() -> PathBuf {
    config_dir().join("issue_cache.json")
}

pub fn load_issue_cache() -> Option<IssueCache> {
    let path = issue_cache_path();
    if !path.exists() { return None; }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_issue_cache(jql: &str, issues: &[Issue]) {
    let cache = IssueCache { jql: jql.to_string(), issues: issues.to_vec() };
    if let Ok(content) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(issue_cache_path(), content);
    }
}

pub fn mine_cache_path() -> PathBuf {
    config_dir().join("mine_cache.json")
}

pub fn load_mine_cache() -> Option<IssueCache> {
    let path = mine_cache_path();
    if !path.exists() { return None; }
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_mine_cache(jql: &str, issues: &[Issue]) {
    let cache = IssueCache { jql: jql.to_string(), issues: issues.to_vec() };
    if let Ok(content) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(mine_cache_path(), content);
    }
}

pub fn transition_cache_path() -> PathBuf {
    config_dir().join("transition_cache.json")
}

pub fn load_transition_cache(issue_key: &str) -> Option<Vec<Transition>> {
    let path = transition_cache_path();
    if !path.exists() { return None; }
    let content = fs::read_to_string(&path).ok()?;
    let map: HashMap<String, Vec<Transition>> = serde_json::from_str(&content).ok()?;
    map.get(issue_key).cloned()
}

pub fn save_transition_cache(issue_key: &str, transitions: &[Transition]) {
    let path = transition_cache_path();
    let mut map: HashMap<String, Vec<Transition>> = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    map.insert(issue_key.to_string(), transitions.to_vec());
    if let Ok(content) = serde_json::to_string_pretty(&map) {
        let _ = fs::write(path, content);
    }
}
