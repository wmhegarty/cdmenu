use serde::{Deserialize, Serialize};

/// Application state shared across the app
#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub credentials: Option<Credentials>,
    pub monitored_pipelines: Vec<MonitoredPipeline>,
    pub polling_interval_seconds: u64,
    pub last_status: Option<OverallStatus>,
}

/// User credentials (password stored in Stronghold)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
}

/// A pipeline configuration to monitor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MonitoredPipeline {
    pub workspace: String,
    pub project_key: Option<String>,
    pub project_name: Option<String>,
    pub repo_slug: String,
    pub repo_name: String,
    /// Optional: monitor a specific branch only
    pub branch: Option<String>,
}

/// Status of an individual pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineState {
    Healthy,
    Failed,
    InProgress,
    Paused,
    Unknown,
}

/// Individual pipeline status info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatusInfo {
    pub workspace: String,
    pub project_key: Option<String>,
    pub project_name: Option<String>,
    pub repo_slug: String,
    pub repo_name: String,
    pub state: PipelineState,
    pub failure_reason: Option<String>,
    pub pipeline_url: Option<String>,
    /// Stage name when pipeline is paused (e.g., deployment environment)
    pub stage_name: Option<String>,
}

/// Overall status of all monitored pipelines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallStatus {
    pub is_healthy: bool,
    pub failed_pipelines: Vec<FailedPipelineInfo>,
    pub pipeline_statuses: Vec<PipelineStatusInfo>,
    pub in_progress_count: usize,
    pub total_monitored: usize,
    pub last_checked: String,
}

/// Information about a failed pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedPipelineInfo {
    pub workspace: String,
    pub repo_slug: String,
    pub repo_name: String,
    pub branch: Option<String>,
    pub build_number: u32,
    pub failure_reason: String,
}

/// Persisted configuration saved to disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedConfig {
    pub username: Option<String>,
    pub monitored_pipelines: Vec<MonitoredPipeline>,
    pub polling_interval_seconds: u64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            credentials: None,
            monitored_pipelines: Vec::new(),
            polling_interval_seconds: 60,
            last_status: None,
        }
    }

    /// Convert to persisted config for saving
    pub fn to_persisted(&self) -> PersistedConfig {
        PersistedConfig {
            username: self.credentials.as_ref().map(|c| c.username.clone()),
            monitored_pipelines: self.monitored_pipelines.clone(),
            polling_interval_seconds: self.polling_interval_seconds,
        }
    }

    /// Load from persisted config
    pub fn from_persisted(config: PersistedConfig) -> Self {
        Self {
            credentials: config.username.map(|username| Credentials { username }),
            monitored_pipelines: config.monitored_pipelines,
            polling_interval_seconds: if config.polling_interval_seconds >= 30 {
                config.polling_interval_seconds
            } else {
                60
            },
            last_status: None,
        }
    }
}

impl OverallStatus {
    pub fn new(
        pipeline_statuses: Vec<PipelineStatusInfo>,
        timestamp: String,
    ) -> Self {
        let failed_pipelines: Vec<FailedPipelineInfo> = pipeline_statuses
            .iter()
            .filter(|p| matches!(p.state, PipelineState::Failed))
            .map(|p| FailedPipelineInfo {
                workspace: p.workspace.clone(),
                repo_slug: p.repo_slug.clone(),
                repo_name: p.repo_name.clone(),
                branch: None,
                build_number: 0,
                failure_reason: p.failure_reason.clone().unwrap_or_else(|| "Unknown".to_string()),
            })
            .collect();

        let in_progress_count = pipeline_statuses
            .iter()
            .filter(|p| matches!(p.state, PipelineState::InProgress))
            .count();

        let is_healthy = failed_pipelines.is_empty();
        let total_monitored = pipeline_statuses.len();

        Self {
            is_healthy,
            failed_pipelines,
            pipeline_statuses,
            in_progress_count,
            total_monitored,
            last_checked: timestamp,
        }
    }
}

