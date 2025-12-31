const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Opener plugin - try both v2 namespaces
const opener = window.__TAURI_PLUGIN_OPENER__ || window.__TAURI__?.opener;
const open = opener?.open || (() => window.open(arguments[0], '_blank'));

// State
let currentUsername = '';
let currentAppPassword = '';
let workspaces = [];
let projects = [];
let repositories = [];
let monitoredPipelines = [];
let currentWorkspace = '';

// DOM Elements
const authForm = document.getElementById('auth-form');
const usernameInput = document.getElementById('username');
const appPasswordInput = document.getElementById('app-password');
const saveAuthBtn = document.getElementById('save-auth-btn');
const authStatus = document.getElementById('auth-status');
const workspaceSelect = document.getElementById('workspace-select');
const projectSelect = document.getElementById('project-select');
const repoSelect = document.getElementById('repo-select');
const addPipelineBtn = document.getElementById('add-pipeline-btn');
const pipelineList = document.getElementById('pipeline-list');
const pollingIntervalInput = document.getElementById('polling-interval');
const statusDetails = document.getElementById('status-details');
const statusIndicator = document.getElementById('status-indicator');
const createPasswordLink = document.getElementById('create-password-link');

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    await loadSavedCredentials();
    await loadMonitoredPipelines();
    await loadPollingInterval();
    await loadCurrentStatus();
    setupEventListeners();
    listenForStatusUpdates();
});

async function loadSavedCredentials() {
    try {
        const username = await invoke('get_credentials');
        if (username) {
            usernameInput.value = username;
            currentUsername = username;
            // Try to get password to check if we have valid credentials
            const password = await invoke('get_app_password');
            if (password) {
                currentAppPassword = password;
                appPasswordInput.placeholder = '••••••••••••••••';
                showAuthStatus('Credentials saved', 'success');
                await loadWorkspaces();
            }
        }
    } catch (e) {
        console.error('Failed to load credentials:', e);
    }
}

async function loadMonitoredPipelines() {
    try {
        monitoredPipelines = await invoke('get_monitored_pipelines');
        renderPipelineList();
    } catch (e) {
        console.error('Failed to load monitored pipelines:', e);
    }
}

async function loadPollingInterval() {
    try {
        const interval = await invoke('get_polling_interval');
        pollingIntervalInput.value = interval;
    } catch (e) {
        console.error('Failed to load polling interval:', e);
    }
}

async function loadCurrentStatus() {
    try {
        const status = await invoke('get_pipeline_statuses');
        if (status) {
            updateStatusDisplay(status);
        }
    } catch (e) {
        console.error('Failed to load status:', e);
    }
}

function setupEventListeners() {
    // Auth form submission
    authForm.addEventListener('submit', async (e) => {
        e.preventDefault();
        await saveCredentials();
    });

    // Workspace selection - loads projects
    workspaceSelect.addEventListener('change', async () => {
        const workspace = workspaceSelect.value;
        currentWorkspace = workspace;
        if (workspace) {
            await loadProjects(workspace);
        } else {
            projectSelect.innerHTML = '<option value="">Select Project</option>';
            projectSelect.disabled = true;
            repoSelect.innerHTML = '<option value="">Select Repository</option>';
            repoSelect.disabled = true;
            addPipelineBtn.disabled = true;
        }
    });

    // Project selection - loads repositories
    projectSelect.addEventListener('change', async () => {
        const projectKey = projectSelect.value;
        if (projectKey && currentWorkspace) {
            await loadRepositoriesByProject(currentWorkspace, projectKey);
        } else {
            repoSelect.innerHTML = '<option value="">Select Repository</option>';
            repoSelect.disabled = true;
            addPipelineBtn.disabled = true;
        }
    });

    // Repository selection
    repoSelect.addEventListener('change', () => {
        addPipelineBtn.disabled = !repoSelect.value;
    });

    // Add pipeline button
    addPipelineBtn.addEventListener('click', addMonitoredPipeline);

    // Save settings
    document.getElementById('save-settings-btn').addEventListener('click', saveSettings);

    // Refresh button
    document.getElementById('refresh-btn').addEventListener('click', async () => {
        statusDetails.innerHTML = '<p class="loading">Refreshing...</p>';
        await invoke('trigger_refresh');
    });

    // Create API token link
    createPasswordLink.addEventListener('click', async (e) => {
        e.preventDefault();
        await open('https://id.atlassian.com/manage-profile/security/api-tokens');
    });
}

