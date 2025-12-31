use serde::{Deserialize, Serialize};

/// Paginated response wrapper from Bitbucket API
#[derive(Debug, Deserialize)]
pub struct PaginatedResponse<T> {
    pub values: Vec<T>,
    pub page: Option<u32>,
    pub size: Option<u32>,
    pub next: Option<String>,
}

/// Bitbucket workspace
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Workspace {
    pub uuid: String,
    pub slug: String,
    pub name: String,
}

/// Bitbucket project (within a workspace)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Project {
    pub uuid: String,
    pub key: String,
    pub name: String,
}

/// Bitbucket repository
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Repository {
    pub uuid: String,
    pub slug: String,
    pub name: String,
    pub full_name: String,
    pub project: Option<Project>,
}

/// Bitbucket pipeline
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Pipeline {
    pub uuid: String,
    pub build_number: u32,
    pub state: PipelineState,
    pub target: PipelineTarget,
    pub created_on: String,
    pub completed_on: Option<String>,
}

/// Pipeline state containing status and result
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PipelineState {
    /// "PENDING", "IN_PROGRESS", "COMPLETED"
    pub name: String,
    /// State type - e.g. "pipeline_state_in_progress_paused" when waiting for manual trigger
    #[serde(rename = "type")]
    pub state_type: Option<String>,
    pub result: Option<PipelineResult>,
    /// Stage info when pipeline is paused
    pub stage: Option<PipelineStage>,
}

/// Pipeline stage info (present when paused)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PipelineStage {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub stage_type: Option<String>,
}

/// Pipeline result when completed
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PipelineResult {
    /// "SUCCESSFUL", "FAILED", "STOPPED", "EXPIRED", "ERROR"
    pub name: String,
}

/// Pipeline target (branch/tag info)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PipelineTarget {
    pub ref_type: Option<String>,
    pub ref_name: Option<String>,
}

/// Pipeline step (individual stage in a pipeline)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PipelineStep {
    pub uuid: String,
    pub name: Option<String>,
    pub state: Option<StepState>,
}

/// State of a pipeline step
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StepState {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub state_type: Option<String>,
}

impl PipelineStep {
    /// Check if this step is pending/waiting for manual trigger
    pub fn is_pending(&self) -> bool {
        if let Some(state) = &self.state {
            if let Some(name) = &state.name {
                return name == "PENDING";
            }
            if let Some(state_type) = &state.state_type {
                return state_type.contains("pending");
            }
        }
        false
    }
}

impl Pipeline {
    /// Check if the pipeline is in a failed state
    pub fn is_failed(&self) -> bool {
        if let Some(result) = &self.state.result {
            matches!(result.name.as_str(), "FAILED" | "ERROR" | "EXPIRED")
        } else {
            false
        }
    }

    /// Check if the pipeline completed successfully
    pub fn is_successful(&self) -> bool {
        if let Some(result) = &self.state.result {
            result.name == "SUCCESSFUL"
        } else {
            false
        }
    }

    /// Check if the pipeline is paused waiting for user input (manual trigger)
    pub fn is_paused(&self) -> bool {
        // Check state.type for "paused"
        if let Some(state_type) = &self.state.state_type {
            if state_type.contains("paused") {
                return true;
            }
        }
        // Check state.stage.name for "PAUSED" (Bitbucket's way of indicating manual step waiting)
        if let Some(stage) = &self.state.stage {
            if let Some(stage_name) = &stage.name {
                if stage_name.to_uppercase() == "PAUSED" {
                    return true;
                }
            }
        }
        false
    }

    /// Check if the pipeline is actively running (not paused/waiting)
    pub fn is_in_progress(&self) -> bool {
        let in_progress = self.state.name == "IN_PROGRESS" || self.state.name == "PENDING";
        // Only return true if actively running, not if paused waiting for input
        in_progress && !self.is_paused()
    }

    /// Get the branch name if available
    pub fn branch(&self) -> Option<&str> {
        self.target.ref_name.as_deref()
    }
}
