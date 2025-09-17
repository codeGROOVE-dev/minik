// Hierarchical Menu System for Minik
// This module handles the creation of hierarchical menus with submenus that expand to the right

const { invoke } = window.__TAURI__.core;

let menuTimeout = null;

// Helper function to dynamically adjust window height
async function adjustWindowHeight(height) {
    const { getCurrentWindow, LogicalSize } = window.__TAURI__.window;
    const currentWindow = getCurrentWindow();
    const size = new LogicalSize(600, height);
    await currentWindow.setSize(size);
}

// Helper function to remove all submenus
function removeAllSubmenus() {
    document.querySelectorAll('.context-submenu').forEach(menu => menu.remove());
}

// Helper function to create a submenu
function createSubmenu(parentItem, id) {
    console.log(`Creating submenu: ${id}`);
    removeAllSubmenus();

    const submenu = document.createElement('div');
    submenu.className = 'context-submenu';
    submenu.id = id;

    // Position it to the right of the parent item with overlap
    const rect = parentItem.getBoundingClientRect();
    submenu.style.left = `${rect.right - 5}px`; // Increased overlap from -2 to -5
    submenu.style.top = `${rect.top}px`;

    // Prevent submenu from closing when hovering over it
    submenu.addEventListener('mouseenter', (e) => {
        console.log(`Submenu ${id} mouseenter`);
        if (menuTimeout) {
            clearTimeout(menuTimeout);
            menuTimeout = null;
        }
        e.stopPropagation(); // Prevent event bubbling
    });

    submenu.addEventListener('mouseleave', (e) => {
        console.log(`Submenu ${id} mouseleave, relatedTarget:`, e.relatedTarget);

        // Check if we're moving to the parent menu item
        if (e.relatedTarget && e.relatedTarget.closest('.context-menu-item')) {
            console.log('Moving to menu item, not closing');
            return;
        }

        menuTimeout = setTimeout(() => {
            console.log('Checking if should close submenu...');
            // Use a simpler check - just see if this specific submenu is still being hovered
            if (!submenu.matches(':hover') && !parentItem.matches(':hover')) {
                console.log('Closing submenu - not hovering over it or parent');
                removeAllSubmenus();
            } else {
                console.log('Keeping submenu open - still hovering');
            }
        }, 150); // Slightly longer timeout for stability
    });

    return submenu;
}


