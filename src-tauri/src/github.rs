//! GitHub API client for interacting with Projects v2
//!
//! This module provides a client for fetching GitHub organization projects
//! and their associated data using both REST and GraphQL APIs.

use anyhow::{Context, Result};
use log::{debug, error, info, trace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

/// Represents a GitHub organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    /// GitHub organization ID
    pub id: u64,
    /// Organization login name
    pub login: String,
    /// Optional display name
    pub name: Option<String>,
}

/// Represents a GitHub Project v2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// GraphQL node ID
    pub id: String,
    /// Project title
    pub title: String,
    /// Project number within the organization
    pub number: u32,
    /// Web URL to the project
    pub url: String,
}

/// Represents a column in a project board
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectColumn {
    /// Column/option ID
    pub id: String,
    /// Column name
    pub name: String,
    /// Number of items in this column
    pub items_count: usize,
}

/// Represents an item (issue/PR) in a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectItem {
    /// Item ID
    pub id: String,
    /// Item title
    pub title: String,
    /// Optional URL to the issue/PR
    pub url: Option<String>,
    /// List of assignee usernames
    pub assignees: Vec<String>,
    /// List of label names
    pub labels: Vec<String>,
    /// ID of the column containing this item
    pub column_id: String,
}

/// Complete project data including columns and items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    /// The project metadata
    pub project: Project,
    /// List of columns/statuses
    pub columns: Vec<ProjectColumn>,
    /// List of items in the project
    pub items: Vec<ProjectItem>,
    /// GraphQL field ID for the status field
    pub status_field_id: String,
    /// List of hidden column IDs
    pub hidden_columns: Vec<String>,
}

/// Type alias for column extraction result
/// Returns (columns, status_field_id, column_map)
type ColumnExtractResult = (Vec<ProjectColumn>, String, HashMap<String, (String, String)>);

/// GitHub API client using authenticated requests
pub struct GitHubClient {
    token: String,
}

/// Find the gh CLI command in common locations
fn find_gh_command() -> Result<String> {
    const POSSIBLE_PATHS: &[&str] = &[
        "/opt/homebrew/bin/gh",  // Apple Silicon Homebrew
        "/usr/local/bin/gh",      // Intel Homebrew
        "gh",                      // System PATH
    ];

    for path in POSSIBLE_PATHS {
        if Command::new(path)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            debug!("Found gh at: {}", path);
            return Ok(path.to_string());
        }
    }

    error!("Could not find gh CLI in any common location");
    anyhow::bail!(
        "GitHub CLI (gh) not found. Please install it with 'brew install gh' \
         and authenticate with 'gh auth login'"
    )
}

impl GitHubClient {
    /// Create a new GitHub client using the gh CLI authentication
    pub fn new() -> Result<Self> {
        debug!("Creating new GitHub client using gh CLI");

        let gh_path = find_gh_command()?;
        let output = Command::new(&gh_path)
            .args(["auth", "token"])
            .output()
            .context("Failed to execute gh command")?;

        if !output.status.success() {
            error!("gh CLI authentication check failed with status: {:?}", output.status);
            error!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("Failed to get GitHub token. Please ensure 'gh' is authenticated.");
        }

        let token = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in gh token output")?
            .trim()
            .to_string();

        info!("GitHub client created successfully (token length: {})", token.len());
        Ok(Self { token })
    }

    /// List all organizations the authenticated user belongs to
    pub async fn list_organizations(&self) -> Result<Vec<Organization>> {
        debug!("Fetching organizations from GitHub API");

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.github.com/user/orgs")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "Minik-Kanban-App")
            .send()
            .await
            .context("Failed to send request to GitHub API")?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await?;
            error!("GitHub API returned error status: {} - {}", status, error_body);
            anyhow::bail!("Failed to fetch organizations: {}", status);
        }

        let orgs: Vec<Organization> = response
            .json()
            .await
            .context("Failed to parse organizations response")?;

        info!("Successfully fetched {} organizations", orgs.len());
        for org in &orgs {
            debug!("  - {} ({})", org.login, org.name.as_deref().unwrap_or("no name"));
        }

