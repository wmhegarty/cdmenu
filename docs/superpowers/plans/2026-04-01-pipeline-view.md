# Pipeline View Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new window that visualizes pipeline steps as a horizontal flow (A → B → C) for selected monitored pipelines, with step-level state coloring and persistence.

**Architecture:** New Tauri window with its own HTML/JS/CSS frontend. Backend adds types for step-level view data, new commands for persisting pipeline view selection, and extends the polling loop to fetch step details and emit a dedicated event. The pipeline-view frontend listens for that event to render.

**Tech Stack:** Tauri v2, Rust (backend), Vanilla HTML/CSS/JS (frontend), Bitbucket REST API

---

## File Structure

### New Files
- `src/pipeline-view.html` — Pipeline view window markup (selector + pipeline rows container)
- `src/pipeline-view.js` — Frontend logic: load saved pipelines, render step flows, listen for updates
- `src/pipeline-view.css` — Dark theme styling matching existing app, step pill colors, layout

### Modified Files
- `src-tauri/src/config.rs` — Add `PipelineViewPipeline`, `PipelineViewData`, `PipelineStepView`, `StepViewState` types; update `AppState` and `PersistedConfig`
- `src-tauri/src/commands.rs` — Add `get_pipeline_view_pipelines`, `save_pipeline_view_pipelines` commands
- `src-tauri/src/lib.rs` — Register new commands; handle close-to-hide for pipeline-view window
- `src-tauri/src/tray.rs` — Add "Pipeline View..." menu item in both `build_initial_menu` and `build_status_menu`; handle click
- `src-tauri/src/polling.rs` — After main status check, fetch step data for pipeline-view pipelines and emit `pipeline-view-updated` event
- `src-tauri/tauri.conf.json` — Add pipeline-view window config

---

### Task 1: Add Types to config.rs

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: Add the new types and update AppState/PersistedConfig**

Add these types after the existing `PipelineState` enum (around line 38):

```rust
/// Identifies a pipeline to show in the Pipeline View window
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineViewPipeline {
    pub workspace: String,
    pub repo_slug: String,
    pub repo_name: String,
    pub branch: Option<String>,
}

/// Step-level state for pipeline view rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepViewState {
    Completed,
    Running,
    Paused,
    Failed,
    Pending,
}

/// A single step in the pipeline view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepView {
    pub name: String,
    pub state: StepViewState,
}

/// Full pipeline data for the pipeline view window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineViewData {
    pub workspace: String,
    pub repo_slug: String,
    pub repo_name: String,
    pub branch: Option<String>,
    pub build_number: u32,
    pub steps: Vec<PipelineStepView>,
}
```

Add `pipeline_view_pipelines` field to `AppState`:

```rust
pub struct AppState {
    pub credentials: Option<Credentials>,
    pub monitored_pipelines: Vec<MonitoredPipeline>,
    pub polling_interval_seconds: u64,
    pub last_status: Option<OverallStatus>,
    pub pipeline_view_pipelines: Vec<PipelineViewPipeline>,
}
```

Update `AppState::new()` to initialize it:

```rust
pub fn new() -> Self {
    Self {
        credentials: None,
        monitored_pipelines: Vec::new(),
        polling_interval_seconds: 60,
        last_status: None,
        pipeline_view_pipelines: Vec::new(),
    }
}
```

Add `pipeline_view_pipelines` to `PersistedConfig`:

```rust
pub struct PersistedConfig {
    pub username: Option<String>,
    pub monitored_pipelines: Vec<MonitoredPipeline>,
    pub polling_interval_seconds: u64,
    #[serde(default)]
    pub pipeline_view_pipelines: Vec<PipelineViewPipeline>,
}
```

Update `to_persisted()`:

```rust
pub fn to_persisted(&self) -> PersistedConfig {
    PersistedConfig {
        username: self.credentials.as_ref().map(|c| c.username.clone()),
        monitored_pipelines: self.monitored_pipelines.clone(),
        polling_interval_seconds: self.polling_interval_seconds,
        pipeline_view_pipelines: self.pipeline_view_pipelines.clone(),
    }
}
```