// Create the main hierarchical context menu
async function showHierarchicalContextMenu(mouseX, mouseY) {
    console.log('Creating hierarchical context menu at', mouseX, mouseY);

    // Remove any existing menus
    if (window.removeContextMenu) {
        await window.removeContextMenu();
    }
    removeAllSubmenus();

    // Temporarily expand window to accommodate context menu
    const currentData = window.getCurrentProjectData ? window.getCurrentProjectData() : null;
    if (currentData && currentData.columns) {
        const hiddenColumns = currentData.hiddenColumns || [];
        const visibleColumns = currentData.columns.filter(
            col => !hiddenColumns.includes(col.id)
        );

        // Calculate current content height
        const board = document.getElementById('kanban-board');
        const expandedView = document.getElementById('expanded-view');
        let currentContentHeight = 0;

        if (expandedView && !expandedView.classList.contains('hidden')) {
            const columns = board.querySelectorAll('.kanban-column');
            columns.forEach(col => {
                const colRect = col.getBoundingClientRect();
                currentContentHeight = Math.max(currentContentHeight, colRect.height);
            });
            currentContentHeight += 12; // Add board padding
        }

        // Context menus need extra space
        // Estimate based on typical menu height (varies with content)
        // We'll add enough space for the menu to render without being cut off
        const estimatedMenuHeight = 300; // Approximate height for a typical menu
        const neededHeight = Math.max(currentContentHeight, mouseY + estimatedMenuHeight);

        // Add extra column width for the context menu (menu is ~250px wide)
        await invoke('resize_window_with_height', {
            columnCount: visibleColumns.length + 1, // Extra "column" worth of width for menu
            height: neededHeight
        });
    }

    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.innerHTML = '<div class="context-menu-content"></div>';

    const content = menu.querySelector('.context-menu-content');

    // Position the content div at mouse cursor
    if (mouseX !== undefined && mouseY !== undefined) {
        content.style.position = 'fixed';
        content.style.left = `${mouseX}px`;
        content.style.top = `${mouseY}px`;

        // After DOM insertion, adjust if menu goes off-screen
        setTimeout(async () => {
            const menuRect = content.getBoundingClientRect();
            const viewportWidth = window.innerWidth;
            const viewportHeight = window.innerHeight;

            // Adjust horizontal position if menu goes off right edge
            if (menuRect.right > viewportWidth) {
                content.style.left = `${Math.max(0, viewportWidth - menuRect.width - 10)}px`;
            }

            // Adjust vertical position if menu goes off bottom edge
            if (menuRect.bottom > viewportHeight) {
                content.style.top = `${Math.max(0, viewportHeight - menuRect.height - 10)}px`;
            }
        }, 0);
    }

    // Add title
    const title = document.createElement('div');
    title.className = 'context-menu-title';

    const currentProjectData = window.getCurrentProjectData ? window.getCurrentProjectData() : null;

    if (currentProjectData) {
        title.textContent = currentProjectData.project.title;
        title.style.cursor = 'pointer';
        title.addEventListener('click', async () => {
            const currentData = window.getCurrentProjectData();
            if (currentData && currentData.project.url) {
                open(currentData.project.url);
                if (window.removeContextMenu) await window.removeContextMenu();
            }
        });
    } else {
        title.textContent = 'Minik';
    }

    content.appendChild(title);

    // Add Projects menu item with hover submenu
    const projectsItem = document.createElement('div');
    projectsItem.className = 'context-menu-item context-menu-has-submenu';
    projectsItem.innerHTML = 'Projects <span class="submenu-arrow">›</span>';

    projectsItem.addEventListener('mouseenter', async () => {
        console.log('Projects menu item mouseenter');
        if (menuTimeout) {
            clearTimeout(menuTimeout);
            menuTimeout = null;
        }

        // Check if submenu already exists
        if (document.getElementById('projects-submenu')) {
            console.log('Projects submenu already exists, skipping creation');
            return;
        }

        const submenu = createSubmenu(projectsItem, 'projects-submenu');
        submenu.innerHTML = '<div class="context-menu-loading">Loading...</div>';
        document.body.appendChild(submenu);

        // Fetch and populate projects
        if (window.cachedProjectData) {
            populateProjectsSubmenu(submenu, window.cachedProjectData);
        } else {
            // Fetch projects
            try {
                const orgs = await invoke('list_organizations');
                const projectsByOrg = {};

                // Fetch all projects in parallel
                const promises = orgs.map(async (org) => {
                    try {
                        const projects = await invoke('list_org_projects', { org: org.login });
                        if (projects && projects.length > 0) {
                            projectsByOrg[org.login] = projects;
                        }
                    } catch (error) {
                        console.error(`Failed to fetch projects for ${org.login}:`, error);
                    }
                });

                await Promise.all(promises);
                window.cachedProjectData = projectsByOrg;
                populateProjectsSubmenu(submenu, projectsByOrg);
            } catch (error) {
                console.error('Failed to fetch organizations:', error);
                submenu.innerHTML = '<div class="context-menu-error">Failed to load projects</div>';
            }
        }
    });

    projectsItem.addEventListener('mouseleave', (e) => {
        console.log('Projects menu item mouseleave, relatedTarget:', e.relatedTarget);

        // If we're entering the submenu, don't set a timeout
        if (e.relatedTarget && e.relatedTarget.closest('#projects-submenu')) {
            console.log('Moving to projects submenu, not setting timeout');
            return;
        }

        menuTimeout = setTimeout(() => {
            const submenu = document.getElementById('projects-submenu');
            // Check if mouse is still over the menu item or its submenu
            if (!projectsItem.matches(':hover') && (!submenu || !submenu.matches(':hover'))) {
                console.log('Closing projects submenu');
                removeAllSubmenus();
            }
        }, 150);
    });

    content.appendChild(projectsItem);

    // Add Columns menu item with hover submenu (only if project is loaded)
    if (window.getCurrentProjectData && window.getCurrentProjectData()) {
        const columnsItem = document.createElement('div');
        columnsItem.className = 'context-menu-item context-menu-has-submenu';
        columnsItem.innerHTML = 'Columns <span class="submenu-arrow">›</span>';

        columnsItem.addEventListener('mouseenter', async () => {
            console.log('Columns menu item mouseenter');
            if (menuTimeout) {
                clearTimeout(menuTimeout);
                menuTimeout = null;
            }

            // Check if submenu already exists
            if (document.getElementById('columns-submenu')) {
                console.log('Columns submenu already exists, skipping creation');
                return;
            }

            const submenu = createSubmenu(columnsItem, 'columns-submenu');
            populateColumnsSubmenu(submenu, window.getCurrentProjectData());
            document.body.appendChild(submenu);
        });

        columnsItem.addEventListener('mouseleave', (e) => {
            console.log('Columns menu item mouseleave, relatedTarget:', e.relatedTarget);

            // If we're entering the submenu, don't set a timeout
            if (e.relatedTarget && e.relatedTarget.closest('#columns-submenu')) {
                console.log('Moving to columns submenu, not setting timeout');
                return;
            }

            menuTimeout = setTimeout(() => {
                const submenu = document.getElementById('columns-submenu');
                // Check if mouse is still over the menu item or its submenu
                if (!columnsItem.matches(':hover') && (!submenu || !submenu.matches(':hover'))) {
                    console.log('Closing columns submenu');
                    removeAllSubmenus();
                }
            }, 150);
        });

        content.appendChild(columnsItem);
    }


    // Append menu to body
    document.body.appendChild(menu);

    // Menu positioning is handled by the resize_for_context_menu command

    // Store the remove function globally so it can be called from anywhere
    window.removeContextMenu = async function() {
        const existingMenu = document.querySelector('.context-menu');
        if (existingMenu) {
            existingMenu.remove();
        }
        removeAllSubmenus();

        // Resize window back to actual content size
        const currentData = window.getCurrentProjectData ? window.getCurrentProjectData() : null;
        if (currentData && currentData.columns) {
            // Calculate actual content height like we do in calculateAndSetWindowHeight
            const board = document.getElementById('kanban-board');
            const expandedView = document.getElementById('expanded-view');

            if (expandedView && !expandedView.classList.contains('hidden')) {
                // Get the actual height of the tallest column
                const columns = board.querySelectorAll('.kanban-column');
                let maxHeight = 0;
                columns.forEach(col => {
                    const colRect = col.getBoundingClientRect();
                    const height = colRect.height;
                    if (height > maxHeight) {
                        maxHeight = height;
                    }
                });

                // Add padding from the board (6px top + 6px bottom = 12px)
                const totalHeight = Math.ceil(maxHeight + 12);

                const hiddenColumns = currentData.hiddenColumns || [];
                const visibleColumns = currentData.columns.filter(
                    col => !hiddenColumns.includes(col.id)
                );

                // Use resize_window_with_height to set the proper height
                await invoke('resize_window_with_height', {
                    columnCount: visibleColumns.length,
                    height: totalHeight
                });
            }
        }
    };

    // Define handlers with forward references
    let escHandler;

    // Close menu when clicking outside
    const clickHandler = async (e) => {
        // Check if click is outside menu and submenu
        const isInsideMenu = e.target.closest('.context-menu-content');
        const isInsideSubmenu = e.target.closest('.context-submenu');

        if (!isInsideMenu && !isInsideSubmenu) {
            if (window.removeContextMenu) await window.removeContextMenu();
            document.removeEventListener('mousedown', clickHandler, true);
            document.removeEventListener('contextmenu', contextHandler, true);
            document.removeEventListener('keydown', escHandler);
        }
    };

    const contextHandler = async (e) => {
        // Prevent default and close menu on any right-click
        e.preventDefault();
        if (window.removeContextMenu) await window.removeContextMenu();
        document.removeEventListener('mousedown', clickHandler, true);
        document.removeEventListener('contextmenu', contextHandler, true);
        document.removeEventListener('keydown', escHandler);
    };

    // Add ESC key handler
    escHandler = async (e) => {
        if (e.key === 'Escape') {
            if (window.removeContextMenu) await window.removeContextMenu();
            document.removeEventListener('keydown', escHandler);
            document.removeEventListener('mousedown', clickHandler, true);
            document.removeEventListener('contextmenu', contextHandler, true);
        }
    };

    // Use mousedown for more immediate response and capture phase
    setTimeout(() => {
        document.addEventListener('mousedown', clickHandler, true);
        document.addEventListener('contextmenu', contextHandler, true);
        document.addEventListener('keydown', escHandler);
    }, 10);
}

