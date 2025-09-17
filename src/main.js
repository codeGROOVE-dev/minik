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
let currentUsername = null;
let showOnlyMyItems = false;
let currentColumns = [];
let availableProjects = {}; // org -> projects map

const COLUMN_COLORS = ['yellow', 'blue', 'green', 'pink', 'orange', 'purple'];

// Initialize app
document.addEventListener('DOMContentLoaded', async () => {
    console.log('DOM Content Loaded, starting initialization...');

    // Setup window dragging for frameless window
    setupWindowDragging();

    try {
        updateStatus('Checking GitHub authentication...');
        await checkAuth();

        updateStatus('Loading saved project...');
        // Restore expanded state from backend BEFORE loading project
        try {
            const savedState = await invoke('is_expanded');
            if (savedState) {
                isExpanded = true;
                document.getElementById('minimized-view').classList.add('hidden');
                document.getElementById('expanded-view').classList.remove('hidden');
                document.getElementById('window-drag-handle').classList.remove('hidden');
                console.log('Restored expanded state from saved settings');
            }
        } catch (error) {
            console.warn('Failed to load expanded state:', error);
        }

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

        // Get current user
        try {
            currentUsername = await invoke('current_user');
            console.log('Current GitHub user:', currentUsername);
        } catch (error) {
            console.warn('Failed to get current user:', error);
        }

        // Get filter state
        try {
            showOnlyMyItems = await invoke('show_only_my_items');
            console.log('Show only my items:', showOnlyMyItems);
        } catch (error) {
            console.warn('Failed to get filter state:', error);
        }
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

        // Load saved column visibility settings for this project
        try {
            const hiddenColumns = await invoke('hidden_columns');
            currentProjectData.hiddenColumns = hiddenColumns;
            console.log('Loaded hidden columns for project:', hiddenColumns);
        } catch (error) {
            console.warn('Failed to load hidden columns:', error);
            currentProjectData.hiddenColumns = [];
        }

        // Ensure the filter state is current
        try {
            showOnlyMyItems = await invoke('show_only_my_items');
            console.log('Filter state:', showOnlyMyItems);
        } catch (error) {
            console.warn('Failed to load filter state:', error);
        }

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

    // Set proper window size based on current view
    if (isExpanded) {
        // Need to wait for DOM to update before calculating heights
        console.log('Project rendered in expanded view, resizing window...');
        setTimeout(() => {
            console.log('Calling calculateAndSetWindowHeight...');
            calculateAndSetWindowHeight();
        }, 200); // Give more time for DOM to render
    } else {
        // Set minimized view size
        console.log('Project rendered in minimized view, resizing window...');
        setTimeout(() => {
            console.log('Calling setMinimizedViewSize...');
            setMinimizedViewSize();
        }, 200);
    }
}

function renderMinimizedView() {
    const summary = document.getElementById('columns-summary');
    const hiddenColumns = currentProjectData.hiddenColumns || [];

    const visibleColumns = currentProjectData.columns.filter(
        col => !hiddenColumns.includes(col.id)
    );

    const columnsHtml = visibleColumns.map((column, index) => {
        const colorClass = `column-${COLUMN_COLORS[index % COLUMN_COLORS.length]}`;

        // Calculate count based on filter
        let itemCount = column.items_count;
        if (showOnlyMyItems && currentUsername) {
            const items = currentProjectData.items.filter(item =>
                item.column_id === column.id &&
                item.assignees && item.assignees.includes(currentUsername)
            );
            itemCount = items.length;
        }

        return `
            <span class="column-badge ${colorClass}">
                ${column.name}: <span class="column-count">${itemCount}</span>
            </span>
        `;
    }).join('');

    summary.innerHTML = columnsHtml || '<span class="loading">No project selected</span>';
}

async function calculateAndSetWindowHeight() {
    console.log('calculateAndSetWindowHeight called');
    // Wait for render to complete
    await new Promise(resolve => setTimeout(resolve, 50));

    const board = document.getElementById('kanban-board');
    const expandedView = document.getElementById('expanded-view');

    console.log('Expanded view visible:', expandedView && !expandedView.classList.contains('hidden'));
    if (expandedView && !expandedView.classList.contains('hidden')) {
        // Calculate actual content height and width
        const rect = board.getBoundingClientRect();
        const columns = board.querySelectorAll('.kanban-column');

        let maxHeight = 0;
        let totalWidth = 0;
        columns.forEach((col, index) => {
            const colRect = col.getBoundingClientRect();
            const height = colRect.height;
            const width = colRect.width;
            console.log(`Column ${index}: ${col.querySelector('.column-header')?.textContent} height = ${height}px, width = ${width}px`);
            if (height > maxHeight) {
                maxHeight = height;
            }
            totalWidth += width;
        });

        // Add padding from the board (6px left + 6px right = 12px) and gaps between columns
        const gaps = Math.max(0, columns.length - 1) * 4; // 4px gap between columns
        const totalWidthWithPadding = Math.ceil(totalWidth + 12 + gaps);

        // Add padding from the board (6px top + 6px bottom = 12px)
        const totalHeight = Math.ceil(maxHeight + 12);
        console.log(`Max column height: ${maxHeight}px, Total width: ${totalWidthWithPadding}px, Total height: ${totalHeight}px`);

        // Resize window to match actual content
        try {
            console.log(`Resizing window to ${totalWidthWithPadding}x${totalHeight} for ${columns.length} visible columns`);
            await invoke('resize_window_to_dimensions', {
                width: totalWidthWithPadding,
                height: totalHeight
            });
            console.log('Window resize completed');
        } catch (error) {
            console.error('Failed to resize window:', error);
        }
    }
}

function renderExpandedView() {
    const board = document.getElementById('kanban-board');
    const hiddenColumns = currentProjectData.hiddenColumns || [];

    const visibleColumns = currentProjectData.columns.filter(
        col => !hiddenColumns.includes(col.id)
    );

    // If no columns are visible, show a dummy column
    if (visibleColumns.length === 0) {
        board.innerHTML = `
            <div class="kanban-column column-yellow dummy-column">
                <div class="column-header">
                    <span class="column-title">All columns are hidden</span>
                    <span class="column-count-badge">!</span>
                </div>
                <div class="column-cards">
                    <div class="dummy-message">Right-click to show columns</div>
                </div>
            </div>
        `;
        return;
    }

    const columnsHtml = visibleColumns.map((column, index) => {
        const colorClass = `column-${COLUMN_COLORS[index % COLUMN_COLORS.length]}`;
        let items = currentProjectData.items.filter(item => item.column_id === column.id);

        // Apply "Show only my items" filter
        if (showOnlyMyItems && currentUsername) {
            items = items.filter(item =>
                item.assignees && item.assignees.includes(currentUsername)
            );
        }

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

    // Setup menu event listeners
    setupMenuListeners();

    // Setup context menu event listeners
    setupContextMenuListeners();

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

    // Removed click-outside handler to prevent accidental minimizing
    // Users can double-click or press ESC to minimize instead
}

async function toggleView() {
    isExpanded = await invoke('toggle_expanded');
    const dragHandle = document.getElementById('window-drag-handle');

    if (isExpanded) {
        document.getElementById('minimized-view').classList.add('hidden');
        document.getElementById('expanded-view').classList.remove('hidden');
        dragHandle.classList.remove('hidden');

        // Resize window based on column count and calculate proper height
        if (currentProjectData && currentProjectData.columns) {
            await calculateAndSetWindowHeight();
        }
    } else {
        document.getElementById('minimized-view').classList.remove('hidden');
        document.getElementById('expanded-view').classList.add('hidden');
        dragHandle.classList.add('hidden');

        // Set minimized view size dynamically
        await setMinimizedViewSize();
    }
}

async function setMinimizedViewSize() {
    console.log('Setting minimized view size dynamically');

    // Wait longer for DOM to fully render
    await new Promise(resolve => setTimeout(resolve, 200));

    const minimizedView = document.getElementById('minimized-view');
    const summary = document.getElementById('columns-summary');

    if (minimizedView && !minimizedView.classList.contains('hidden') && summary) {
        // Get the natural content width by temporarily removing width constraints
        const originalStyle = summary.style.cssText;
        summary.style.width = 'auto';
        summary.style.maxWidth = 'none';

        // Force layout recalculation
        summary.offsetWidth;

        // Measure the actual content width
        const rect = summary.getBoundingClientRect();
        const contentWidth = rect.width;

        // Restore original styling
        summary.style.cssText = originalStyle;

        // Add padding (10px left + 10px right) plus extra buffer to prevent clipping
        const paddedWidth = Math.ceil(contentWidth + 40); // Increased from 20 to 40 to prevent clipping

        // Calculate actual height needed - the minimized view is 40px height + 8px top/bottom padding = 56px
        // But let's measure it to be sure
        const minimizedRect = minimizedView.getBoundingClientRect();
        const actualHeight = Math.ceil(minimizedRect.height);

        const badges = summary.querySelectorAll('.column-badge');
        console.log(`Minimized view: content width = ${contentWidth}px, final width = ${paddedWidth}px, actual height = ${actualHeight}px, badges = ${badges.length}`);

        try {
            await invoke('resize_window_to_dimensions', {
                width: paddedWidth,
                height: actualHeight
            });
        } catch (error) {
            console.error('Failed to resize minimized window:', error);
        }
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

function setupMenuListeners() {
    // Check if Tauri event API is available
    if (!window.__TAURI__ || !window.__TAURI__.event) {
        console.warn('Tauri event API not available, skipping menu listeners');
        return;
    }

    const { listen } = window.__TAURI__.event;

    // Listen for menu refresh event
    listen('menu-refresh', async () => {
        console.log('Menu refresh triggered');
        if (currentProjectData) {
            await refreshProject();
        }
    });

    // Listen for menu toggle my items event
    listen('menu-toggle-my-items', async () => {
        console.log('Menu toggle my items triggered');
        await toggleMyItems();
    });

    // Listen for menu toggle expanded view event
    listen('menu-toggle-expanded', () => {
        console.log('Menu toggle expanded triggered');
        toggleView();
    });

    // Listen for menu select project event
    listen('menu-select-project', async () => {
        console.log('Menu select project triggered');
        await showProjectSelector();
    });

    // Listen for menu toggle column visibility event
    listen('menu-toggle-column', async (event) => {
        console.log('Menu toggle column triggered', event.payload);
        const columnId = event.payload;
        await invoke('toggle_column_visibility', { columnId });
        if (currentProjectData) {
            renderProject();
        }
    });
}

function setupContextMenuListeners() {
    // Check if Tauri event API is available
    if (!window.__TAURI__ || !window.__TAURI__.event) {
        console.warn('Tauri event API not available, skipping context menu listeners');
        return;
    }

    const { listen } = window.__TAURI__.event;

    // Listen for project context menu data
    listen('show-project-context-menu', (event) => {
        console.log('Project context menu data received', event.payload);
        const organizations = event.payload;
        showProjectContextMenu(organizations);
    });

    // Listen for column context menu data
    listen('show-column-context-menu', (event) => {
        console.log('Column context menu data received', event.payload);
        const [projectId, columns] = event.payload;
        showColumnContextMenu(projectId, columns);
    });

    // Listen for project selector event
    listen('show-project-selector', async (event) => {
        console.log('Show project selector event received');
        console.log('Event payload:', event.payload);
        await showProjectSelector();
    });

    // Listen for optimized project selector with pre-fetched data
    listen('show-project-context-menu-with-projects', async (event) => {
        console.log('Show project selector with pre-fetched data received');
        console.log('Event payload:', event.payload);
        console.log('Event payload type:', typeof event.payload);
        console.log('Event payload keys:', event.payload ? Object.keys(event.payload) : 'null');
        await showProjectMenuWithData(event.payload);
    });

    // Add right-click handlers to the app - hierarchical context menu from anywhere
    document.addEventListener('contextmenu', async (e) => {
        e.preventDefault(); // Prevent default context menu

        // Show hierarchical context menu at mouse position
        console.log('Right-clicked at', e.clientX, e.clientY, '- showing hierarchical context menu');
        if (window.showHierarchicalContextMenu) {
            await window.showHierarchicalContextMenu(e.clientX, e.clientY);
        } else {
            console.error('Hierarchical context menu not loaded');
            await showUnifiedContextMenu();
        }
    });
}

async function showProjectContextMenu(organizations) {
    // Remove any existing menu first
    removeContextMenu();

    // Create dynamic context menu for projects
    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.innerHTML = '<div class="context-menu-content"></div>';

    const content = menu.querySelector('.context-menu-content');

    // Add title
    const title = document.createElement('div');
    title.className = 'context-menu-title';
    title.textContent = 'Select Project';
    content.appendChild(title);

    // Fetch projects for all organizations first, then filter out orgs without projects
    for (const org of organizations) {
        try {
            const projects = await invoke('list_org_projects', { org: org.login });

            // Only add organization section if it has projects
            if (projects && projects.length > 0) {
                const orgSection = document.createElement('div');
                orgSection.className = 'context-menu-section';

                const orgTitle = document.createElement('div');
                orgTitle.className = 'context-menu-org-title';
                orgTitle.textContent = org.login;
                orgSection.appendChild(orgTitle);

                // Add projects for this organization
                projects.forEach(project => {
                    const projectItem = document.createElement('div');
                    projectItem.className = 'context-menu-item';
                    projectItem.textContent = project.title;
                    projectItem.addEventListener('click', async () => {
                        console.log('Selected project:', project.id);
                        await invoke('select_project', { projectId: project.id });
                        await loadProjectData(project.id);
                        removeContextMenu();
                    });
                    orgSection.appendChild(projectItem);
                });

                content.appendChild(orgSection);
            }
        } catch (error) {
            console.warn(`Failed to fetch projects for org ${org.login}:`, error);
            // Skip organizations where we can't fetch projects
        }
    }

    // Position and show menu
    document.body.appendChild(menu);

    // Remove menu when clicking outside
    setTimeout(() => {
        document.addEventListener('click', removeContextMenu, { once: true });
    }, 100);
}

async function showColumnContextMenu(projectId, columns) {
    // Remove any existing menu first
    removeContextMenu();

    // Create dynamic context menu for column visibility
    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.innerHTML = '<div class="context-menu-content"></div>';

    const content = menu.querySelector('.context-menu-content');

    // Add title
    const title = document.createElement('div');
    title.className = 'context-menu-title';
    title.textContent = 'Show/Hide Columns';
    content.appendChild(title);

    // Get current hidden columns
    try {
        const hiddenColumns = await invoke('hidden_columns');

        columns.forEach(column => {
            const columnItem = document.createElement('div');
            columnItem.className = 'context-menu-item context-menu-checkbox-item';

            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.checked = !hiddenColumns.includes(column.id);
            checkbox.id = `column-${column.id}`;

            const label = document.createElement('label');
            label.htmlFor = `column-${column.id}`;
            label.textContent = column.name;

            columnItem.appendChild(checkbox);
            columnItem.appendChild(label);

            columnItem.addEventListener('click', async (e) => {
                // Prevent event propagation to avoid closing menu
                e.stopPropagation();

                if (e.target.type !== 'checkbox') {
                    checkbox.checked = !checkbox.checked;
                }

                console.log('Toggling column visibility:', column.id, checkbox.checked);

                try {
                    const isVisible = await invoke('toggle_column_visibility', { columnId: column.id });

                    // Update the local currentProjectData with the new hidden columns state
                    if (currentProjectData) {
                        // Get the updated hidden columns list from the backend
                        const updatedHiddenColumns = await invoke('hidden_columns');
                        currentProjectData.hiddenColumns = updatedHiddenColumns;

                        // Re-render the project to reflect changes
                        renderProject();
                    }
                } catch (error) {
                    console.error('Failed to toggle column visibility:', error);
                }
            });

            content.appendChild(columnItem);
        });
    } catch (error) {
        console.error('Failed to get hidden columns:', error);
    }

    // Position and show menu
    document.body.appendChild(menu);

    // Remove menu when clicking outside
    setTimeout(() => {
        document.addEventListener('click', removeContextMenu, { once: true });
    }, 100);
}

function showAppContextMenu() {
    // Create main context menu with both options
    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.innerHTML = '<div class="context-menu-content"></div>';

    const content = menu.querySelector('.context-menu-content');

    // Add title
    const title = document.createElement('div');
    title.className = 'context-menu-title';
    title.textContent = 'Minik';
    content.appendChild(title);

    // Add project selection option
    const projectItem = document.createElement('div');
    projectItem.className = 'context-menu-item';
    projectItem.textContent = 'Select Project...';
    projectItem.addEventListener('click', async () => {
        removeContextMenu();
        await invoke('show_project_context_menu');
    });
    content.appendChild(projectItem);

    // Add column visibility option (only if project is loaded)
    if (currentProjectData) {
        const columnItem = document.createElement('div');
        columnItem.className = 'context-menu-item';
        columnItem.textContent = 'Show/Hide Columns...';
        columnItem.addEventListener('click', async () => {
            removeContextMenu();
            await invoke('show_column_context_menu', { projectId: currentProjectData.project.id });
        });
        content.appendChild(columnItem);
    }

    // Position and show menu
    document.body.appendChild(menu);

    // Remove menu when clicking outside
    setTimeout(() => {
        document.addEventListener('click', removeContextMenu, { once: true });
    }, 100);
}

async function showUnifiedContextMenu() {
    // Create unified context menu with submenus
    console.log('Creating unified context menu');
    console.log('Current project data available:', !!currentProjectData);
    console.log('Is expanded:', isExpanded);
    if (currentProjectData) {
        console.log('Project title:', currentProjectData.project.title);
    }

    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.innerHTML = '<div class="context-menu-content"></div>';

    const content = menu.querySelector('.context-menu-content');

    // Add title - show current project name or "Minik" if no project
    const title = document.createElement('div');
    title.className = 'context-menu-title';

    if (currentProjectData) {
        title.textContent = currentProjectData.project.title;
        title.style.cursor = 'pointer';
        title.addEventListener('click', () => {
            console.log('Project title clicked');
            console.log('Current project data:', currentProjectData);
            console.log('Project URL:', currentProjectData.project.url);
            console.log('Open function available:', typeof open);
            if (currentProjectData.project.url) {
                try {
                    console.log('Attempting to open URL:', currentProjectData.project.url);
                    open(currentProjectData.project.url);
                    console.log('Open function called successfully');
                    removeContextMenu();
                } catch (error) {
                    console.error('Error opening URL:', error);
                }
            } else {
                console.log('No URL available to open');
            }
        });
    } else {
        title.textContent = 'Minik';
    }

    content.appendChild(title);

    // Add Projects submenu with expandable behavior
    const projectsItem = document.createElement('div');
    projectsItem.className = 'context-menu-item context-menu-expandable';
    projectsItem.id = 'projects-menu-item';
    projectsItem.innerHTML = 'Projects <span style="float: right;">›</span>';

    // Create a container for the expanded projects
    const projectsContainer = document.createElement('div');
    projectsContainer.className = 'context-menu-expanded-section';
    projectsContainer.style.display = 'none';

    projectsItem.addEventListener('click', async () => {
        console.log('Projects menu clicked - expanding inline');

        // Toggle expansion
        const isExpanded = projectsContainer.style.display !== 'none';
        if (isExpanded) {
            projectsContainer.style.display = 'none';
            projectsItem.innerHTML = 'Projects <span style="float: right;">›</span>';
        } else {
            projectsItem.innerHTML = 'Projects <span style="float: right;">˅</span>';
            projectsContainer.style.display = 'block';

            // Check if we have pending data
            if (window.pendingProjectData) {
                console.log('Using pending project data');
                await showProjectMenuWithData(window.pendingProjectData);
                window.pendingProjectData = null;
            } else {
                projectsContainer.innerHTML = '<div class="context-menu-loading">Loading...</div>';
                // Fetch projects in the background
                await invoke('show_project_context_menu');
            }
        }
    });

    content.appendChild(projectsItem);
    content.appendChild(projectsContainer);

    // Add Columns submenu (only if project is loaded)
    if (currentProjectData) {
        const columnsItem = document.createElement('div');
        columnsItem.className = 'context-menu-item context-menu-expandable';
        columnsItem.innerHTML = 'Columns <span style="float: right;">›</span>';
        columnsItem.addEventListener('click', async () => {
            console.log('Columns menu clicked - invoking show_column_context_menu');
            // Don't remove the menu immediately, let the backend trigger the new menu
            await invoke('show_column_context_menu', { projectId: currentProjectData.project.id });
        });
        content.appendChild(columnsItem);
    }

    // Position and show menu
    document.body.appendChild(menu);

    // Remove menu when clicking outside
    setTimeout(() => {
        document.addEventListener('click', removeContextMenu, { once: true });
    }, 100);
}

function removeContextMenu() {
    const existing = document.querySelector('.context-menu');
    if (existing) {
        existing.remove();
    }
}

// Make functions and variables globally available for hierarchical-menu.js
window.removeContextMenu = removeContextMenu;
window.getCurrentProjectData = () => currentProjectData;
window.setCurrentProjectData = (data) => { currentProjectData = data; };
window.COLUMN_COLORS = COLUMN_COLORS;
window.loadProjectData = loadProjectData;
window.renderProject = renderProject;
window.showError = showError;

async function showProjectSelector() {
    // Remove any existing menu first
    removeContextMenu();

    try {
        // Create a simple project selector dialog
        const orgs = await invoke('list_organizations');
        if (!orgs || orgs.length === 0) {
            showError('No GitHub organizations found');
            return;
        }

        // Build HTML for project selector
        let projectOptions = [];
        for (const org of orgs) {
            const projects = await invoke('list_org_projects', { org: org.login });
            if (projects && projects.length > 0) {
                projects.forEach(project => {
                    projectOptions.push({
                        id: project.id,
                        name: `${org.login} / ${project.title}`,
                        org: org.login,
                        title: project.title
                    });
                });
            }
        }

        if (projectOptions.length === 0) {
            showError('No projects found in any organization');
            return;
        }

        // Create a simple selection dialog
        const projectList = projectOptions.map((p, index) =>
            `${index + 1}. ${p.name}`
        ).join('\n');

        // For now, we'll use a simple prompt - in production you'd want a proper dialog
        const selection = prompt(`Select a project by number:\n\n${projectList}`);

        if (selection) {
            const index = parseInt(selection) - 1;
            if (index >= 0 && index < projectOptions.length) {
                const selected = projectOptions[index];
                await invoke('select_project', { projectId: selected.id });
                await loadProjectData(selected.id);
            }
        }
    } catch (error) {
        console.error('Failed to show project selector:', error);
        showError(`Failed to load projects: ${error}`);
    }
}

async function showProjectMenuWithData(orgProjects) {
    try {
        console.log('Processing pre-fetched project data for inline menu:', orgProjects);

        // Wait a bit for the menu to be created if it doesn't exist yet
        let projectsContainer = document.querySelector('.context-menu-expanded-section');
        if (!projectsContainer) {
            // Menu might not be created yet, wait for it
            await new Promise(resolve => setTimeout(resolve, 100));
            projectsContainer = document.querySelector('.context-menu-expanded-section');
        }

        if (!projectsContainer) {
            console.log('Projects container not found, menu might not be open');
            // Store the data for when the menu opens
            window.pendingProjectData = orgProjects;
            return;
        }

        // Clear loading state
        projectsContainer.innerHTML = '';

        // Process the projects data
        let hasProjects = false;

        for (const [orgLogin, projects] of Object.entries(orgProjects)) {
            if (projects && projects.length > 0) {
                hasProjects = true;

                // Create organization section
                const orgSection = document.createElement('div');
                orgSection.className = 'context-menu-section';

                const orgTitle = document.createElement('div');
                orgTitle.className = 'context-menu-org-title';
                orgTitle.textContent = orgLogin;
                orgSection.appendChild(orgTitle);

                // Add projects for this organization
                projects.forEach(project => {
                    const projectItem = document.createElement('div');
                    projectItem.className = 'context-menu-item context-menu-nested';
                    projectItem.textContent = project.title;

                    projectItem.addEventListener('click', async () => {
                        console.log('=====================================')
                        console.log('PROJECT SELECTED');
                        console.log('=====================================')
                        console.log('Selected project:', project.id, project.title);

                        removeContextMenu();

                        try {
                            console.log(`Invoking select_project with ID: ${project.id}`);
                            await invoke('select_project', { projectId: project.id });
                            console.log('Project selection command sent to backend');

                            console.log(`Loading project data for ID: ${project.id}`);
                            await loadProjectData(project.id);
                            console.log('Project data loaded successfully');
                        } catch (error) {
                            console.error('Failed to select project:', error);
                            showError(`Failed to select project: ${error}`);
                        }
                    });

                    orgSection.appendChild(projectItem);
                });

                projectsContainer.appendChild(orgSection);
            }
        }

        if (!hasProjects) {
            projectsContainer.innerHTML = '<div class="context-menu-no-items">No projects found</div>';
        }

        console.log('Project menu populated inline');

    } catch (error) {
        console.error('Failed to populate project menu:', error);
        const projectsContainer = document.querySelector('.context-menu-expanded-section');
        if (projectsContainer) {
            projectsContainer.innerHTML = '<div class="context-menu-error">Failed to load projects</div>';
        }
    }
}

async function toggleMyItems() {
    try {
        showOnlyMyItems = await invoke('toggle_my_items');
        console.log('My items filter toggled to:', showOnlyMyItems);

        // Get current username if we don't have it
        if (showOnlyMyItems && !currentUsername) {
            try {
                currentUsername = await invoke('current_user');
                console.log('Got current user:', currentUsername);
            } catch (error) {
                console.error('Failed to get current user:', error);
                showError('Failed to get current GitHub user. Filter may not work correctly.');
            }
        }

        if (currentProjectData) {
            renderProject();
        }
    } catch (error) {
        console.error('Failed to toggle my items filter:', error);
    }
}

async function refreshProject() {
    if (!currentProjectData) return;

    try {
        console.log('Refreshing project data...');
        updateStatus('Refreshing project...');
        await loadProjectData(currentProjectData.project.id);
        updateStatus('Project refreshed');
    } catch (error) {
        console.error('Failed to refresh project:', error);
        showError(`Failed to refresh project: ${error}`);
    }
}


function setupWindowDragging() {
    console.log('Setting up window dragging for frameless window...');

    // Wait a bit for Tauri APIs to be fully loaded
    setTimeout(async () => {
        try {
            // Check if Tauri API is available
            if (!window.__TAURI__) {
                console.error('Tauri API not available');
                return;
            }

            // Helper function to start dragging with delay to not interfere with double-clicks
            let dragTimer = null;
            const startDragging = async (e) => {
                // Don't interfere with card dragging
                if (isDragging || e.target.closest('.kanban-card')) {
                    return;
                }

                if (e.button === 0) { // Left mouse button
                    // Clear any existing drag timer
                    if (dragTimer) {
                        clearTimeout(dragTimer);
                        dragTimer = null;
                    }

                    // Delay drag to allow double-click to take precedence
                    dragTimer = setTimeout(async () => {
                        try {
                            console.log('Starting window drag...');
                            // Use Tauri v2 API structure
                            const { getCurrentWindow } = window.__TAURI__.window;
                            const appWindow = getCurrentWindow();
                            await appWindow.startDragging();
                            console.log('Window drag completed');
                        } catch (error) {
                            console.error('Failed to start window dragging:', error);
                            console.error('Error details:', error.message || error);
                        }
                        dragTimer = null;
                    }, 200); // 200ms delay to allow double-click detection
                }
            };

            // Helper function to cancel drag when double-click occurs
            const cancelDrag = () => {
                if (dragTimer) {
                    clearTimeout(dragTimer);
                    dragTimer = null;
                    console.log('Drag cancelled due to double-click');
                }
            };

            // Make minimized view draggable
            const minimizedView = document.getElementById('minimized-view');
            if (minimizedView) {
                console.log('Adding drag handler to minimized view');
                minimizedView.addEventListener('mousedown', startDragging);
                minimizedView.addEventListener('dblclick', cancelDrag);
            }

            // Make expanded view draggable via column headers and empty space
            document.addEventListener('mousedown', async (e) => {
                // Check if we're in expanded view
                const expandedView = document.getElementById('expanded-view');
                if (!expandedView || expandedView.classList.contains('hidden')) {
                    return;
                }

                // Allow dragging from column headers, window drag handle, or empty areas
                const isDragArea = (
                    e.target.closest('.column-header') ||
                    e.target.closest('#window-drag-handle') ||
                    e.target.id === 'kanban-board' ||
                    e.target.id === 'expanded-view'
                );

                if (isDragArea) {
                    await startDragging(e);
                }
            });

            console.log('Window dragging setup complete');
        } catch (error) {
            console.error('Error setting up window dragging:', error);
        }
    }, 100); // Small delay to ensure Tauri APIs are ready
}

// Listen for project selection from Rust backend
window.__TAURI__.event.listen('project-selected', async (event) => {
    await loadProjectData(event.payload.projectId);
});

// Listen for menu events
window.__TAURI__.event.listen('menu-refresh', async () => {
    if (currentProjectData) {
        await loadProjectData(currentProjectData.project.id);
    } else {
        await loadFirstAvailableProject();
    }
});

window.__TAURI__.event.listen('menu-toggle-expanded', async () => {
    await toggleView();
});

window.__TAURI__.event.listen('menu-select-project', async () => {
    // Show project selection dialog
    await showProjectSelector();
});