Update `from_persisted()`:

```rust
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
        pipeline_view_pipelines: config.pipeline_view_pipelines,
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no new errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat: add pipeline view types and config persistence"
```

---

### Task 2: Add Commands for Pipeline View Persistence

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the two new commands in commands.rs**

Add these after the existing `trigger_refresh` command (around line 193), before the helper functions:

```rust
/// Get the list of pipelines shown in the pipeline view
#[command]
pub async fn get_pipeline_view_pipelines(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<PipelineViewPipeline>, String> {
    let state_guard = state.lock().await;
    Ok(state_guard.pipeline_view_pipelines.clone())
}

/// Save the list of pipelines shown in the pipeline view
#[command]
pub async fn save_pipeline_view_pipelines(
    app_handle: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    pipelines: Vec<PipelineViewPipeline>,
) -> Result<(), String> {
    {
        let mut state_guard = state.lock().await;
        state_guard.pipeline_view_pipelines = pipelines;
    }
    save_config_helper(&app_handle, &state).await
}
```

Update the import at the top of `commands.rs` to include `PipelineViewPipeline`:

```rust
use crate::config::{AppState, Credentials, MonitoredPipeline, OverallStatus, PersistedConfig, PipelineViewPipeline};
```

- [ ] **Step 2: Register the new commands in lib.rs**

Add to the `invoke_handler` list in `lib.rs` (after `commands::trigger_refresh`):

```rust
commands::get_pipeline_view_pipelines,
commands::save_pipeline_view_pipelines,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no new errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add commands for pipeline view persistence"
```

---

### Task 3: Add Tray Menu Item and Window Config

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the pipeline-view window to tauri.conf.json**

Add a second entry to the `app.windows` array, after the existing settings window:

```json
{
  "title": "Pipeline View",
  "label": "pipeline-view",
  "width": 600,
  "height": 400,
  "resizable": true,
  "visible": false,
  "center": true,
  "decorations": true,
  "url": "pipeline-view.html"
}
```

- [ ] **Step 2: Add "Pipeline View..." menu item to the initial menu in tray.rs**

In `build_initial_menu`, add a `pipeline_view` menu item between `refresh` and `settings`:

```rust
fn build_initial_menu<R: Runtime>(app: &tauri::App<R>) -> Result<Menu<R>, tauri::Error> {
    let status_item = MenuItem::with_id(app, "status", "Loading...", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh Now", true, None::<&str>)?;
    let pipeline_view = MenuItem::with_id(app, "pipeline_view", "Pipeline View...", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Menu::with_items(app, &[&status_item, &separator, &refresh, &pipeline_view, &settings, &quit])
}
```

- [ ] **Step 3: Add "Pipeline View..." to the dynamic status menu in tray.rs**

In `build_status_menu`, find where the action items are added (around line 240-245). Insert the pipeline_view item between refresh and settings:

```rust
    // Action items
    let refresh = MenuItem::with_id(app_handle, "refresh", "Refresh Now", true, None::<&str>)?;
    let pipeline_view = MenuItem::with_id(app_handle, "pipeline_view", "Pipeline View...", true, None::<&str>)?;
    let settings = MenuItem::with_id(app_handle, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;

    items.push(Box::new(refresh));
    items.push(Box::new(pipeline_view));
    items.push(Box::new(settings));
    items.push(Box::new(quit));
```

- [ ] **Step 4: Handle the "pipeline_view" click event in tray.rs**

In the `on_menu_event` handler inside `build_tray`, add a case for `"pipeline_view"` after `"settings"`:

```rust
"pipeline_view" => {
    log::info!("Opening pipeline view window");
    if let Some(window) = app.get_webview_window("pipeline-view") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
```

- [ ] **Step 5: Handle close-to-hide for pipeline-view window in lib.rs**

Update the `on_window_event` closure to also handle the pipeline-view window. Change the condition from checking just `"settings"` to checking both:

```rust
.on_window_event(|window, event| {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == "settings" || window.label() == "pipeline-view" {
            api.prevent_close();
            let _ = window.hide();
        }
    }
})
```

- [ ] **Step 6: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no new errors (the HTML file doesn't need to exist for check).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/tray.rs src-tauri/src/lib.rs
git commit -m "feat: add Pipeline View tray menu item and window config"
```

---

### Task 4: Add Step Fetching to Polling Loop

**Files:**
- Modify: `src-tauri/src/polling.rs`

- [ ] **Step 1: Add the step-fetching function**

Add this function after the existing `check_all_pipelines` function (after line 326):

```rust
use crate::config::{PipelineViewData, PipelineViewPipeline, PipelineStepView, StepViewState};

/// Fetch step-level data for pipelines in the pipeline view
async fn fetch_pipeline_view_data(
    username: &str,
    app_password: &str,
    view_pipelines: &[PipelineViewPipeline],
) -> Vec<PipelineViewData> {
    let client = BitbucketClient::new(username, app_password);
    let mut results = Vec::new();

    for vp in view_pipelines {
        let latest = match client
            .get_latest_pipeline(&vp.workspace, &vp.repo_slug, vp.branch.as_deref())
            .await
        {
            Ok(Some(pipeline)) => pipeline,
            Ok(None) => {
                log::debug!("No pipeline found for {}/{} in view", vp.workspace, vp.repo_slug);
                continue;
            }
            Err(e) => {
                log::error!("Failed to get pipeline for view {}/{}: {}", vp.workspace, vp.repo_slug, e);
                continue;
            }
        };

        let steps = match client
            .get_pipeline_steps(&vp.workspace, &vp.repo_slug, &latest.uuid)
            .await
        {
            Ok(steps) => steps,
            Err(e) => {
                log::error!("Failed to get steps for {}/{}: {}", vp.workspace, vp.repo_slug, e);
                continue;
            }
        };

        let step_views: Vec<PipelineStepView> = steps
            .iter()
            .map(|step| {
                let state = match step.state.as_ref().and_then(|s| s.name.as_deref()) {
                    Some("COMPLETED") => {
                        // Check if completed successfully or failed
                        if let Some(state_type) = step.state.as_ref().and_then(|s| s.state_type.as_deref()) {
                            if state_type.contains("error") || state_type.contains("failed") {
                                StepViewState::Failed
                            } else {
                                StepViewState::Completed
                            }
                        } else {
                            StepViewState::Completed
                        }
                    }
                    Some("IN_PROGRESS") => StepViewState::Running,
                    Some("PENDING") => {
                        // Check if it's paused (waiting for manual trigger) vs just pending
                        if let Some(state_type) = step.state.as_ref().and_then(|s| s.state_type.as_deref()) {
                            if state_type.contains("paused") || state_type.contains("halted") {
                                StepViewState::Paused
                            } else {
                                StepViewState::Pending
                            }
                        } else {
                            StepViewState::Pending
                        }
                    }
                    _ => StepViewState::Pending,
                };

                PipelineStepView {
                    name: step.name.clone().unwrap_or_else(|| "Step".to_string()),
                    state,
                }
            })
            .collect();

        results.push(PipelineViewData {
            workspace: vp.workspace.clone(),
            repo_slug: vp.repo_slug.clone(),
            repo_name: vp.repo_name.clone(),
            branch: latest.target.ref_name.clone(),
            build_number: latest.build_number,
            steps: step_views,
        });
    }

    results
}
```

- [ ] **Step 2: Call it from check_pipelines_once and emit the event**

In `check_pipelines_once`, after the line `let _ = app_handle.emit("status-updated", &status);` (line 204), add:

```rust
    // Fetch and emit pipeline view data if any pipelines are selected
    let view_pipelines = {
        let state: tauri::State<Arc<Mutex<AppState>>> = app_handle.state();
        let state_guard = state.lock().await;
        state_guard.pipeline_view_pipelines.clone()
    };

    if !view_pipelines.is_empty() {
        let view_data = fetch_pipeline_view_data(
            &credentials.username,
            &app_password,
            &view_pipelines,
        )
        .await;
        let _ = app_handle.emit("pipeline-view-updated", &view_data);
    }
```

Note: `credentials` and `app_password` are already in scope from earlier in the function.

- [ ] **Step 3: Update the import at the top of polling.rs**

Change the config import line to include the new types:

```rust
use crate::config::{AppState, MonitoredPipeline, OverallStatus, PipelineState, PipelineStatusInfo, PipelineViewData, PipelineViewPipeline, PipelineStepView, StepViewState};
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles with no new errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/polling.rs
git commit -m "feat: fetch pipeline step data and emit pipeline-view-updated event"
```

---

### Task 5: Create Pipeline View Frontend

**Files:**
- Create: `src/pipeline-view.html`
- Create: `src/pipeline-view.css`
- Create: `src/pipeline-view.js`

- [ ] **Step 1: Create pipeline-view.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Pipeline View</title>
    <link rel="stylesheet" href="pipeline-view.css">
</head>
<body>
    <div id="app">
        <div id="pipeline-selector">
            <select id="pipeline-select">
                <option value="">Select a pipeline to add...</option>
            </select>
            <button type="button" id="add-btn" disabled>Add</button>
        </div>

        <div id="pipeline-container">
            <p id="empty-message">No pipelines added. Use the dropdown above to add one.</p>
        </div>
    </div>

    <script type="module" src="pipeline-view.js"></script>
</body>
</html>
```

- [ ] **Step 2: Create pipeline-view.css**

```css
:root {
    --bg-primary: #0f0f1a;
    --bg-card: #1a1a2e;
    --bg-step-pending: #2c2c3e;
    --text-primary: #e0e0e0;
    --text-secondary: #888;
    --text-pending: #666;
    --color-green: #2ecc71;
    --color-orange: #f39c12;
    --color-blue: #3498db;
    --color-red: #e74c3c;
    --border-radius: 8px;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
    min-height: 100vh;
}

#app {
    padding: 16px;
}

/* Pipeline Selector */
#pipeline-selector {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
}

#pipeline-selector select {
    flex: 1;
    background: var(--bg-card);
    color: var(--text-primary);
    border: 1px solid #333;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 13px;
}

#pipeline-selector select:focus {
    outline: none;
    border-color: var(--color-blue);
}

#pipeline-selector button {
    background: var(--color-blue);
    color: #fff;
    border: none;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
}

#pipeline-selector button:hover:not(:disabled) {
    opacity: 0.9;
}

#pipeline-selector button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

/* Pipeline Container */
#pipeline-container {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

#empty-message {
    color: var(--text-secondary);
    text-align: center;
    padding: 40px 20px;
    font-style: italic;
}

/* Pipeline Row */
.pipeline-row {
    background: var(--bg-card);
    border-radius: var(--border-radius);
    padding: 14px;
    border-left: 3px solid var(--color-green);
}

.pipeline-row.state-failed {
    border-left-color: var(--color-red);
}

.pipeline-row.state-running {
    border-left-color: var(--color-orange);
}

.pipeline-row.state-paused {
    border-left-color: var(--color-blue);
}

/* Pipeline Header */
.pipeline-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
}

.pipeline-info .pipeline-name {
    color: #ccc;
    font-size: 13px;
    font-weight: 600;
}

.pipeline-info .pipeline-meta {
    color: var(--text-secondary);
    font-size: 11px;
    margin-left: 8px;
}

.remove-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 18px;
    padding: 0 4px;
    line-height: 1;
}

.remove-btn:hover {
    color: var(--color-red);
}

/* Step Flow */
.step-flow {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
}

.step-arrow {
    color: #444;
    font-size: 14px;
    user-select: none;
}

/* Step Pills */
.step-pill {
    padding: 5px 12px;
    border-radius: 5px;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
}

.step-pill.completed {
    background: var(--color-green);
    color: #000;
}

.step-pill.running {
    background: var(--color-orange);
    color: #000;
    box-shadow: 0 0 8px rgba(243, 156, 18, 0.3);
    animation: glow 1.5s ease-in-out infinite;
}

.step-pill.paused {
    background: var(--color-blue);
    color: #fff;
    box-shadow: 0 0 8px rgba(52, 152, 219, 0.3);
}

.step-pill.failed {
    background: var(--color-red);
    color: #fff;
}

.step-pill.pending {
    background: var(--bg-step-pending);
    color: var(--text-pending);
}

@keyframes glow {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
}

/* Loading state for pipeline rows */
.pipeline-row.loading .step-flow {
    color: var(--text-secondary);
    font-style: italic;
    font-size: 12px;
}

/* Scrollbar */
::-webkit-scrollbar {
    width: 8px;
}

::-webkit-scrollbar-track {
    background: var(--bg-primary);
}

::-webkit-scrollbar-thumb {
    background: var(--text-secondary);
    border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
    background: var(--text-primary);
}
```

- [ ] **Step 3: Create pipeline-view.js**

```javascript
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let viewPipelines = []; // Persisted list of pipelines in the view
let monitoredPipelines = []; // All monitored pipelines (for dropdown)
let latestViewData = []; // Latest step data from polling

// DOM Elements
const pipelineSelect = document.getElementById('pipeline-select');
const addBtn = document.getElementById('add-btn');
const pipelineContainer = document.getElementById('pipeline-container');
const emptyMessage = document.getElementById('empty-message');

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    await loadViewPipelines();
    await loadMonitoredPipelines();
    populateDropdown();
    setupEventListeners();
    listenForUpdates();
    // Trigger a refresh to get initial data
    try {
        await invoke('trigger_refresh');
    } catch (e) {
        console.error('Failed to trigger initial refresh:', e);
    }
});

async function loadViewPipelines() {
    try {
        viewPipelines = await invoke('get_pipeline_view_pipelines');
        renderPipelines();
    } catch (e) {
        console.error('Failed to load view pipelines:', e);
    }
}

async function loadMonitoredPipelines() {
    try {
        monitoredPipelines = await invoke('get_monitored_pipelines');
    } catch (e) {
        console.error('Failed to load monitored pipelines:', e);
    }
}

function populateDropdown() {
    pipelineSelect.innerHTML = '<option value="">Select a pipeline to add...</option>';

    // Filter out pipelines already in the view
    const available = monitoredPipelines.filter(mp =>
        !viewPipelines.some(vp =>
            vp.workspace === mp.workspace && vp.repo_slug === mp.repo_slug
        )
    );

    available.forEach(mp => {
        const option = document.createElement('option');
        option.value = JSON.stringify({
            workspace: mp.workspace,
            repo_slug: mp.repo_slug,
            repo_name: mp.repo_name,
            branch: mp.branch
        });
        const name = mp.repo_name || mp.repo_slug;
        const branch = mp.branch ? ` (${mp.branch})` : '';
        option.textContent = `${mp.workspace} / ${name}${branch}`;
        pipelineSelect.appendChild(option);
    });
}

function setupEventListeners() {
    pipelineSelect.addEventListener('change', () => {
        addBtn.disabled = !pipelineSelect.value;
    });

    addBtn.addEventListener('click', async () => {
        if (!pipelineSelect.value) return;

        const pipeline = JSON.parse(pipelineSelect.value);
        viewPipelines.push(pipeline);

        try {
            await invoke('save_pipeline_view_pipelines', { pipelines: viewPipelines });
            populateDropdown();
            renderPipelines();
            // Trigger refresh to get step data for the new pipeline
            await invoke('trigger_refresh');
        } catch (e) {
            console.error('Failed to save:', e);
            viewPipelines.pop();
        }
    });
}

async function removePipeline(index) {
    viewPipelines.splice(index, 1);
    try {
        await invoke('save_pipeline_view_pipelines', { pipelines: viewPipelines });
        populateDropdown();
        renderPipelines();
    } catch (e) {
        console.error('Failed to remove:', e);
    }
}

function listenForUpdates() {
    listen('pipeline-view-updated', (event) => {
        latestViewData = event.payload;
        renderPipelines();
    });
}

function renderPipelines() {
    pipelineContainer.innerHTML = '';

    if (viewPipelines.length === 0) {
        pipelineContainer.innerHTML = '<p id="empty-message">No pipelines added. Use the dropdown above to add one.</p>';
        return;
    }

    viewPipelines.forEach((vp, index) => {
        const viewData = latestViewData.find(d =>
            d.workspace === vp.workspace && d.repo_slug === vp.repo_slug
        );

        const row = document.createElement('div');
        row.className = 'pipeline-row';

        if (viewData) {
            // Determine overall state for left border
            const overallState = getOverallState(viewData.steps);
            row.classList.add(`state-${overallState}`);

            const branch = viewData.branch || 'default';
            row.innerHTML = `
                <div class="pipeline-header">
                    <div class="pipeline-info">
                        <span class="pipeline-name">${viewData.repo_name || viewData.repo_slug}</span>
                        <span class="pipeline-meta">${branch} · #${viewData.build_number}</span>
                    </div>
                    <button class="remove-btn" title="Remove">×</button>
                </div>
                <div class="step-flow">
                    ${renderSteps(viewData.steps)}
                </div>
            `;
        } else {
            // No data yet — show loading state
            row.classList.add('loading');
            row.innerHTML = `
                <div class="pipeline-header">
                    <div class="pipeline-info">
                        <span class="pipeline-name">${vp.repo_name || vp.repo_slug}</span>
                        <span class="pipeline-meta">Loading...</span>
                    </div>
                    <button class="remove-btn" title="Remove">×</button>
                </div>
                <div class="step-flow">Waiting for data...</div>
            `;
        }

        row.querySelector('.remove-btn').addEventListener('click', () => removePipeline(index));
        pipelineContainer.appendChild(row);
    });
}

function renderSteps(steps) {
    return steps.map((step, i) => {
        const prefix = step.state === 'Paused' ? '⏸ ' : '';
        const pill = `<span class="step-pill ${step.state.toLowerCase()}">${prefix}${step.name}</span>`;
        const arrow = i < steps.length - 1 ? '<span class="step-arrow">→</span>' : '';
        return pill + arrow;
    }).join('');
}

function getOverallState(steps) {
    // Priority: failed > running > paused > completed
    if (steps.some(s => s.state === 'Failed')) return 'failed';
    if (steps.some(s => s.state === 'Running')) return 'running';
    if (steps.some(s => s.state === 'Paused')) return 'paused';
    return 'completed';
}
```

- [ ] **Step 4: Verify the full app compiles and runs**

Run: `cd src-tauri && cargo check`
Expected: Compiles. The app can now be tested with `cargo tauri dev`.

- [ ] **Step 5: Commit**

```bash
git add src/pipeline-view.html src/pipeline-view.css src/pipeline-view.js
git commit -m "feat: add pipeline view frontend with step visualization"
```

---

### Task 6: Manual Testing and Polish

**Files:**
- Potentially any of the above files for fixes

- [ ] **Step 1: Run the app in dev mode**

Run: `cargo tauri dev`

Test the following:
1. Click the tray icon — verify "Pipeline View..." appears between "Refresh Now" and "Settings..."
2. Click "Pipeline View..." — verify the window opens
3. Close the pipeline view window — verify it hides (not quits the app)
4. Reopen it from the tray — verify it reappears
5. Add a pipeline from the dropdown — verify it appears as a row
6. Wait for a polling cycle — verify step pills appear with correct colors
7. Remove a pipeline — verify it disappears and reappears in the dropdown
8. Close and reopen the window — verify the added pipelines are still there
9. Quit and restart the app — verify persistence (added pipelines survive restart)

- [ ] **Step 2: Fix any issues found during testing**

Address any visual, functional, or data issues found.

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "feat: pipeline view window - complete implementation"
```