// Populate projects submenu
function populateProjectsSubmenu(submenu, projectsByOrg) {
    submenu.innerHTML = '';

    const hasProjects = Object.keys(projectsByOrg).length > 0;

    if (!hasProjects) {
        submenu.innerHTML = '<div class="context-menu-no-items">No projects found</div>';
        return;
    }

    for (const [orgLogin, projects] of Object.entries(projectsByOrg)) {
        // Add organization header
        const orgHeader = document.createElement('div');
        orgHeader.className = 'context-submenu-header';
        orgHeader.textContent = orgLogin;
        submenu.appendChild(orgHeader);

        // Add projects
        projects.forEach(project => {
            const projectItem = document.createElement('div');
            projectItem.className = 'context-menu-item';
            projectItem.textContent = project.title;

            // Check if this is the current project
            const currentData = window.getCurrentProjectData ? window.getCurrentProjectData() : null;
            if (currentData && currentData.project.id === project.id) {
                projectItem.classList.add('context-menu-item-selected');
            }

            projectItem.addEventListener('click', async () => {
                console.log('Selected project:', project.id, project.title);
                if (window.removeContextMenu) await window.removeContextMenu();
                removeAllSubmenus();

                try {
                    await invoke('select_project', { projectId: project.id });
                    if (window.loadProjectData) {
                        await window.loadProjectData(project.id);
                    }
                } catch (error) {
                    console.error('Failed to select project:', error);
                    if (window.showError) {
                        window.showError(`Failed to select project: ${error}`);
                    }
                }
            });

            submenu.appendChild(projectItem);
        });
    }
}