async function saveCredentials() {
    const username = usernameInput.value.trim();
    const appPassword = appPasswordInput.value.trim();

    if (!username || !appPassword) {
        showAuthStatus('Please enter both username and app password', 'error');
        return;
    }

    // Show loading state
    saveAuthBtn.querySelector('.btn-text').style.display = 'none';
    saveAuthBtn.querySelector('.btn-loading').style.display = 'inline';
    saveAuthBtn.disabled = true;

    try {
        await invoke('save_credentials', { username, appPassword });

        currentUsername = username;
        currentAppPassword = appPassword;
        appPasswordInput.value = '';
        appPasswordInput.placeholder = '••••••••••••••••';

        showAuthStatus('Credentials saved successfully!', 'success');
        await loadWorkspaces();
    } catch (e) {
        showAuthStatus(`Error: ${e}`, 'error');
    } finally {
        saveAuthBtn.querySelector('.btn-text').style.display = 'inline';
        saveAuthBtn.querySelector('.btn-loading').style.display = 'none';
        saveAuthBtn.disabled = false;
    }
}

async function loadWorkspaces() {
    if (!currentUsername || !currentAppPassword) return;

    try {
        workspaces = await invoke('get_workspaces', {
            username: currentUsername,
            appPassword: currentAppPassword
        });
        populateWorkspaceSelect();
    } catch (e) {
        console.error('Failed to load workspaces:', e);
        showAuthStatus(`Failed to load workspaces: ${e}`, 'error');
    }
}

async function loadProjects(workspace) {
    if (!currentUsername || !currentAppPassword) return;

    projectSelect.innerHTML = '<option value="">Loading...</option>';
    projectSelect.disabled = true;
    repoSelect.innerHTML = '<option value="">Select Repository</option>';
    repoSelect.disabled = true;

    try {
        projects = await invoke('get_projects', {
            username: currentUsername,
            appPassword: currentAppPassword,
            workspace
        });
        populateProjectSelect();
    } catch (e) {
        console.error('Failed to load projects:', e);
        projectSelect.innerHTML = '<option value="">Error loading projects</option>';
    }
}

async function loadRepositoriesByProject(workspace, projectKey) {
    if (!currentUsername || !currentAppPassword) return;

    repoSelect.innerHTML = '<option value="">Loading...</option>';
    repoSelect.disabled = true;

    try {
        repositories = await invoke('get_repositories_by_project', {
            username: currentUsername,
            appPassword: currentAppPassword,
            workspace,
            projectKey
        });
        populateRepoSelect();
    } catch (e) {
        console.error('Failed to load repositories:', e);
        repoSelect.innerHTML = '<option value="">Error loading repos</option>';
    }
}

async function loadRepositories(workspace) {
    if (!currentUsername || !currentAppPassword) return;

    repoSelect.innerHTML = '<option value="">Loading...</option>';
    repoSelect.disabled = true;

    try {
        repositories = await invoke('get_repositories', {
            username: currentUsername,
            appPassword: currentAppPassword,
            workspace
        });
        populateRepoSelect();
    } catch (e) {
        console.error('Failed to load repositories:', e);
        repoSelect.innerHTML = '<option value="">Error loading repos</option>';
    }
}

function populateWorkspaceSelect() {
    workspaceSelect.innerHTML = '<option value="">Select Workspace</option>';
    workspaces.forEach(ws => {
        const option = document.createElement('option');
        option.value = ws.slug;
        option.textContent = ws.name || ws.slug;
        workspaceSelect.appendChild(option);
    });
    workspaceSelect.disabled = false;
}

function populateProjectSelect() {
    projectSelect.innerHTML = '<option value="">Select Project</option>';
    projects.forEach(proj => {
        const option = document.createElement('option');
        option.value = proj.key;
        option.textContent = proj.name || proj.key;
        projectSelect.appendChild(option);
    });
    projectSelect.disabled = false;
}

function populateRepoSelect() {
    repoSelect.innerHTML = '<option value="">Select Repository</option>';
    repositories.forEach(repo => {
        const option = document.createElement('option');
        option.value = repo.slug;
        option.textContent = repo.name || repo.slug;
        repoSelect.appendChild(option);
    });
    repoSelect.disabled = false;
}

