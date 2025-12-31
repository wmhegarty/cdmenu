use crate::config::{OverallStatus, PipelineState};
use tauri::{
    image::Image,
    menu::{IconMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Runtime,
};
use std::sync::RwLock;
use std::collections::HashMap;

// Store pipeline URLs for click handling
static PIPELINE_URLS: RwLock<Option<HashMap<String, String>>> = RwLock::new(None);

pub const TRAY_ID: &str = "main";

// Embed the tray icons at compile time
const ICON_GREEN: &[u8] = include_bytes!("../icons/tray-green.png");
const ICON_RED: &[u8] = include_bytes!("../icons/tray-red.png");
const ICON_GRAY: &[u8] = include_bytes!("../icons/tray-gray.png");

// Menu icons (smaller versions)
const MENU_ICON_GREEN: &[u8] = include_bytes!("../icons/menu-green.png");
const MENU_ICON_RED: &[u8] = include_bytes!("../icons/menu-red.png");
const MENU_ICON_GRAY: &[u8] = include_bytes!("../icons/menu-gray.png");
const MENU_ICON_BLUE: &[u8] = include_bytes!("../icons/menu-blue.png");

/// Tray status indicator
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TrayStatus {
    /// All pipelines healthy (green)
    Green,
    /// At least one pipeline failed (red)
    Red,
    /// Loading or no pipelines configured (gray)
    Gray,
}

/// Build the system tray with menu
pub fn build_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), tauri::Error> {
    // Create initial menu (will be updated dynamically)
    let menu = build_initial_menu(app)?;

    // Load initial gray icon
    let icon = Image::from_bytes(ICON_GRAY)?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("cdMenu - Loading...")
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "refresh" => {
                    log::info!("Refresh requested from tray menu");
                    let _ = app.emit("trigger-refresh", ());
                }
                "settings" => {
                    log::info!("Opening settings window");
                    if let Some(window) = app.get_webview_window("settings") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    log::info!("Quit requested from tray menu");
                    app.exit(0);
                }
                _ => {
                    // Check if it's a pipeline click
                    if id.starts_with("pipeline_") {
                        if let Ok(urls) = PIPELINE_URLS.read() {
                            if let Some(url_map) = urls.as_ref() {
                                if let Some(url) = url_map.get(id) {
                                    log::info!("Opening pipeline URL: {}", url);
                                    let _ = open::that(url);
                                }
                            }
                        }
                    }
                }
            }
        })
        .on_tray_icon_event(|_tray, _event| {
            // Menu shows on click, no additional handling needed
        })
        .build(app)?;

    Ok(())
}

/// Build the initial menu before any status is available
fn build_initial_menu<R: Runtime>(app: &tauri::App<R>) -> Result<Menu<R>, tauri::Error> {
    let status_item = MenuItem::with_id(app, "status", "Loading...", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&status_item, &separator, &refresh, &settings, &quit])
}