// Populate columns submenu
function populateColumnsSubmenu(submenu, projectData) {
    submenu.innerHTML = '';

    const hiddenColumns = projectData.hiddenColumns || [];

    projectData.columns.forEach((column, index) => {
        const columnItem = document.createElement('div');
        columnItem.className = 'context-menu-item context-menu-checkbox-item';

        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = !hiddenColumns.includes(column.id);
        checkbox.id = `column-check-${column.id}`;

        const label = document.createElement('label');
        label.htmlFor = `column-check-${column.id}`;
        label.textContent = column.name;

        columnItem.appendChild(checkbox);
        columnItem.appendChild(label);

        // Color indicator
        const colorIndicator = document.createElement('span');
        const COLUMN_COLORS = window.COLUMN_COLORS || ['yellow', 'blue', 'green', 'pink', 'orange', 'purple'];
        const colorClass = COLUMN_COLORS[index % COLUMN_COLORS.length];
        colorIndicator.className = `column-color-indicator column-${colorClass}`;
        columnItem.appendChild(colorIndicator);

        // Handle checkbox change directly
        checkbox.addEventListener('change', async (e) => {
            e.stopPropagation(); // Prevent event bubbling

            try {
                if (checkbox.checked) {
                    await invoke('show_column', {
                        projectId: projectData.project.id,
                        columnId: column.id
                    });
                } else {
                    await invoke('hide_column', {
                        projectId: projectData.project.id,
                        columnId: column.id
                    });
                }

                // Update local state
                if (checkbox.checked) {
                    const idx = hiddenColumns.indexOf(column.id);
                    if (idx > -1) {
                        hiddenColumns.splice(idx, 1);
                    }
                } else {
                    if (!hiddenColumns.includes(column.id)) {
                        hiddenColumns.push(column.id);
                    }
                }

                // Update the currentProjectData
                const currentData = window.getCurrentProjectData();
                if (currentData) {
                    currentData.hiddenColumns = hiddenColumns;
                    window.setCurrentProjectData(currentData);
                }
                if (window.renderProject) {
                    window.renderProject();
                }
            } catch (error) {
                console.error('Failed to toggle column visibility:', error);
                checkbox.checked = !checkbox.checked;
            }
        });

        // Handle click on the item (but not on checkbox)
        columnItem.addEventListener('click', (e) => {
            // Only toggle if we didn't click the checkbox itself
            if (e.target !== checkbox && e.target.type !== 'checkbox') {
                e.preventDefault();
                checkbox.checked = !checkbox.checked;
                // Trigger the change event
                checkbox.dispatchEvent(new Event('change'));
            }
        });

        submenu.appendChild(columnItem);
    });
}


// Export the new menu function
window.showHierarchicalContextMenu = showHierarchicalContextMenu;