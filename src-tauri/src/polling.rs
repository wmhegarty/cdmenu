use crate::bitbucket::BitbucketClient;
use crate::config::{AppState, MonitoredPipeline, OverallStatus, PipelineState, PipelineStatusInfo};
use crate::tray::{update_tray_icon, update_tray_menu, update_tray_tooltip, TrayStatus};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Start the background polling loop
pub async fn start_polling(app_handle: AppHandle) {
    log::info!("Starting background polling loop");

    // Initial delay to let the app initialize
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Do an initial check immediately
    check_pipelines_once(&app_handle).await;

    // Then poll at regular intervals
    let mut check_interval = interval(Duration::from_secs(10));

    loop {
        check_interval.tick().await;

        // Get current polling interval from state
        let interval_secs = {
            let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();
            let state_guard = state.lock().await;
            state_guard.polling_interval_seconds
        };

        // Adjust interval if needed
        check_interval = interval(Duration::from_secs(interval_secs));

        check_pipelines_once(&app_handle).await;
    }
}

/// Perform a single check of all monitored pipelines
async fn check_pipelines_once(app_handle: &AppHandle) {
    let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();

    // Get current configuration
    let (credentials, monitored) = {
        let state_guard = state.lock().await;

        // Skip if no credentials or no pipelines
        if state_guard.credentials.is_none() || state_guard.monitored_pipelines.is_empty() {
            update_tray_icon(app_handle, TrayStatus::Gray);
            if state_guard.credentials.is_none() {
                update_tray_tooltip(app_handle, "cdMenu - Not configured");
            } else {
                update_tray_tooltip(app_handle, "cdMenu - No pipelines selected");
            }
            return;
        }

        (
            state_guard.credentials.clone().unwrap(),
            state_guard.monitored_pipelines.clone(),
        )
    };

    // Get app password from config
    let app_password = match get_app_password(app_handle) {
        Some(pw) => pw,
        None => {
            log::warn!("No app password found");
            update_tray_icon(app_handle, TrayStatus::Gray);
            update_tray_tooltip(app_handle, "cdMenu - Auth required");
            return;
        }
    };

    // Check all pipelines
    log::info!("Checking {} pipelines...", monitored.len());
    let status = check_all_pipelines(&credentials.username, &app_password, &monitored).await;

    // Update tray based on status
    if status.is_healthy {
        update_tray_icon(app_handle, TrayStatus::Green);

        let mut tooltip = format!(
            "cdMenu\n{} pipeline(s) healthy",
            status.total_monitored
        );
        if status.in_progress_count > 0 {
            tooltip.push_str(&format!("\n{} in progress", status.in_progress_count));
        }
        tooltip.push_str(&format!("\nLast checked: {}", status.last_checked));

        update_tray_tooltip(app_handle, &tooltip);
    } else {
        update_tray_icon(app_handle, TrayStatus::Red);

        let failed_names: Vec<String> = status
            .failed_pipelines
            .iter()
            .take(3) // Limit to 3 for tooltip
            .map(|p| format!("{}/{}", p.workspace, p.repo_slug))
            .collect();

        let mut tooltip = format!(
            "cdMenu\n{} pipeline(s) FAILED",
            status.failed_pipelines.len()
        );
        tooltip.push_str(&format!("\n{}", failed_names.join(", ")));
        if status.failed_pipelines.len() > 3 {
            tooltip.push_str(&format!(" +{} more", status.failed_pipelines.len() - 3));
        }
        tooltip.push_str(&format!("\nLast checked: {}", status.last_checked));

        update_tray_tooltip(app_handle, &tooltip);
    }

    // Check for status changes and send notifications
    {
        let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();
        let state_guard = state.lock().await;
        if let Some(old_status) = &state_guard.last_status {
            // Check each pipeline for status changes
            for new_pipeline in &status.pipeline_statuses {
                // Find matching old pipeline
                let old_pipeline = old_status.pipeline_statuses.iter().find(|p| {
                    p.workspace == new_pipeline.workspace && p.repo_slug == new_pipeline.repo_slug
                });

                if let Some(old) = old_pipeline {
                    let was_failed = matches!(old.state, PipelineState::Failed);
                    let is_failed = matches!(new_pipeline.state, PipelineState::Failed);

                    let name = if new_pipeline.repo_name.is_empty() {
                        &new_pipeline.repo_slug
                    } else {
                        &new_pipeline.repo_name
                    };

                    // Notify on new failure
                    if !was_failed && is_failed {
                        let body = if let Some(url) = &new_pipeline.pipeline_url {
                            format!("{} has failed\n{}", name, url)
                        } else {
                            format!("{} has failed", name)
                        };
                        let _ = app_handle
                            .notification()
                            .builder()
                            .title("Pipeline Failed")
                            .body(&body)
                            .show();
                    }

                    // Notify when fixed
                    if was_failed && !is_failed && matches!(new_pipeline.state, PipelineState::Healthy) {
                        let body = if let Some(url) = &new_pipeline.pipeline_url {
                            format!("{} is now healthy\n{}", name, url)
                        } else {
                            format!("{} is now healthy", name)
                        };
                        let _ = app_handle
                            .notification()
                            .builder()
                            .title("Pipeline Fixed")
                            .body(&body)
                            .show();
                    }
                }
            }
        }
    }

    // Check if status changed before updating menu
    let status_changed = {
        let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();
        let state_guard = state.lock().await;
        match &state_guard.last_status {
            Some(old) => old.is_healthy != status.is_healthy
                || old.pipeline_statuses.len() != status.pipeline_statuses.len()
                || old.pipeline_statuses.iter().zip(status.pipeline_statuses.iter())
                    .any(|(a, b)| std::mem::discriminant(&a.state) != std::mem::discriminant(&b.state)),
            None => true,
        }
    };

    // Store status in state
    {
        let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();
        let mut state_guard = state.lock().await;
        state_guard.last_status = Some(status.clone());
    }

    // Only update tray menu if status changed (avoids menu closing)
    if status_changed {
        update_tray_menu(app_handle, Some(&status));
    }

    // Emit event to frontend
    let _ = app_handle.emit("status-updated", &status);
}

