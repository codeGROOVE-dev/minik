// Wait for Tauri to be available
const { invoke } = window.__TAURI__.core;

// Check if shell plugin is available
let open;
if (window.__TAURI__.shell) {
    open = window.__TAURI__.shell.open;
} else {
    console.warn('Shell plugin not available, links will not open');
    open = () => console.log('Shell plugin not available');
}

let currentProjectData = null;
let isExpanded = false;
let refreshInterval = null;
let draggedItem = null;
let isDragging = false;
let dragElement = null;
let dragOffset = { x: 0, y: 0 };
let originalParent = null;

const COLUMN_COLORS = ['yellow', 'blue', 'green', 'pink', 'orange', 'purple'];

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
    console.log('DOM Content Loaded, starting initialization...');

    try {
        updateStatus('Checking GitHub authentication...');
        await checkAuth();

        updateStatus('Loading saved project...');
        await loadSavedProject();

        updateStatus('Setting up interface...');
        setupEventListeners();
        startAutoRefresh();

        // If no project is loaded, try to load the first available project
        if (!currentProjectData) {
            updateStatus('No saved project found, searching for projects...');
            await loadFirstAvailableProject();
        }
    } catch (error) {
        console.error('Initialization error:', error);
        updateStatus(`Error: ${error.message || error}`);
        showError(`Initialization failed: ${error}`);
    }
});

async function checkAuth() {
    try {
        await invoke('github_token');
        updateStatus('GitHub authenticated successfully');
    } catch (error) {
        updateStatus('GitHub authentication failed!');
        showError('GitHub authentication required. Please run "gh auth login" first.');
    }
}

async function loadSavedProject() {
    try {
        const projectId = await invoke('current_project');
        if (projectId) {
            updateStatus('Loading saved project data...');
            await loadProjectData(projectId);
        } else {
            updateStatus('No saved project found');
        }
    } catch (error) {
        console.error('Failed to load saved project:', error);
        updateStatus('Failed to load saved project');
    }
}

async function loadFirstAvailableProject() {
    try {
        console.log('No saved project found, loading first available project...');
        updateStatus('Fetching your GitHub organizations...');

        // Get list of organizations
        const orgs = await invoke('list_organizations');
        if (!orgs || orgs.length === 0) {
            console.log('No organizations found');
            updateStatus('No organizations found');
            showError('No GitHub organizations found. Please ensure you have access to at least one organization with projects.');
            return;
        }

        updateStatus(`Found ${orgs.length} organization(s), searching for projects...`);
        console.log(`Organizations found: ${orgs.map(o => o.login).join(', ')}`);

        // Try to find a project in all organizations
        for (let i = 0; i < orgs.length; i++) {
            const org = orgs[i];
            console.log(`Checking organization ${i + 1}/${orgs.length}: ${org.login}`);
            updateStatus(`Checking org ${i + 1}/${orgs.length}: ${org.login}...`);
            try {
                const projects = await invoke('list_org_projects', { org: org.login });
                if (projects && projects.length > 0) {
                    console.log(`Found ${projects.length} projects in ${org.login}`);
                    updateStatus(`Found ${projects.length} project(s) in ${org.login}`);
                    // Load the first project
                    const firstProject = projects[0];
                    updateStatus(`Loading project: ${firstProject.title}...`);
                    await invoke('select_project', { projectId: firstProject.id });
                    await loadProjectData(firstProject.id);
                    console.log(`Loaded project: ${firstProject.title}`);
                    return;
                }
            } catch (error) {
                console.error(`Failed to list projects for ${org.login}:`, error);
                // Check if it's a permissions error
                if (error.toString().includes('INSUFFICIENT_SCOPES') || error.toString().includes('read:project')) {
                    updateStatus(`Need 'project' scope for ${org.login}, trying next...`);
                } else {
                    updateStatus(`Error checking ${org.login}, trying next...`);
                }
            }
        }

        console.log('No projects found in any organization');
        updateStatus('No projects found in any organization');
        showError('No GitHub projects found. Please create a project in one of your organizations first.');
    } catch (error) {
        console.error('Failed to load first available project:', error);
        showError(`Failed to load projects: ${error}`);
    }
}

