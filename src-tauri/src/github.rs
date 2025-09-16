use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;
use log::{debug, info, error, trace};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Organization {
    pub id: u64,  // GitHub API returns this as a number
    pub login: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub number: u32,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectColumn {
    pub id: String,
    pub name: String,
    pub items_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectItem {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub column_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectData {
    pub project: Project,
    pub columns: Vec<ProjectColumn>,
    pub items: Vec<ProjectItem>,
}

pub struct GitHubClient {
    token: String,
}

impl GitHubClient {
    pub fn new() -> Result<Self> {
        debug!("Creating new GitHub client using gh CLI");
        let output = Command::new("gh")
            .args(&["auth", "token"])
            .output()?;

        if !output.status.success() {
            error!("gh CLI authentication check failed with status: {:?}", output.status);
            error!("stderr: {}", String::from_utf8_lossy(&output.stderr));
            anyhow::bail!("Failed to get GitHub token. Please ensure 'gh' is authenticated.");
        }

        let token = String::from_utf8(output.stdout)?.trim().to_string();
        info!("GitHub client created successfully (token length: {})", token.len());
        Ok(Self { token })
    }

    pub async fn list_organizations(&self) -> Result<Vec<Organization>> {
        debug!("Fetching organizations from GitHub API");
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.github.com/user/orgs")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "Minik-Kanban-App")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            error!("GitHub API returned error status: {}", status);
            let error_body = response.text().await?;
            error!("Error response body: {}", error_body);
            anyhow::bail!("Failed to fetch organizations: {}", status);
        }

        let orgs: Vec<Organization> = response.json().await?;
        info!("Successfully fetched {} organizations", orgs.len());
        for org in &orgs {
            debug!("  - {} ({})", org.login, org.name.as_deref().unwrap_or("no name"));
        }
        Ok(orgs)
    }

    pub async fn list_org_projects(&self, org: &str) -> Result<Vec<Project>> {
        debug!("Fetching projects for organization: {}", org);
        let query = r#"
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

        let variables = serde_json::json!({
            "org": org
        });

        let response = self.graphql_request(query, variables).await?;

        let projects_nodes = &response["data"]["organization"]["projectsV2"]["nodes"];
        let projects: Vec<Project> = projects_nodes
            .as_array()
            .ok_or_else(|| {
                error!("Failed to parse projects array from response");
                anyhow::anyhow!("Failed to parse projects")
            })?
            .iter()
            .filter_map(|p| {
                Some(Project {
                    id: p["id"].as_str()?.to_string(),
                    title: p["title"].as_str()?.to_string(),
                    number: p["number"].as_u64()? as u32,
                    url: p["url"].as_str()?.to_string(),
                })
            })
            .collect();

        info!("Successfully fetched {} projects for org {}", projects.len(), org);
        for project in &projects {
            debug!("  - {} (#{}) - {}", project.title, project.number, project.url);
        }
        Ok(projects)
    }

    pub async fn project_data(&self, project_id: &str) -> Result<ProjectData> {
        info!("Fetching detailed data for project ID: {}", project_id);
        let query = r#"
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

        let variables = serde_json::json!({
            "projectId": project_id
        });

        let response = self.graphql_request(query, variables).await?;
        let project_node = &response["data"]["node"];

        if project_node.is_null() {
            error!("Project not found for ID: {}", project_id);
            anyhow::bail!("Project not found");
        }

        let project = Project {
            id: project_node["id"].as_str().unwrap_or_default().to_string(),
            title: project_node["title"].as_str().unwrap_or_default().to_string(),
            number: project_node["number"].as_u64().unwrap_or_default() as u32,
            url: project_node["url"].as_str().unwrap_or_default().to_string(),
        };

        debug!("Project: {} (#{}) - {}", project.title, project.number, project.url);

        let mut columns = Vec::new();
        let mut column_map = std::collections::HashMap::new();

        if let Some(views) = project_node["views"]["nodes"].as_array() {
            if let Some(first_view) = views.first() {
                if let Some(fields) = first_view["fields"]["nodes"].as_array() {
                    for field in fields {
                        if let Some(options) = field["options"].as_array() {
                            let field_id = field["id"].as_str().unwrap_or_default();
                            for option in options {
                                let option_id = option["id"].as_str().unwrap_or_default().to_string();
                                let option_name = option["name"].as_str().unwrap_or_default().to_string();
                                column_map.insert(option_id.clone(), (field_id.to_string(), option_name.clone()));
                                columns.push(ProjectColumn {
                                    id: option_id.clone(),
                                    name: option_name.clone(),
                                    items_count: 0,
                                });
                                trace!("  Column: {} - {}", option_name, option_id);
                            }
                        }
                    }
                }
            }
        }

        let mut items = Vec::new();
        let mut column_counts = std::collections::HashMap::new();

        if let Some(items_nodes) = project_node["items"]["nodes"].as_array() {
            debug!("Processing {} project items", items_nodes.len());
            for item in items_nodes {
                let content = &item["content"];
                if content.is_null() {
                    trace!("Skipping item with null content");
                    continue;
                }

                let title = content["title"].as_str().unwrap_or("Untitled").to_string();
                let url = content["url"].as_str().map(|s| s.to_string());

                let assignees = content["assignees"]["nodes"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|a| a["login"].as_str().map(|s| s.to_string()))
                    .collect();

                let labels = content["labels"]["nodes"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                    .collect();

                let mut column_id = String::new();
                if let Some(field_values) = item["fieldValues"]["nodes"].as_array() {
                    for fv in field_values {
                        if let Some(option_id) = fv["optionId"].as_str() {
                            column_id = option_id.to_string();
                            *column_counts.entry(option_id.to_string()).or_insert(0) += 1;
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

        for column in &mut columns {
            column.items_count = *column_counts.get(&column.id).unwrap_or(&0);
            debug!("Column '{}': {} items", column.name, column.items_count);
        }

        info!("Successfully fetched project data: {} columns, {} items total",
              columns.len(), items.len());

        Ok(ProjectData {
            project,
            columns,
            items,
        })
    }

    async fn graphql_request(&self, query: &str, variables: serde_json::Value) -> Result<serde_json::Value> {
        trace!("GraphQL request with variables: {:?}", variables);
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.github.com/graphql")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "Minik-Kanban-App")
            .json(&serde_json::json!({
                "query": query,
                "variables": variables
            }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            error!("GraphQL request failed with status {}: {}", status, error_text);
            anyhow::bail!("GraphQL request failed: {}", error_text);
        }

        let data: serde_json::Value = response.json().await?;

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