/// Check all monitored pipelines and return aggregated status
async fn check_all_pipelines(
    username: &str,
    app_password: &str,
    monitored: &[MonitoredPipeline],
) -> OverallStatus {
    let client = BitbucketClient::new(username, app_password);
    let mut pipeline_statuses = Vec::new();

    for pipeline_config in monitored {
        match client
            .get_latest_pipeline(
                &pipeline_config.workspace,
                &pipeline_config.repo_slug,
                pipeline_config.branch.as_deref(),
            )
            .await
        {
            Ok(Some(pipeline)) => {
                let (state, failure_reason, stage_name) = if pipeline.is_failed() {
                    (
                        PipelineState::Failed,
                        pipeline.state.result.as_ref().map(|r| r.name.clone()),
                        None,
                    )
                } else if pipeline.is_paused() {
                    // Pipeline is waiting for manual trigger/approval
                    // Fetch steps to get the name of the pending step
                    let pending_step_name = match client
                        .get_pipeline_steps(
                            &pipeline_config.workspace,
                            &pipeline_config.repo_slug,
                            &pipeline.uuid,
                        )
                        .await
                    {
                        Ok(steps) => {
                            // Find the first pending step
                            steps
                                .iter()
                                .find(|s| s.is_pending())
                                .and_then(|s| s.name.clone())
                                .unwrap_or_else(|| "paused".to_string())
                        }
                        Err(_) => "paused".to_string(),
                    };
                    (PipelineState::Paused, None, Some(pending_step_name))
                } else if pipeline.is_in_progress() {
                    (PipelineState::InProgress, None, None)
                } else {
                    (PipelineState::Healthy, None, None)
                };

                let pipeline_url = Some(format!(
                    "https://bitbucket.org/{}/{}/pipelines/results/{}",
                    pipeline_config.workspace,
                    pipeline_config.repo_slug,
                    pipeline.build_number
                ));

                pipeline_statuses.push(PipelineStatusInfo {
                    workspace: pipeline_config.workspace.clone(),
                    project_key: pipeline_config.project_key.clone(),
                    project_name: pipeline_config.project_name.clone(),
                    repo_slug: pipeline_config.repo_slug.clone(),
                    repo_name: pipeline_config.repo_name.clone(),
                    state,
                    failure_reason,
                    pipeline_url,
                    stage_name,
                });
            }
            Ok(None) => {
                // No pipelines found for this repo - treat as unknown
                log::debug!(
                    "No pipelines found for {}/{}",
                    pipeline_config.workspace,
                    pipeline_config.repo_slug
                );
                pipeline_statuses.push(PipelineStatusInfo {
                    workspace: pipeline_config.workspace.clone(),
                    project_key: pipeline_config.project_key.clone(),
                    project_name: pipeline_config.project_name.clone(),
                    repo_slug: pipeline_config.repo_slug.clone(),
                    repo_name: pipeline_config.repo_name.clone(),
                    state: PipelineState::Unknown,
                    failure_reason: None,
                    pipeline_url: Some(format!(
                        "https://bitbucket.org/{}/{}/pipelines",
                        pipeline_config.workspace,
                        pipeline_config.repo_slug
                    )),
                    stage_name: None,
                });
            }
            Err(e) => {
                log::error!(
                    "Failed to check pipeline {}/{}: {}",
                    pipeline_config.workspace,
                    pipeline_config.repo_slug,
                    e
                );
                pipeline_statuses.push(PipelineStatusInfo {
                    workspace: pipeline_config.workspace.clone(),
                    project_key: pipeline_config.project_key.clone(),
                    project_name: pipeline_config.project_name.clone(),
                    repo_slug: pipeline_config.repo_slug.clone(),
                    repo_name: pipeline_config.repo_name.clone(),
                    state: PipelineState::Unknown,
                    failure_reason: Some(format!("Error: {}", e)),
                    pipeline_url: None,
                    stage_name: None,
                });
            }
        }
    }

    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
    OverallStatus::new(pipeline_statuses, timestamp)
}

/// Get the app password from config file
fn get_app_password(app_handle: &AppHandle) -> Option<String> {
    let config_dir = app_handle.path().app_config_dir().ok()?;
    let creds_path = config_dir.join(".credentials");

    if !creds_path.exists() {
        return None;
    }

    let encoded = std::fs::read_to_string(&creds_path).ok()?;
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    String::from_utf8(decoded).ok()
}

/// Listen for manual refresh triggers
pub fn setup_refresh_listener(app_handle: AppHandle) {
    let handle = app_handle.clone();
    app_handle.listen("trigger-refresh", move |_| {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            log::info!("Manual refresh triggered");
            check_pipelines_once(&handle).await;
        });
    });
}