async function loadProjectData(projectId) {
    try {
        updateStatus('Fetching project data from GitHub...');
        const projectData = await invoke('project_data', { projectId });
        currentProjectData = projectData;
        updateStatus('Rendering project...');
        renderProject();
    } catch (error) {
        updateStatus('Failed to load project data');
        showError(`Failed to load project: ${error}`);
    }
}

function renderProject() {
    if (!currentProjectData) return;

    renderMinimizedView();
    renderExpandedView();
}

function renderMinimizedView() {
    const summary = document.getElementById('columns-summary');
    const hiddenColumns = currentProjectData.hiddenColumns || [];

    const visibleColumns = currentProjectData.columns.filter(
        col => !hiddenColumns.includes(col.id)
    );

    const columnsHtml = visibleColumns.map((column, index) => {
        const colorClass = `column-${COLUMN_COLORS[index % COLUMN_COLORS.length]}`;
        return `
            <span class="column-badge ${colorClass}">
                ${column.name}: <span class="column-count">${column.items_count}</span>
            </span>
        `;
    }).join('');

    summary.innerHTML = columnsHtml || '<span class="loading">No project selected</span>';
}

function renderExpandedView() {
    const board = document.getElementById('kanban-board');
    const hiddenColumns = currentProjectData.hiddenColumns || [];

    const visibleColumns = currentProjectData.columns.filter(
        col => !hiddenColumns.includes(col.id)
    );

    const columnsHtml = visibleColumns.map((column, index) => {
        const colorClass = `column-${COLUMN_COLORS[index % COLUMN_COLORS.length]}`;
        const items = currentProjectData.items.filter(item => item.column_id === column.id);

        const cardsHtml = items.map(item => {
            const hasMetadata = item.assignees.length > 0 || item.labels.length > 0;
            return `
                <div class="kanban-card"
                     draggable="true"
                     data-item-id="${item.id}"
                     data-column-id="${column.id}"
                     data-url="${item.url || '#'}">
                    <div class="card-title">${escapeHtml(item.title)}</div>
                    ${hasMetadata ? `
                        <div class="card-meta">
                            ${item.assignees.length > 0 ?
                                item.assignees.slice(0, 2).map(a => `<span class="assignee">@${escapeHtml(a)}</span>`).join('') +
                                (item.assignees.length > 2 ? `<span class="assignee">+${item.assignees.length - 2}</span>` : '')
                            : ''}
                            ${item.labels.length > 0 ?
                                item.labels.slice(0, 3).map(l => `<span class="label">${escapeHtml(l)}</span>`).join('')
                            : ''}
                        </div>
                    ` : ''}
                </div>
            `;
        }).join('');

        return `
            <div class="kanban-column ${colorClass}" data-column-id="${column.id}">
                <div class="column-header">
                    <span>${escapeHtml(column.name)}</span>
                    <span class="column-count-badge">${items.length}</span>
                </div>
                <div class="column-cards">
                    ${cardsHtml || '<div class="column-empty">No items</div>'}
                </div>
            </div>
        `;
    }).join('');

    board.innerHTML = columnsHtml || '<div style="padding: 20px; color: #999;">No columns to display</div>';

    // Add click and custom drag handlers for cards
    const cards = board.querySelectorAll('.kanban-card');
    console.log(`Setting up drag handlers for ${cards.length} cards`);

    cards.forEach((card, index) => {
        // Remove native draggable attribute to avoid conflicts
        card.draggable = false;
        card.style.cursor = 'grab';

        // Click handler for opening URLs
        card.addEventListener('click', (e) => {
            // Don't open URL if we just finished dragging
            if (!isDragging) {
                const url = card.dataset.url;
                if (url && url !== '#') {
                    open(url);
                }
            }
        });

        // Custom drag implementation using mouse events
        card.addEventListener('mousedown', (e) => {
            e.preventDefault(); // Prevent text selection
            e.stopPropagation();

            console.log('🖱️ Mouse down on card:', card.dataset.itemId);

            // Store the original position
            const rect = card.getBoundingClientRect();
            dragOffset.x = e.clientX - rect.left;
            dragOffset.y = e.clientY - rect.top;

            // Create a drag clone
            dragElement = card.cloneNode(true);
            dragElement.style.position = 'fixed';
            dragElement.style.zIndex = '10000';
            dragElement.style.opacity = '0.8';
            dragElement.style.cursor = 'grabbing';
            dragElement.style.pointerEvents = 'none';
            dragElement.style.width = rect.width + 'px';
            dragElement.style.left = (e.clientX - dragOffset.x) + 'px';
            dragElement.style.top = (e.clientY - dragOffset.y) + 'px';
            dragElement.classList.add('dragging');
            document.body.appendChild(dragElement);

            // Store drag info
            draggedItem = {
                itemId: card.dataset.itemId,
                fromColumnId: card.dataset.columnId,
                element: card
            };

            // Mark original card as being dragged
            card.style.opacity = '0.3';
            originalParent = card.parentElement;

            isDragging = true;

            console.log('🎯 Custom drag started:', {
                itemId: draggedItem.itemId,
                fromColumnId: draggedItem.fromColumnId,
                cardTitle: card.querySelector('.card-title')?.textContent
            });
        });
    });

    // Global mouse event handlers for custom drag
    document.addEventListener('mousemove', (e) => {
        if (isDragging && dragElement) {
            e.preventDefault();
            dragElement.style.left = (e.clientX - dragOffset.x) + 'px';
            dragElement.style.top = (e.clientY - dragOffset.y) + 'px';

            // Check which column we're over
            const elementBelow = document.elementFromPoint(e.clientX, e.clientY);
            if (elementBelow) {
                const column = elementBelow.closest('.kanban-column');

                // Remove drag-over from all columns
                board.querySelectorAll('.kanban-column').forEach(col => {
                    col.classList.remove('drag-over');
                });

                // Add drag-over to current column
                if (column) {
                    column.classList.add('drag-over');
                }
            }
        }
    });

    document.addEventListener('mouseup', async (e) => {
        if (isDragging) {
            console.log('🏁 Mouse up - ending drag');

            // Find which column we're dropping on
            const elementBelow = document.elementFromPoint(e.clientX, e.clientY);
            let targetColumn = null;

            if (elementBelow) {
                targetColumn = elementBelow.closest('.kanban-column');
            }

            // Clean up drag visuals
            if (dragElement) {
                document.body.removeChild(dragElement);
                dragElement = null;
            }

            // Remove drag-over styling
            board.querySelectorAll('.kanban-column').forEach(col => {
                col.classList.remove('drag-over');
            });

            // Restore original card opacity
            if (draggedItem && draggedItem.element) {
                draggedItem.element.style.opacity = '1';
            }

            // Handle the drop
            if (targetColumn && draggedItem) {
                const toColumnId = targetColumn.dataset.columnId;
                const toColumnName = targetColumn.querySelector('.column-header span')?.textContent;

                // Store draggedItem data before async operation
                const itemToMove = {
                    itemId: draggedItem.itemId,
                    fromColumnId: draggedItem.fromColumnId
                };

                console.log('📦 Drop event:', {
                    fromColumnId: itemToMove.fromColumnId,
                    toColumnId: toColumnId,
                    toColumnName: toColumnName,
                    itemId: itemToMove.itemId
                });

                // Only update if moved to a different column
                if (itemToMove.fromColumnId !== toColumnId) {
                    console.log(`🚀 Moving item ${itemToMove.itemId} to ${toColumnName}`);

                    try {
                        console.log('📡 Calling update_item_column with:', {
                            projectId: currentProjectData.project.id,
                            itemId: itemToMove.itemId,
                            columnId: toColumnId
                        });

                        await invoke('update_item_column', {
                            projectId: currentProjectData.project.id,
                            itemId: itemToMove.itemId,
                            columnId: toColumnId
                        });

                        console.log('✅ GitHub update successful');

                        // Update local data
                        const item = currentProjectData.items.find(i => i.id === itemToMove.itemId);
                        if (item) {
                            console.log('📝 Updating local item data');
                            const oldColumnId = item.column_id;
                            item.column_id = toColumnId;
                            console.log(`Local update: ${oldColumnId} -> ${toColumnId}`);
                            renderExpandedView();
                            renderMinimizedView();
                        } else {
                            console.error('❌ Item not found in local data:', itemToMove.itemId);
                        }
                    } catch (error) {
                        console.error('❌ Failed to update item:', error);
                        showError(`Failed to move item: ${error}`);
                        // Refresh to restore correct state
                        await loadProjectData(currentProjectData.project.id);
                    }
                } else {
                    console.log('ℹ️ Item dropped in same column, no update needed');
                }
            }

            // Reset drag state
            isDragging = false;
            draggedItem = null;

            // Small delay to prevent click event from firing
            setTimeout(() => {
                isDragging = false;
            }, 100);
        }
    });
}