        Ok(orgs)
    }

    /// List all projects for a given organization
    pub async fn list_org_projects(&self, org: &str) -> Result<Vec<Project>> {
        debug!("Fetching projects for organization: {}", org);

        const QUERY: &str = r#"
        query($org: String!) {
            organization(login: $org) {
                projectsV2(first: 100) {
                    nodes {
                        id
                        title
                        number
                        url
                    }
                }
            }
        }
        "#;

        let variables = serde_json::json!({ "org": org });
        let response = self.graphql_request(QUERY, variables).await?;

        let projects_nodes = &response["data"]["organization"]["projectsV2"]["nodes"];
        let projects = projects_nodes
            .as_array()
            .context("Failed to parse projects array")?
            .iter()
            .filter_map(|p| {
                Some(Project {
                    id: p["id"].as_str()?.to_string(),
                    title: p["title"].as_str()?.to_string(),
                    number: p["number"].as_u64()? as u32,
                    url: p["url"].as_str()?.to_string(),
                })
            })
            .collect::<Vec<_>>();

        info!("Successfully fetched {} projects for org {}", projects.len(), org);
        for project in &projects {
            debug!("  - {} (#{}) - {}", project.title, project.number, project.url);
        }

        Ok(projects)
    }

    /// Get detailed data for a specific project
    pub async fn project_data(&self, project_id: &str) -> Result<ProjectData> {
        info!("Fetching detailed data for project ID: {}", project_id);

        const QUERY: &str = r#"
        query($projectId: ID!) {
            node(id: $projectId) {
                ... on ProjectV2 {
                    id
                    title
                    number
                    url
                    views(first: 1) {
                        nodes {
                            fields(first: 20) {
                                nodes {
                                    ... on ProjectV2SingleSelectField {
                                        id
                                        name
                                        options {
                                            id
                                            name
                                        }
                                    }
                                }
                            }
                        }
                    }
                    items(first: 100) {
                        nodes {
                            id
                            content {
                                ... on Issue {
                                    title
                                    url
                                    assignees(first: 10) {
                                        nodes {
                                            login
                                        }
                                    }
                                    labels(first: 10) {
                                        nodes {
                                            name
                                        }
                                    }
                                }
                                ... on PullRequest {
                                    title
                                    url
                                    assignees(first: 10) {
                                        nodes {
                                            login
                                        }
                                    }
                                    labels(first: 10) {
                                        nodes {
                                            name
                                        }
                                    }
                                }
                            }
                            fieldValues(first: 20) {
                                nodes {
                                    ... on ProjectV2ItemFieldSingleSelectValue {
                                        field {
                                            ... on ProjectV2SingleSelectField {
                                                id
                                            }
                                        }
                                        optionId
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "#;

        let variables = serde_json::json!({ "projectId": project_id });
        let response = self.graphql_request(QUERY, variables).await?;
        let project_node = &response["data"]["node"];

        if project_node.is_null() {
            error!("Project not found for ID: {}", project_id);
            anyhow::bail!("Project not found");
        }

        let project = Project {
            id: project_node["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            title: project_node["title"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            number: project_node["number"].as_u64().unwrap_or_default() as u32,
            url: project_node["url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };

        debug!("Project: {} (#{}) - {}", project.title, project.number, project.url);

        let (columns, status_field_id, _column_map) = self.extract_columns(project_node)?;
        let (items, column_counts) = self.extract_items(project_node)?;

        // Update column item counts
        let mut columns = columns;
        for column in &mut columns {
            column.items_count = column_counts.get(&column.id).copied().unwrap_or(0);
            debug!("Column '{}': {} items", column.name, column.items_count);
        }

        info!(
            "Successfully fetched project data: {} columns, {} items total",
            columns.len(),
            items.len()
        );

        Ok(ProjectData {
            project,
            columns,
            items,
            status_field_id,
            hidden_columns: Vec::new(), // Will be populated by the caller
        })
    }

    /// Extract columns from project node response
    /// Extract columns from project data
    fn extract_columns(&self, project_node: &serde_json::Value) -> Result<ColumnExtractResult> {
        let mut columns = Vec::new();
        let mut column_map = HashMap::new();
        let mut status_field_id = String::new();

        if let Some(views) = project_node["views"]["nodes"].as_array() {
            if let Some(first_view) = views.first() {
                if let Some(fields) = first_view["fields"]["nodes"].as_array() {
                    for field in fields {
                        let field_name = field["name"].as_str().unwrap_or_default();
                        debug!("Found field: {}", field_name);

                        // Only process the Status field for Kanban columns
                        if field_name == "Status" {
                            if let Some(options) = field["options"].as_array() {
                                let field_id = field["id"].as_str().unwrap_or_default();
                                status_field_id = field_id.to_string();
                                info!("Found Status field with ID: {}", field_id);

                                for option in options {
                                    let option_id = option["id"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string();
                                    let option_name = option["name"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string();

                                    column_map.insert(
                                        option_id.clone(),
                                        (field_id.to_string(), option_name.clone()),
                                    );

                                    columns.push(ProjectColumn {
                                        id: option_id.clone(),
                                        name: option_name.clone(),
                                        items_count: 0,
                                    });

                                    info!("  Status column: '{}' with option ID: {}", option_name, option_id);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((columns, status_field_id, column_map))
    }

    /// Extract items from project node response
    fn extract_items(&self, project_node: &serde_json::Value) -> Result<(Vec<ProjectItem>, HashMap<String, usize>)> {
        let mut items = Vec::new();
        let mut column_counts: HashMap<String, usize> = HashMap::new();

        if let Some(items_nodes) = project_node["items"]["nodes"].as_array() {
            debug!("Processing {} project items", items_nodes.len());

            for item in items_nodes {
                let content = &item["content"];
                if content.is_null() {
                    trace!("Skipping item with null content");
                    continue;
                }

                let title = content["title"]
                    .as_str()
                    .unwrap_or("Untitled")
                    .to_string();
                let url = content["url"].as_str().map(String::from);

                let assignees = content["assignees"]["nodes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a["login"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let labels = content["labels"]["nodes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|l| l["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut column_id = String::new();
                if let Some(field_values) = item["fieldValues"]["nodes"].as_array() {
                    for fv in field_values {
                        if let Some(option_id) = fv["optionId"].as_str() {
                            column_id = option_id.to_string();
                            *column_counts.entry(option_id.to_string()).or_default() += 1;
                            break;
                        }
                    }
                }

                items.push(ProjectItem {
                    id: item["id"].as_str().unwrap_or_default().to_string(),
                    title,
                    url,
                    assignees,
                    labels,
                    column_id,
                });
            }
        }

        Ok((items, column_counts))
    }

    /// Update a project item's field value
    pub async fn update_item_field(
        &self,
        project_id: &str,
        item_id: &str,
        field_id: &str,
        option_id: &str,
    ) -> Result<()> {
        info!("====== DRAG & DROP UPDATE ======");
        info!("Project ID: {}", project_id);
        info!("Item ID: {}", item_id);
        info!("Field ID (Status): {}", field_id);
        info!("Option ID (Column): {}", option_id);
        info!("=================================");

        const MUTATION: &str = r#"
        mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $value: ProjectV2FieldValue!) {
            updateProjectV2ItemFieldValue(input: {
                projectId: $projectId
                itemId: $itemId
                fieldId: $fieldId
                value: $value
            }) {
                projectV2Item {
                    id
                }
            }
        }
        "#;

        let variables = serde_json::json!({
            "projectId": project_id,
            "itemId": item_id,
            "fieldId": field_id,
            "value": {
                "singleSelectOptionId": option_id
            }
        });

        info!(
            "Sending GraphQL mutation with variables: {}",
            serde_json::to_string_pretty(&variables).unwrap_or_default()
        );

        let response = self.graphql_request(MUTATION, variables).await?;

        info!(
            "GraphQL response: {}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );

        if let Some(errors) = response["errors"].as_array() {
            if !errors.is_empty() {
                error!("GraphQL errors: {:?}", errors);
                anyhow::bail!("GraphQL errors: {:?}", errors);
            }
        }

        if response["data"]["updateProjectV2ItemFieldValue"]["projectV2Item"]["id"].is_null() {
            error!("Failed to update item field - no item ID in response");
            error!("Full response: {:?}", response);
            anyhow::bail!("Failed to update item field - no item ID in response");
        }

        info!("✅ Successfully updated item to new column!");
        Ok(())
    }

    /// Execute a GraphQL request
    async fn graphql_request(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value> {
        info!("🌐 ========== GRAPHQL REQUEST ==========");
        info!("📍 Endpoint: https://api.github.com/graphql");
        info!("🔑 Token present: {} (length: {})", !self.token.is_empty(), self.token.len());
        info!(
            "📝 Query preview: {}",
            query.lines().take(2).collect::<Vec<_>>().join(" ")
        );
        info!(
            "📊 Variables: {}",
            serde_json::to_string_pretty(&variables).unwrap_or_default()
        );

        let client = reqwest::Client::new();
        info!("🚀 Sending HTTP POST request to GitHub GraphQL API...");

        let request_body = serde_json::json!({
            "query": query,
            "variables": variables
        });

        let response = client
            .post("https://api.github.com/graphql")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "Minik-Kanban-App")
            .json(&request_body)
            .send()
            .await
            .context("Failed to send GraphQL request")?;

        let status = response.status();
        info!("📨 Response received! Status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await?;
            error!("GraphQL request failed with status {}: {}", status, error_text);
            anyhow::bail!("GraphQL request failed: {}", error_text);
        }

        let data: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse GraphQL response")?;

        if let Some(errors) = data["errors"].as_array() {
            if !errors.is_empty() {
                error!("GraphQL response contains errors: {:?}", errors);
                anyhow::bail!("GraphQL errors: {:?}", errors);
            }
        }

        trace!("GraphQL request successful");
        Ok(data)
    }
}