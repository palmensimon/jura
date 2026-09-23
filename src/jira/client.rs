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
                ("fields", "summary,status,issuetype,priority,assignee,components,labels,parent,description,customfield_10020,fixVersions"),
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
            .query(&[("fields", "summary,status,issuetype,priority,assignee,components,labels,parent,description,customfield_10020,fixVersions")])
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

    pub async fn get_active_sprint_id(&self, board_id: u64) -> Result<u64> {
        let url = format!("{}/rest/agile/1.0/board/{board_id}/sprint", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("state", "active")])
            .send()
            .await
            .with_context(|| format!("Failed to fetch sprints for board {board_id}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get sprints returned {status}: {body}");
        }

        let list: SprintList = resp.json().await.context("Failed to parse sprint list")?;
        list.values
            .into_iter()
            .next()
            .map(|s| s.id)
            .ok_or_else(|| anyhow::anyhow!("No active sprint found for board {board_id}"))
    }

    pub async fn move_to_active_sprint(&self, issue_key: &str, board_id: u64) -> Result<()> {
        let sprint_id = self.get_active_sprint_id(board_id).await?;
        self.assign_issue_to_sprint(sprint_id, issue_key).await
    }

    /// Lists all Jira boards visible to the current user (paginated).
    pub async fn get_boards(&self) -> Result<Vec<Board>> {
        let url = format!("{}/rest/agile/1.0/board", self.base_url);
        let mut boards = Vec::new();
        let mut start_at = 0u32;
        loop {
            let resp = self
                .client
                .get(&url)
                .query(&[("startAt", start_at.to_string()), ("maxResults", "50".to_string())])
                .send()
                .await
                .context("Failed to fetch boards")?;

            let status = resp.status();
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("Jira get boards returned {status}: {body}");
            }

            let page: BoardList = resp.json().await.context("Failed to parse boards response")?;
            let is_last = page.is_last || page.values.is_empty();
            boards.extend(page.values);
            if is_last || boards.len() >= 1000 {
                break;
            }
            start_at += 50;
        }
        boards.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(boards)
    }

    /// Fetches a single board by id — used to resolve the display name of a previously
    /// picked board without paging through the whole list.
    pub async fn get_board(&self, board_id: u64) -> Result<Board> {
        let url = format!("{}/rest/agile/1.0/board/{board_id}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch board {board_id}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get board returned {status}: {body}");
        }

        resp.json::<Board>().await.context("Failed to parse board response")
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

    /// Global list of issue priorities configured on this Jira instance (not project-scoped).
    pub async fn get_priorities(&self) -> Result<Vec<Priority>> {
        let url = format!("{}/rest/api/2/priority", self.base_url);
        let resp = self.client.get(&url).send().await.context("Failed to get priorities")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get priorities returned {status}: {body}");
        }

        resp.json::<Vec<Priority>>().await.context("Failed to parse priorities response")
    }

    /// Issue types creatable for a project (Jira 8.4+ granular createmeta endpoint).
    pub async fn get_issue_types(&self, project_key: &str) -> Result<Vec<IssueType>> {
        let url = format!("{}/rest/api/2/issue/createmeta/{project_key}/issuetypes", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to get issue types for {project_key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get issue types returned {status}: {body}");
        }

        let list: IssueTypeList = resp.json().await.context("Failed to parse issue types response")?;
        Ok(list.values)
    }

    /// Fix versions (releases) configured for a project.
    pub async fn get_project_versions(&self, project_key: &str) -> Result<Vec<FixVersion>> {
        let url = format!("{}/rest/api/2/project/{project_key}/versions", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to get versions for {project_key}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get versions returned {status}: {body}");
        }

        resp.json::<Vec<FixVersion>>().await.context("Failed to parse versions response")
    }

    /// Jira's JQL-bar autocomplete suggestions for a given field name (e.g. "Team").
    /// `query` may be empty to fetch the field's default suggestion list.
    pub async fn search_field_suggestions(&self, field_name: &str, query: &str) -> Result<Vec<FieldSuggestion>> {
        let url = format!("{}/rest/api/2/jql/autocompletedata/suggestions", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[("fieldName", field_name), ("fieldValue", query)])
            .send()
            .await
            .with_context(|| format!("Failed to get suggestions for {field_name}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Jira get suggestions returned {status}: {body}");
        }

        let list: FieldSuggestionList = resp.json().await.context("Failed to parse suggestions response")?;
        Ok(list
            .results
            .into_iter()
            .map(|r| FieldSuggestion {
                value: r.value,
                display_name: clean_suggestion_html(&r.display_name),
            })
            .collect())
    }
}

/// Strips Jira's `<b>…</b>` highlight markup and unescapes the handful of HTML entities
/// that show up in JQL-suggestion display names (e.g. `"R&amp;D"` -> `"R&D"`).
fn clean_suggestion_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}