function setupEventListeners() {
    // Double-click to toggle view on both minimized and expanded views
    document.getElementById('minimized-view').addEventListener('dblclick', toggleView);
    document.getElementById('expanded-view').addEventListener('dblclick', (e) => {
        // Only toggle if not clicking on interactive elements
        if (!e.target.closest('.kanban-card') &&
            !e.target.closest('.control-btn') &&
            !e.target.closest('button')) {
            toggleView();
        }
    });

    // Note: Minimize and refresh buttons removed from UI

    // Error close button
    document.getElementById('error-close').addEventListener('click', () => {
        document.getElementById('error-message').classList.add('hidden');
    });

    // Keyboard shortcuts
    document.addEventListener('keydown', (e) => {
        // ESC key to minimize
        if (e.key === 'Escape' && isExpanded) {
            toggleView();
        }

        // Cmd+R to refresh
        if ((e.metaKey || e.ctrlKey) && e.key === 'r') {
            e.preventDefault();
            if (currentProjectData) {
                loadProjectData(currentProjectData.project.id);
            } else {
                loadFirstAvailableProject();
            }
        }

        // Cmd+P to load first project (for testing)
        if ((e.metaKey || e.ctrlKey) && e.key === 'p') {
            e.preventDefault();
            loadFirstAvailableProject();
        }
    });

    // Click outside to minimize (when expanded)
    document.addEventListener('click', (e) => {
        const expandedView = document.getElementById('expanded-view');
        if (isExpanded && !expandedView.contains(e.target)) {
            toggleView();
        }
    });
}

