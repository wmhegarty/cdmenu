mod bitbucket;
mod commands;
mod config;
mod polling;
mod tray;

use config::AppState;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            log::info!("Setting up cdMenu...");

            // Set macOS to accessory mode (no dock icon)
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            // Load persisted config
            let initial_state = if let Some(config) = commands::load_config(app.handle()) {
                log::info!(
                    "Loaded config with {} monitored pipelines",
                    config.monitored_pipelines.len()
                );
                AppState::from_persisted(config)
            } else {
                log::info!("No existing config found, using defaults");
                AppState::new()
            };

            // Initialize shared state
            let app_state = Arc::new(Mutex::new(initial_state));
            app.manage(app_state);

            // Build system tray
            tray::build_tray(app)?;

            // Set up refresh listener
            polling::setup_refresh_listener(app.handle().clone());

            // Start background polling
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                polling::start_polling(app_handle).await;
            });

            log::info!("cdMenu setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_workspaces,
            commands::get_projects,
            commands::get_repositories,
            commands::get_repositories_by_project,
            commands::get_pipelines,
            commands::save_credentials,
            commands::get_credentials,
            commands::get_app_password,
            commands::save_monitored_pipelines,
            commands::get_monitored_pipelines,
            commands::get_pipeline_statuses,
            commands::set_polling_interval,
            commands::get_polling_interval,
            commands::trigger_refresh,
        ])
        .on_window_event(|window, event| {
            // Hide settings window on close instead of quitting
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
