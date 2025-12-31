use crate::bitbucket::{BitbucketClient, Pipeline, Project, Repository, Workspace};
use crate::config::{AppState, Credentials, MonitoredPipeline, OverallStatus, PersistedConfig};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::Arc;
use tauri::{command, AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

/// Get all workspaces accessible to the user
#[command]
pub async fn get_workspaces(
    username: String,
    app_password: String,
) -> Result<Vec<Workspace>, String> {
    let client = BitbucketClient::new(&username, &app_password);
    client
        .get_workspaces()
        .await
        .map_err(|e| format!("{}", e))
}

/// Get all projects in a workspace
#[command]
pub async fn get_projects(
    username: String,
    app_password: String,
    workspace: String,
) -> Result<Vec<Project>, String> {
    let client = BitbucketClient::new(&username, &app_password);
    client
        .get_projects(&workspace)
        .await
        .map_err(|e| format!("{}", e))
}

/// Get all repositories in a workspace
#[command]
pub async fn get_repositories(
    username: String,
    app_password: String,
    workspace: String,
) -> Result<Vec<Repository>, String> {
    let client = BitbucketClient::new(&username, &app_password);
    client
        .get_repositories(&workspace)
        .await
        .map_err(|e| format!("{}", e))
}

/// Get repositories filtered by project
#[command]
pub async fn get_repositories_by_project(
    username: String,
    app_password: String,
    workspace: String,
    project_key: String,
) -> Result<Vec<Repository>, String> {
    let client = BitbucketClient::new(&username, &app_password);
    client
        .get_repositories_by_project(&workspace, &project_key)
        .await
        .map_err(|e| format!("{}", e))
}

/// Get recent pipelines for a repository
#[command]
pub async fn get_pipelines(
    username: String,
    app_password: String,
    workspace: String,
    repo_slug: String,
) -> Result<Vec<Pipeline>, String> {
    let client = BitbucketClient::new(&username, &app_password);
    client
        .get_pipelines(&workspace, &repo_slug, 10)
        .await
        .map_err(|e| format!("{}", e))
}

/// Save user credentials (username in state, password obfuscated in config)
#[command]
pub async fn save_credentials(
    app_handle: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    username: String,
    app_password: String,
) -> Result<(), String> {
    // Validate credentials first
    let client = BitbucketClient::new(&username, &app_password);
    if !client
        .validate_credentials()
        .await
        .map_err(|e| format!("{}", e))?
    {
        return Err("Invalid credentials".to_string());
    }

    // Store username in state
    {
        let mut state_guard = state.lock().await;
        state_guard.credentials = Some(Credentials {
            username: username.clone(),
        });
    }

    // Save password to secure config
    save_password(&app_handle, &app_password)?;

    // Save config to disk
    save_config_helper(&app_handle, &state).await?;

    Ok(())
}

/// Get the saved username (if any)
#[command]
pub async fn get_credentials(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<String>, String> {
    let state_guard = state.lock().await;
    Ok(state_guard.credentials.as_ref().map(|c| c.username.clone()))
}

/// Get the app password from secure storage
#[command]
pub async fn get_app_password(app_handle: AppHandle) -> Result<Option<String>, String> {
    retrieve_password(&app_handle)
}

/// Save the list of monitored pipelines
#[command]
pub async fn save_monitored_pipelines(
    app_handle: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    pipelines: Vec<MonitoredPipeline>,
) -> Result<(), String> {
    {
        let mut state_guard = state.lock().await;
        state_guard.monitored_pipelines = pipelines;
    }
    save_config_helper(&app_handle, &state).await
}

/// Get the list of monitored pipelines
#[command]
pub async fn get_monitored_pipelines(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<MonitoredPipeline>, String> {
    let state_guard = state.lock().await;
    Ok(state_guard.monitored_pipelines.clone())
}

/// Get the current pipeline status
#[command]
pub async fn get_pipeline_statuses(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<OverallStatus>, String> {
    let state_guard = state.lock().await;
    Ok(state_guard.last_status.clone())
}

/// Set the polling interval
#[command]
pub async fn set_polling_interval(
    app_handle: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    seconds: u64,
) -> Result<(), String> {
    if seconds < 30 {
        return Err("Polling interval must be at least 30 seconds".to_string());
    }
    {
        let mut state_guard = state.lock().await;
        state_guard.polling_interval_seconds = seconds;
    }
    save_config_helper(&app_handle, &state).await
}

/// Get the polling interval
#[command]
pub async fn get_polling_interval(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<u64, String> {
    let state_guard = state.lock().await;
    Ok(state_guard.polling_interval_seconds)
}

/// Trigger an immediate refresh
#[command]
pub async fn trigger_refresh(app_handle: AppHandle) -> Result<(), String> {
    app_handle
        .emit("trigger-refresh", ())
        .map_err(|e: tauri::Error| e.to_string())
}

// Helper: Save password to secure file (base64 obfuscated for MVP)
fn save_password(app_handle: &AppHandle, password: &str) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let creds_path = config_dir.join(".credentials");
    let encoded = STANDARD.encode(password.as_bytes());

    std::fs::write(&creds_path, encoded)
        .map_err(|e| format!("Failed to write credentials: {}", e))?;

    Ok(())
}

// Helper: Retrieve password from secure file
fn retrieve_password(app_handle: &AppHandle) -> Result<Option<String>, String> {
    let config_dir = match app_handle.path().app_config_dir() {
        Ok(dir) => dir,
        Err(_) => return Ok(None),
    };

    let creds_path = config_dir.join(".credentials");

    if !creds_path.exists() {
        return Ok(None);
    }

    let encoded = match std::fs::read_to_string(&creds_path) {
        Ok(e) => e,
        Err(_) => return Ok(None),
    };

    let decoded = STANDARD
        .decode(encoded.trim())
        .map_err(|e| format!("Failed to decode credentials: {}", e))?;

    String::from_utf8(decoded)
        .map(Some)
        .map_err(|e| format!("Invalid credential data: {}", e))
}

// Helper: Save config to disk
async fn save_config_helper(
    app_handle: &AppHandle,
    state: &State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let state_guard = state.lock().await;
    let config = state_guard.to_persisted();

    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let config_path = config_dir.join("config.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    std::fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(())
}

/// Load config from disk
pub fn load_config(app_handle: &AppHandle) -> Option<PersistedConfig> {
    let config_dir = app_handle.path().app_config_dir().ok()?;
    let config_path = config_dir.join("config.json");

    if !config_path.exists() {
        return None;
    }

    let json = std::fs::read_to_string(&config_path).ok()?;
    serde_json::from_str(&json).ok()
}