async function toggleView() {
    isExpanded = await invoke('toggle_expanded');

    if (isExpanded) {
        document.getElementById('minimized-view').classList.add('hidden');
        document.getElementById('expanded-view').classList.remove('hidden');

        // Resize window based on column count
        if (currentProjectData && currentProjectData.columns) {
            const hiddenColumns = currentProjectData.hiddenColumns || [];
            const visibleColumns = currentProjectData.columns.filter(
                col => !hiddenColumns.includes(col.id)
            );
            console.log(`Resizing window for ${visibleColumns.length} columns`);
            try {
                await invoke('resize_window_for_columns', { columnCount: visibleColumns.length });
            } catch (error) {
                console.error('Failed to resize window:', error);
            }
        }
    } else {
        document.getElementById('minimized-view').classList.remove('hidden');
        document.getElementById('expanded-view').classList.add('hidden');
    }
}

function startAutoRefresh() {
    // Refresh every 5 minutes
    refreshInterval = setInterval(async () => {
        if (currentProjectData) {
            await loadProjectData(currentProjectData.project.id);
        }
    }, 5 * 60 * 1000);
}

function showError(message) {
    const errorEl = document.getElementById('error-message');
    const errorText = document.getElementById('error-text');

    errorText.textContent = message;
    errorEl.classList.remove('hidden');

    // Auto-hide after 10 seconds
    setTimeout(() => {
        errorEl.classList.add('hidden');
    }, 10000);
}

function escapeHtml(text) {
    const map = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#39;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
}

function updateStatus(message) {
    const statusElement = document.getElementById('status-message');
    if (statusElement) {
        statusElement.textContent = message;
        console.log(`Status: ${message}`);
    }
}

// Listen for project selection from Rust backend
window.__TAURI__.event.listen('project-selected', async (event) => {
    await loadProjectData(event.payload.projectId);
});

// Listen for refresh event from tray menu
window.__TAURI__.event.listen('refresh-project', async () => {
    if (currentProjectData) {
        await loadProjectData(currentProjectData.project.id);
    } else {
        await loadFirstAvailableProject();
    }
});