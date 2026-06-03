#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Issue {
    pub id: String,
    pub key: String,
    pub fields: IssueFields,
}

impl Issue {
    pub fn summary(&self) -> &str {
        &self.fields.summary
    }

    pub fn status(&self) -> &str {
        self.fields
            .status
            .as_ref()
            .map(|s| s.name.as_str())
            .unwrap_or("")
    }

    pub fn issue_type(&self) -> &str {
        self.fields
            .issuetype
            .as_ref()
            .map(|t| t.name.as_str())
            .unwrap_or("")
    }

    pub fn priority(&self) -> &str {
        self.fields
            .priority
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("")
    }

    pub fn assignee(&self) -> &str {
        self.fields
            .assignee
            .as_ref()
            .and_then(|a| a.display_name.as_deref())
            .unwrap_or("Unassigned")
    }

    pub fn component_names(&self) -> Vec<&str> {
        self.fields
            .components
            .iter()
            .map(|c| c.name.as_str())
            .collect()
    }

    pub fn description_text(&self) -> Option<&str> {
        self.fields.description.as_deref()
    }

    pub fn fix_version_names(&self) -> Vec<&str> {
        self.fields.fix_versions.iter().map(|v| v.name.as_str()).collect()
    }

    pub fn sprint_name(&self) -> Option<String> {
        let val = self.fields.customfield_10020.as_ref()?;
        if let Some(arr) = val.as_array() {
            let sprint = arr
                .iter()
                .find(|s| {
                    s.get("state")
                        .and_then(|v| v.as_str())
                        .map(|state| {
                            state.eq_ignore_ascii_case("active")
                                || state.eq_ignore_ascii_case("started")
                        })
                        .unwrap_or(false)
                })
                .or_else(|| arr.last())?;
            return sprint
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
        // Legacy Jira Server string format: "...name=Sprint 1,..."
        if let Some(s) = val.as_str() {
            if let Some(pos) = s.find("name=") {
                let rest = &s[pos + 5..];
                let end = rest.find(',').unwrap_or(rest.len());
                return Some(rest[..end].to_string());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueFields {
    pub summary: String,
    pub description: Option<String>,
    pub status: Option<Status>,
    pub issuetype: Option<IssueType>,
    pub priority: Option<Priority>,
    pub assignee: Option<User>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub parent: Option<Parent>,
    #[serde(rename = "customfield_10020", default)]
    pub customfield_10020: Option<serde_json::Value>,
    #[serde(rename = "fixVersions", default)]
    pub fix_versions: Vec<FixVersion>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Status {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueType {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Priority {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixVersion {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "emailAddress")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Component {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parent {
    pub key: String,
    pub fields: Option<ParentFields>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParentFields {
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResult {
    pub issues: Vec<Issue>,
    pub total: u32,
    #[serde(rename = "startAt")]
    pub start_at: u32,
    #[serde(rename = "maxResults")]
    pub max_results: u32,
}


#[derive(Debug, Clone, Serialize)]
pub struct CreateIssueRequest {
    pub fields: CreateIssueFields,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateIssueFields {
    pub project: ProjectRef,
    pub summary: String,
    pub issuetype: NameRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<NameRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<NameRef>,
    #[serde(rename = "customfield_10014", skip_serializing_if = "Option::is_none")]
    pub epic_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<NameRef>,
    #[serde(rename = "customfield_10001", skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(rename = "fixVersions", skip_serializing_if = "Vec::is_empty")]
    pub fix_versions: Vec<NameRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectRef {
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NameRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdRef {
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyRef {
    pub key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateIssueResponse {
    pub id: String,
    pub key: String,
    #[serde(rename = "self")]
    pub self_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentList {
    #[serde(default)]
    pub values: Vec<ProjectComponent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectComponent {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusMeta {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Transition {
    pub id: String,
    pub name: String,
    pub to: TransitionTo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionTo {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionList {
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionRequest {
    pub transition: TransitionRef,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sprint {
    pub id: u64,
    pub name: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SprintList {
    pub values: Vec<Sprint>,
}
