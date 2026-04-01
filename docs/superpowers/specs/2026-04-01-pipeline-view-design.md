# Pipeline View Window — Design Spec

## Overview

A new window accessible from the tray menu that visualizes pipeline steps as a horizontal flow (A → B → C) for selected monitored pipelines. Shows the latest run only, with step-level state visible at a glance.

## Window

- **Title**: "Pipeline View"
- **Label**: `pipeline-view`
- **Size**: 600 x 400, resizable
- **Behavior**: Hidden by default. Opened via tray menu "Pipeline View..." item. Close hides (same pattern as settings window).
- **URL**: `pipeline-view.html` (new frontend page)

## Tray Menu Integration

Add "Pipeline View..." menu item between "Refresh Now" and "Settings..." in the tray menu. Clicking it shows/focuses the pipeline-view window.

## Layout

Stacked rows. Each pipeline is a full-width horizontal row containing:

1. **Header line**: Repo name (bold), branch + build number (dimmed), × remove button (right-aligned)
2. **Step flow**: Horizontal sequence of step pills connected by → arrows, wrapping if needed
3. **Left border**: Color matches the pipeline's overall state for quick scanning

Pipelines stack vertically. Window scrolls if content exceeds height.

## Pipeline Selector

At the top of the window, a dropdown + "Add" button:

- Dropdown lists all monitored pipelines from the app's config (workspace/repo/branch)
- Only pipelines not already in the view are shown
- Clicking "Add" adds the pipeline to the view and persists the selection

## Step Colors

| State | Color | Description |
|-------|-------|-------------|
| Completed/Successful | Green (#2ecc71) | Step finished successfully |
| Running | Orange (#f39c12) | Step currently executing, subtle glow |
| Paused | Blue (#3498db) | Waiting for manual approval, ⏸ icon prefix |
| Failed | Red (#e74c3c) | Step failed |
| Pending | Dark gray (#2c2c3e, text #666) | Not started yet |

## Left Border Colors

Matches the overall pipeline state:
- Green: all steps completed successfully
- Orange: a step is currently running
- Blue: a step is paused/awaiting approval
- Red: a step has failed
- Priority: Red > Orange > Blue > Green

## Data Flow

### New Tauri Command: `get_pipeline_steps_for_view`

Takes a list of pipeline identifiers (workspace, repo_slug, branch). For each:

1. Calls `get_latest_pipeline(workspace, repo_slug, branch)` to get the latest pipeline run
2. Calls `get_pipeline_steps(workspace, repo_slug, pipeline_uuid)` to get step details
3. Returns a `PipelineViewData` struct per pipeline containing:
   - repo_name, workspace, branch, build_number
   - Vec of steps, each with: name, state (completed/running/paused/failed/pending)

### Polling Integration

The existing polling loop already runs on a configurable interval. After `check_pipelines_once` completes, emit a new event `pipeline-view-updated` with the step-level data for pipelines in the view. The pipeline-view frontend listens for this event.

To avoid unnecessary API calls, only fetch step data when the pipeline-view window has pipelines selected. The Rust backend checks the persisted `pipeline_view_pipelines` list — if empty, skip the step fetches.

### New Event: `pipeline-view-updated`

Payload: `Vec<PipelineViewData>` where:

```rust
struct PipelineViewData {
    workspace: String,
    repo_slug: String,
    repo_name: String,
    branch: Option<String>,
    build_number: u32,
    steps: Vec<PipelineStepView>,
}

struct PipelineStepView {
    name: String,
    state: StepViewState,
}

enum StepViewState {
    Completed,
    Running,
    Paused,
    Failed,
    Pending,
}
```

## Persistence

### Config Changes

Add to `PersistedConfig`:

```rust
pub pipeline_view_pipelines: Vec<PipelineViewPipeline>,
```

Where `PipelineViewPipeline` identifies a pipeline by workspace + repo_slug + branch (same fields as `MonitoredPipeline` but a separate list for what's shown in the view).

Saved to `config.json` alongside existing config. Loaded on app startup.

### New Tauri Commands

- `get_pipeline_view_pipelines` — returns the persisted list of pipelines in the view
- `save_pipeline_view_pipelines` — saves the list (called when user adds/removes a pipeline)

## Frontend

New files:
- `src/pipeline-view.html` — page markup
- `src/pipeline-view.js` — logic for rendering steps, managing the selector, listening for events
- `src/pipeline-view.css` — styling (dark theme matching existing app)

The frontend:
1. On load, calls `get_pipeline_view_pipelines` to restore saved selection
2. Calls `get_monitored_pipelines` to populate the dropdown (filtering out already-added ones)
3. Listens for `pipeline-view-updated` events to re-render step flows
4. On add/remove, calls `save_pipeline_view_pipelines` to persist

## Tauri Config Changes

Add a second window entry in `tauri.conf.json`:

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

## Files to Create/Modify

### New Files
- `src/pipeline-view.html`
- `src/pipeline-view.js`
- `src/pipeline-view.css`

### Modified Files
- `src-tauri/tauri.conf.json` — add pipeline-view window config
- `src-tauri/src/tray.rs` — add "Pipeline View..." menu item, handle click
- `src-tauri/src/config.rs` — add `PipelineViewPipeline`, update `PersistedConfig` and `AppState`
- `src-tauri/src/commands.rs` — add new commands for pipeline view data
- `src-tauri/src/lib.rs` — register new commands in invoke_handler
- `src-tauri/src/polling.rs` — fetch step data and emit `pipeline-view-updated` event
- `src-tauri/src/bitbucket/types.rs` — add `PipelineViewData` and `PipelineStepView` types (if not using existing step types directly)