/// Update the tray menu with current pipeline status
pub fn update_tray_menu(app_handle: &AppHandle, status: Option<&OverallStatus>) {
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_status_menu(app_handle, status) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

/// Build menu with pipeline status grouped by project
fn build_status_menu(app_handle: &AppHandle, status: Option<&OverallStatus>) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let mut items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = Vec::new();
    let mut url_map: HashMap<String, String> = HashMap::new();

    match status {
        Some(s) => {
            // Group pipelines by project (use project_name, fallback to workspace)
            let mut projects: Vec<String> = Vec::new();
            for pipeline in &s.pipeline_statuses {
                let project = pipeline.project_name.clone()
                    .unwrap_or_else(|| pipeline.workspace.clone());
                if !projects.contains(&project) {
                    projects.push(project);
                }
            }

            for (proj_idx, project) in projects.iter().enumerate() {
                // Add project header
                let proj_header = MenuItem::with_id(
                    app_handle,
                    format!("proj_header_{}", proj_idx),
                    project.to_uppercase(),
                    false,
                    None::<&str>,
                )?;
                items.push(Box::new(proj_header));

                // Add pipelines for this project
                for (i, pipeline) in s.pipeline_statuses.iter().enumerate() {
                    let pipeline_project = pipeline.project_name.clone()
                        .unwrap_or_else(|| pipeline.workspace.clone());
                    if &pipeline_project != project {
                        continue;
                    }

                    let name = if pipeline.repo_name.is_empty() {
                        &pipeline.repo_slug
                    } else {
                        &pipeline.repo_name
                    };

                    let (icon_bytes, status_text) = match pipeline.state {
                        PipelineState::Healthy => (MENU_ICON_GREEN, String::new()),
                        PipelineState::Failed => (MENU_ICON_RED, " - FAILED".to_string()),
                        PipelineState::InProgress => (MENU_ICON_BLUE, " - running".to_string()),
                        PipelineState::Paused => {
                            let stage = pipeline.stage_name.as_deref().unwrap_or("paused");
                            (MENU_ICON_GREEN, format!(" - ({})", stage))
                        }
                        PipelineState::Unknown => (MENU_ICON_GRAY, String::new()),
                    };

                    let menu_id = format!("pipeline_{}", i);
                    let has_url = pipeline.pipeline_url.is_some();

                    // Store URL for click handling
                    if let Some(ref url) = pipeline.pipeline_url {
                        url_map.insert(menu_id.clone(), url.clone());
                    }

                    // Create menu item with icon (indented with spaces)
                    let display_text = format!("  {}{}", name, status_text);
                    if let Ok(icon) = Image::from_bytes(icon_bytes) {
                        let item = IconMenuItem::with_id(
                            app_handle,
                            &menu_id,
                            &display_text,
                            has_url,
                            Some(icon),
                            None::<&str>,
                        )?;
                        items.push(Box::new(item));
                    } else {
                        let item = MenuItem::with_id(
                            app_handle,
                            &menu_id,
                            &display_text,
                            has_url,
                            None::<&str>,
                        )?;
                        items.push(Box::new(item));
                    }
                }

                // Add separator between projects (but not after the last one)
                if proj_idx < projects.len() - 1 {
                    let sep = PredefinedMenuItem::separator(app_handle)?;
                    items.push(Box::new(sep));
                }
            }

            // Separator before last checked
            let sep1 = PredefinedMenuItem::separator(app_handle)?;
            items.push(Box::new(sep1));

            // Add last checked time
            let last_checked = MenuItem::with_id(
                app_handle,
                "last_checked",
                format!("Last checked: {}", s.last_checked),
                false,
                None::<&str>,
            )?;
            items.push(Box::new(last_checked));
        }
        None => {
            let no_status = MenuItem::with_id(
                app_handle,
                "no_status",
                "No pipelines configured",
                false,
                None::<&str>,
            )?;
            items.push(Box::new(no_status));
        }
    }

    // Store URLs globally for click handler
    if let Ok(mut urls) = PIPELINE_URLS.write() {
        *urls = Some(url_map);
    }

    // Separator
    let separator = PredefinedMenuItem::separator(app_handle)?;
    items.push(Box::new(separator));

    // Action items
    let refresh = MenuItem::with_id(app_handle, "refresh", "Refresh Now", true, None::<&str>)?;
    let settings = MenuItem::with_id(app_handle, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;

    items.push(Box::new(refresh));
    items.push(Box::new(settings));
    items.push(Box::new(quit));

    // Build menu from items
    let item_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items.iter().map(|b| b.as_ref()).collect();
    Menu::with_items(app_handle, &item_refs)
}

/// Update the tray icon based on status
pub fn update_tray_icon(app_handle: &AppHandle, status: TrayStatus) {
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        let icon_bytes = match status {
            TrayStatus::Green => ICON_GREEN,
            TrayStatus::Red => ICON_RED,
            TrayStatus::Gray => ICON_GRAY,
        };

        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

/// Update the tray tooltip
pub fn update_tray_tooltip(app_handle: &AppHandle, tooltip: &str) {
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip));
    }
}