async function addMonitoredPipeline() {
    const workspace = workspaceSelect.value;
    const projectKey = projectSelect.value;
    const projectName = projectSelect.options[projectSelect.selectedIndex]?.text || null;
    const repoSlug = repoSelect.value;
    const repoName = repoSelect.options[repoSelect.selectedIndex].text;

    if (!workspace || !projectKey || !repoSlug) {
        showNotification('Please select workspace, project, and repository', 'error');
        return;
    }

    // Check for duplicates
    const exists = monitoredPipelines.some(
        p => p.workspace === workspace && p.repo_slug === repoSlug
    );

    if (exists) {
        showNotification('This pipeline is already being monitored', 'error');
        return;
    }

    monitoredPipelines.push({
        workspace,
        project_key: projectKey,
        project_name: projectName,
        repo_slug: repoSlug,
        repo_name: repoName,
        branch: null
    });

    try {
        await invoke('save_monitored_pipelines', { pipelines: monitoredPipelines });
        renderPipelineList();
        showNotification('Pipeline added!', 'success');

        // Reset selects
        repoSelect.value = '';
        addPipelineBtn.disabled = true;
    } catch (e) {
        showNotification(`Failed to save: ${e}`, 'error');
        monitoredPipelines.pop();
    }
}

function renderPipelineList() {
    pipelineList.innerHTML = '';

    if (monitoredPipelines.length === 0) {
        pipelineList.innerHTML = '<li class="empty">No pipelines monitored</li>';
        return;
    }

    // Group pipelines by project (fall back to workspace if no project)
    const grouped = {};
    monitoredPipelines.forEach((pipeline, index) => {
        const groupKey = pipeline.project_name || pipeline.workspace;
        if (!grouped[groupKey]) {
            grouped[groupKey] = [];
        }
        grouped[groupKey].push({ pipeline, index });
    });

    // Render grouped pipelines
    Object.keys(grouped).forEach(projectName => {
        // Project header
        const header = document.createElement('li');
        header.className = 'workspace-header';
        header.textContent = projectName;
        pipelineList.appendChild(header);

        // Pipelines in this project
        grouped[projectName].forEach(({ pipeline, index }) => {
            const li = document.createElement('li');
            li.className = 'pipeline-item';
            li.innerHTML = `
                <span class="pipeline-name">${pipeline.repo_name || pipeline.repo_slug}</span>
                <button type="button" class="remove-btn" data-index="${index}">Remove</button>
            `;
            li.querySelector('.remove-btn').addEventListener('click', () => {
                removePipeline(index);
            });
            pipelineList.appendChild(li);
        });
    });
}

async function removePipeline(index) {
    monitoredPipelines.splice(index, 1);
    try {
        await invoke('save_monitored_pipelines', { pipelines: monitoredPipelines });
        renderPipelineList();
        showNotification('Pipeline removed', 'success');
    } catch (e) {
        showNotification(`Failed to remove: ${e}`, 'error');
    }
}

async function saveSettings() {
    const interval = parseInt(pollingIntervalInput.value, 10);

    if (interval < 30) {
        showNotification('Interval must be at least 30 seconds', 'error');
        return;
    }

    try {
        await invoke('set_polling_interval', { seconds: interval });
        showNotification('Settings saved!', 'success');
    } catch (e) {
        showNotification(`Failed to save settings: ${e}`, 'error');
    }
}

function listenForStatusUpdates() {
    listen('status-updated', (event) => {
        const status = event.payload;
        updateStatusDisplay(status);
    });
}

function updateStatusDisplay(status) {
    if (!status) {
        statusDetails.innerHTML = '<p>No status available</p>';
        statusIndicator.className = 'status-gray';
        return;
    }

    statusIndicator.className = status.is_healthy ? 'status-green' : 'status-red';

    if (status.is_healthy) {
        let html = `<p class="healthy">All ${status.total_monitored} pipeline(s) healthy</p>`;
        if (status.in_progress_count > 0) {
            html += `<p class="in-progress">${status.in_progress_count} in progress</p>`;
        }
        html += `<p class="last-checked">Last checked: ${status.last_checked}</p>`;
        statusDetails.innerHTML = html;
    } else {
        const failedList = status.failed_pipelines
            .map(p => `<li>${p.repo_name || p.repo_slug} - ${p.failure_reason}</li>`)
            .join('');
        statusDetails.innerHTML = `
            <p class="failed">${status.failed_pipelines.length} pipeline(s) failed</p>
            <ul class="failed-list">${failedList}</ul>
            <p class="last-checked">Last checked: ${status.last_checked}</p>
        `;
    }
}

function showAuthStatus(message, type) {
    authStatus.textContent = message;
    authStatus.className = `status-message ${type}`;
    authStatus.style.display = 'block';
}

function showNotification(message, type) {
    const existing = document.querySelector('.notification');
    if (existing) existing.remove();

    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.textContent = message;
    document.body.appendChild(notification);

    setTimeout(() => notification.remove(), 3000);
}
