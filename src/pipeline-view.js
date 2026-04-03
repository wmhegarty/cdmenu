const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;

// State
let viewPipelines = []; // Persisted list of pipelines in the view
let monitoredPipelines = []; // All monitored pipelines (for dropdown)
let latestViewData = []; // Latest step data from polling
let hasResized = false; // Only auto-size window once per session

// DOM Elements
const pipelineSelect = document.getElementById('pipeline-select');
const addBtn = document.getElementById('add-btn');
const pipelineContainer = document.getElementById('pipeline-container');
const emptyMessage = document.getElementById('empty-message');

let refreshInterval = null;

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    await loadViewPipelines();
    await loadMonitoredPipelines();
    populateDropdown();
    setupEventListeners();
    listenForUpdates();
    await fetchViewData();
    startRefreshInterval();
});

// Fetch fresh data every time the window becomes visible
document.addEventListener('visibilitychange', async () => {
    if (!document.hidden) {
        await loadViewPipelines();
        await loadMonitoredPipelines();
        populateDropdown();
        await fetchViewData();
        startRefreshInterval();
    } else {
        stopRefreshInterval();
    }
});

function startRefreshInterval() {
    stopRefreshInterval();
    // Refresh every 15 seconds while visible
    refreshInterval = setInterval(fetchViewData, 15000);
}

function stopRefreshInterval() {
    if (refreshInterval) {
        clearInterval(refreshInterval);
        refreshInterval = null;
    }
}

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
            // Fetch step data for the new pipeline
            await fetchViewData();
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

async function fetchViewData() {
    if (viewPipelines.length === 0) return;
    try {
        latestViewData = await invoke('get_pipeline_view_data');
        renderPipelines();
    } catch (e) {
        console.error('Failed to fetch view data:', e);
    }
}

function listenForUpdates() {
    listen('pipeline-view-updated', (event) => {
        latestViewData = event.payload;
        renderPipelines();
    });
    // Also refresh when the main status poll completes (e.g. from tray "Refresh Now")
    listen('status-updated', async () => {
        await fetchViewData();
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
            const shortHash = viewData.commit_hash ? viewData.commit_hash.substring(0, 7) : '';
            const commitLink = shortHash
                ? `<a class="commit-link" href="#" data-url="https://bitbucket.org/${viewData.workspace}/${viewData.repo_slug}/commits/${viewData.commit_hash}">${shortHash}</a>`
                : '';
            const metaParts = [branch, `#${viewData.build_number}`, commitLink].filter(Boolean);
            row.innerHTML = `
                <div class="pipeline-header">
                    <div class="pipeline-info">
                        <span class="pipeline-name">${viewData.repo_name || viewData.repo_slug}</span>
                        <span class="pipeline-meta">${metaParts.join(' · ')}</span>
                    </div>
                    <button class="remove-btn" title="Remove">×</button>
                </div>
                <div class="step-flow"></div>
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

        // Attach step click/context handlers if we have data
        if (viewData) {
            const stepFlow = row.querySelector('.step-flow');
            renderSteps(stepFlow, viewData);
        }

        // Commit link click handler
        const commitLink = row.querySelector('.commit-link');
        if (commitLink) {
            commitLink.addEventListener('click', (e) => {
                e.preventDefault();
                invoke('open_url', { url: commitLink.dataset.url }).catch(err => console.error('Failed to open URL:', err));
            });
        }

        row.querySelector('.remove-btn').addEventListener('click', () => removePipeline(index));
        pipelineContainer.appendChild(row);
    });

    // Auto-size window only once per session
    if (!hasResized) {
        hasResized = true;
        requestAnimationFrame(() => resizeWindowToFit());
    }
}

async function resizeWindowToFit() {
    try {
        const win = getCurrentWindow();

        const screenWidth = window.screen.availWidth;
        const screenHeight = window.screen.availHeight;
        const maxWidth = screenWidth - 40;
        const maxHeight = screenHeight - 40;

        // Measure each step-flow's natural single-line width using a hidden measurement div
        const stepFlows = document.querySelectorAll('.step-flow');
        let widestRow = 0;
        stepFlows.forEach(flow => {
            // Clone the flow, set nowrap, measure offscreen
            const clone = flow.cloneNode(true);
            clone.style.cssText = 'display:inline-flex;flex-wrap:nowrap;position:absolute;visibility:hidden;left:-9999px;gap:6px';
            document.body.appendChild(clone);
            const w = clone.offsetWidth;
            document.body.removeChild(clone);
            if (w > widestRow) widestRow = w;
        });

        // Add: pipeline-row padding (14*2) + border (3) + app padding (16*2) + remove btn (~30) + safety
        const neededWidth = widestRow + 28 + 3 + 32 + 30 + 40;
        const finalWidth = Math.max(Math.min(neededWidth, maxWidth), 600);

        // Set width first so content reflows at the correct width
        await win.setSize(new LogicalSize(finalWidth, maxHeight));

        // Wait several frames for full reflow
        await new Promise(r => setTimeout(r, 100));

        const contentHeight = document.documentElement.scrollHeight;
        const finalHeight = Math.max(Math.min(contentHeight + 40, maxHeight), 200);

        await win.setSize(new LogicalSize(finalWidth, finalHeight));
        await win.center();
    } catch (e) {
        // silently fail — window stays at default size
    }
}

function renderSteps(container, viewData) {
    viewData.steps.forEach((step, i) => {
        const prefix = step.state === 'Paused' ? '⏸ ' : '';
        const pill = document.createElement('span');
        pill.className = `step-pill ${step.state.toLowerCase()}`;
        pill.textContent = `${prefix}${step.name}`;
        pill.style.cursor = 'pointer';

        // Click: open step in Bitbucket
        const stepUrl = `https://bitbucket.org/${viewData.workspace}/${viewData.repo_slug}/pipelines/results/${viewData.build_number}/steps/${encodeURIComponent(step.uuid)}`;
        pill.addEventListener('click', () => {
            invoke('open_url', { url: stepUrl }).catch(e => console.error('Failed to open URL:', e));
        });

        // Right-click paused step: open in Bitbucket where the Run button is
        if (step.state === 'Paused') {
            pill.title = 'Click to open in Bitbucket (manual trigger)';
        }

        container.appendChild(pill);

        if (i < viewData.steps.length - 1) {
            const arrow = document.createElement('span');
            arrow.className = 'step-arrow';
            arrow.textContent = '→';
            container.appendChild(arrow);
        }
    });
}


function showError(message) {
    const existing = document.querySelector('.error-toast');
    if (existing) existing.remove();

    const toast = document.createElement('div');
    toast.className = 'error-toast';
    toast.textContent = message;
    document.body.appendChild(toast);
    setTimeout(() => toast.remove(), 5000);
}

function getOverallState(steps) {
    // Priority: failed > running > paused > completed
    if (steps.some(s => s.state === 'Failed')) return 'failed';
    if (steps.some(s => s.state === 'Running')) return 'running';
    if (steps.some(s => s.state === 'Paused')) return 'paused';
    return 'completed';
}
