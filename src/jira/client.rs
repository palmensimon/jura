#![allow(dead_code)]
use anyhow::{Context, Result};
use reqwest::{Client, header};

use crate::config::JiraConfig;
use super::models::*;

pub struct JiraClient {
    client: Client,
    base_url: String,
}

impl JiraClient {
    pub fn new(config: &JiraConfig) -> Result<Self> {
        let auth_value = format!("Bearer {}", config.token);

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&auth_value)
                .context("Invalid auth header value")?,
        );
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to build HTTP client")?;

        let base_url = config.base_url.trim_end_matches('/').to_string();
        Ok(Self { client, base_url })
    }

    pub async fn search_issues(&self, jql: &str, max_results: u32) -> Result<SearchResult> {
        let url = format!("{}/rest/api/2/search", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("jql", jql),
                ("maxResults", &max_results.to_string()),
                ("fields", "summary,status,issuetype,priority,assignee,components,labels,parent,description,customfield_10020"),
            ])
            .send()
            .await
            .context("Jira search request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira search returned {status}: {body}");
        }

        resp.json::<SearchResult>().await.context("Failed to parse search response")
    }

    pub async fn get_issue(&self, key: &str) -> Result<Issue> {
        let url = format!("{}/rest/api/2/issue/{key}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("fields", "summary,status,issuetype,priority,assignee,components,labels,parent,description,customfield_10020")])
            .send()
            .await
            .with_context(|| format!("Failed to fetch issue {key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get issue {key} returned {status}: {body}");
        }

        resp.json::<Issue>().await.context("Failed to parse issue response")
    }

    pub async fn create_issue(&self, req: &CreateIssueRequest) -> Result<CreateIssueResponse> {
        let url = format!("{}/rest/api/2/issue", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .context("Failed to create issue")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira create issue returned {status}: {body}");
        }

        resp.json::<CreateIssueResponse>()
            .await
            .context("Failed to parse create issue response")
    }

    pub async fn get_transitions(&self, key: &str) -> Result<Vec<Transition>> {
        let url = format!("{}/rest/api/2/issue/{key}/transitions", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch transitions for {key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get transitions returned {status}: {body}");
        }

        let list: TransitionList = resp.json().await.context("Failed to parse transitions")?;
        Ok(list.transitions)
    }

    pub async fn do_transition(&self, key: &str, transition_id: &str) -> Result<()> {
        let url = format!("{}/rest/api/2/issue/{key}/transitions", self.base_url);
        let body = TransitionRequest {
            transition: TransitionRef { id: transition_id.to_string() },
        };
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to transition {key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira transition {key} returned {status}: {body}");
        }
        Ok(())
    }

    pub async fn get_myself(&self) -> Result<super::models::User> {
        let url = format!("{}/rest/api/2/myself", self.base_url);
        let resp = self.client.get(&url).send().await.context("Failed to fetch current user")?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get current user {status}: {body}");
        }
        resp.json::<super::models::User>().await.context("Failed to parse user response")
    }

    pub async fn assign_issue(&self, key: &str, username: Option<&str>) -> Result<()> {
        let url = format!("{}/rest/api/2/issue/{key}/assignee", self.base_url);
        let body = serde_json::json!({ "name": username.unwrap_or("-1") });
        let resp = self.client.put(&url).json(&body).send().await
            .with_context(|| format!("Failed to assign {key}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira assign {key} returned {status}: {body}");
        }
        Ok(())
    }

    pub async fn get_statuses(&self) -> Result<Vec<String>> {
        let url = format!("{}/rest/api/2/status", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch statuses")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get statuses returned {status}: {body}");
        }

        let metas: Vec<StatusMeta> = resp.json().await.context("Failed to parse statuses")?;
        Ok(metas.into_iter().map(|s| s.name).collect())
    }

    pub async fn assign_issue_to_sprint(&self, sprint_id: u64, issue_key: &str) -> Result<()> {
        let url = format!("{}/rest/agile/1.0/sprint/{sprint_id}/issue", self.base_url);
        let body = serde_json::json!({ "issues": [issue_key] });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("Failed to assign {issue_key} to sprint {sprint_id}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira assign to sprint returned {status}: {body}");
        }
        Ok(())
    }

    pub async fn move_to_active_sprint(&self, issue_key: &str, sprint_id: u64) -> Result<()> {
        self.assign_issue_to_sprint(sprint_id, issue_key).await
    }

    pub async fn get_project_components(&self, project_key: &str) -> Result<Vec<ProjectComponent>> {
        let url = format!("{}/rest/api/2/project/{project_key}/components", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to get components for {project_key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get components returned {status}: {body}");
        }

        resp.json::<Vec<ProjectComponent>>()
            .await
            .context("Failed to parse components response")
    }
}
