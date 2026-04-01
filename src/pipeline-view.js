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
