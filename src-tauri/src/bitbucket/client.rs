use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{header, Client};
use thiserror::Error;

use super::types::{PaginatedResponse, Pipeline, PipelineStep, Project, Repository, Workspace};

const BITBUCKET_API_BASE: &str = "https://api.bitbucket.org/2.0";

#[derive(Error, Debug)]
pub enum BitbucketError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication failed - check username and app password")]
    AuthenticationFailed,
    #[error("Rate limited - please wait before retrying")]
    RateLimited,
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("API error: {0}")]
    ApiError(String),
}

/// Client for interacting with the Bitbucket Cloud REST API
pub struct BitbucketClient {
    client: Client,
    auth_header: String,
}

impl BitbucketClient {
    /// Create a new Bitbucket client with basic auth credentials
    pub fn new(username: &str, app_password: &str) -> Self {
        let credentials = format!("{}:{}", username, app_password);
        let auth_header = format!("Basic {}", STANDARD.encode(credentials));

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            auth_header,
        }
    }

    /// Get all workspaces accessible to the authenticated user
    pub async fn get_workspaces(&self) -> Result<Vec<Workspace>, BitbucketError> {
        let url = format!("{}/workspaces?pagelen=100", BITBUCKET_API_BASE);
        let response: PaginatedResponse<Workspace> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Get all projects in a workspace
    pub async fn get_projects(&self, workspace: &str) -> Result<Vec<Project>, BitbucketError> {
        let url = format!(
            "{}/workspaces/{}/projects?pagelen=100",
            BITBUCKET_API_BASE, workspace
        );
        let response: PaginatedResponse<Project> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Get all repositories in a workspace
    pub async fn get_repositories(&self, workspace: &str) -> Result<Vec<Repository>, BitbucketError> {
        let url = format!(
            "{}/repositories/{}?pagelen=100&sort=-updated_on",
            BITBUCKET_API_BASE, workspace
        );
        let response: PaginatedResponse<Repository> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Get repositories in a workspace filtered by project key
    pub async fn get_repositories_by_project(
        &self,
        workspace: &str,
        project_key: &str,
    ) -> Result<Vec<Repository>, BitbucketError> {
        let url = format!(
            "{}/repositories/{}?pagelen=100&sort=-updated_on&q=project.key=\"{}\"",
            BITBUCKET_API_BASE, workspace, project_key
        );
        let response: PaginatedResponse<Repository> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Get recent pipelines for a repository
    pub async fn get_pipelines(
        &self,
        workspace: &str,
        repo_slug: &str,
        limit: u32,
    ) -> Result<Vec<Pipeline>, BitbucketError> {
        let url = format!(
            "{}/repositories/{}/{}/pipelines/?sort=-created_on&pagelen={}",
            BITBUCKET_API_BASE, workspace, repo_slug, limit
        );
        let response: PaginatedResponse<Pipeline> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Get the latest pipeline for a repository, optionally filtered by branch
    pub async fn get_latest_pipeline(
        &self,
        workspace: &str,
        repo_slug: &str,
        branch: Option<&str>,
    ) -> Result<Option<Pipeline>, BitbucketError> {
        // Fetch recent pipelines
        let pipelines = self.get_pipelines(workspace, repo_slug, 20).await?;

        // If branch filter is specified, find the first matching pipeline
        if let Some(branch_name) = branch {
            Ok(pipelines
                .into_iter()
                .find(|p| p.target.ref_name.as_deref() == Some(branch_name)))
        } else {
            // Return the most recent pipeline
            Ok(pipelines.into_iter().next())
        }
    }

    /// Get steps for a specific pipeline
    pub async fn get_pipeline_steps(
        &self,
        workspace: &str,
        repo_slug: &str,
        pipeline_uuid: &str,
    ) -> Result<Vec<PipelineStep>, BitbucketError> {
        let url = format!(
            "{}/repositories/{}/{}/pipelines/{}/steps/",
            BITBUCKET_API_BASE, workspace, repo_slug, pipeline_uuid
        );
        let response: PaginatedResponse<PipelineStep> = self.get(&url).await?;
        Ok(response.values)
    }

    /// Validate credentials by attempting to fetch workspaces
    pub async fn validate_credentials(&self) -> Result<bool, BitbucketError> {
        match self.get_workspaces().await {
            Ok(_) => Ok(true),
            Err(BitbucketError::AuthenticationFailed) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Make a GET request to the Bitbucket API
    async fn get<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
    ) -> Result<T, BitbucketError> {
        let response = self
            .client
            .get(url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;

        match response.status().as_u16() {
            200 => Ok(response.json().await?),
            401 => Err(BitbucketError::AuthenticationFailed),
            429 => Err(BitbucketError::RateLimited),
            404 => Err(BitbucketError::NotFound(url.to_string())),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(BitbucketError::ApiError(format!(
                    "Status {}: {}",
                    status, body
                )))
            }
        }
    }
}
