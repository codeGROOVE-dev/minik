mod github;
mod logging;

use github::{GitHubClient, Organization, Project, ProjectData};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State, AppHandle, WindowEvent, Emitter, PhysicalPosition};
use tauri::menu::{MenuItemBuilder, SubmenuBuilder, Menu};

#[derive(Serialize, Deserialize, Clone)]
struct AppState {
    selected_project_id: Option<String>,
    is_expanded: bool,
    show_only_my_items: bool,
    hidden_columns: Vec<String>,
    window_x: i32,
    window_y: i32,
    #[serde(default)]
    last_column_count: u32,
    #[serde(default)]
    status_field_id: String,
    #[serde(default)]
    project_column_settings: std::collections::HashMap<String, Vec<String>>, // project_id -> hidden columns
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            selected_project_id: None,
            is_expanded: false,
            show_only_my_items: false,
            hidden_columns: Vec::new(),
            window_x: 100,
            window_y: 50,
            last_column_count: 5,
            status_field_id: String::new(),
            project_column_settings: std::collections::HashMap::new(),
        }
    }
}

struct AppStateWrapper(Mutex<AppState>);

#[tauri::command]
async fn github_token() -> Result<String, String> {
    log::info!("github_token command called from frontend");
    log::debug!("Checking GitHub authentication");
    let result = GitHubClient::new()
        .map(|_| "authenticated".to_string())
        .map_err(|e| {
            log::error!("GitHub authentication failed: {}", e);
            e.to_string()
        });
    if result.is_ok() {
        log::info!("GitHub authentication successful");
    }
    result
}

#[tauri::command]
async fn list_organizations() -> Result<Vec<Organization>, String> {
    log::debug!("Listing GitHub organizations");
    let client = GitHubClient::new().map_err(|e| {
        log::error!("Failed to create GitHub client: {}", e);
        e.to_string()
    })?;
    let result = client
        .list_organizations()
        .await
        .map_err(|e| {
            log::error!("Failed to list organizations: {}", e);
            e.to_string()
        });
    if let Ok(ref orgs) = result {
        log::info!("Successfully fetched {} organizations", orgs.len());
        for org in orgs {
            log::debug!("  Organization: {} (id: {})", org.login, org.id);
        }
    }
    result
}

#[tauri::command]
async fn list_org_projects(org: String) -> Result<Vec<Project>, String> {
    log::debug!("Listing projects for organization: {}", org);
    let client = GitHubClient::new().map_err(|e| {
        log::error!("Failed to create GitHub client: {}", e);
        e.to_string()
    })?;
    let result = client
        .list_org_projects(&org)
        .await
        .map_err(|e| {
            log::error!("Failed to list projects for org {}: {}", org, e);
            e.to_string()
        });
    if let Ok(ref projects) = result {
        log::info!("Successfully fetched {} projects for org {}", projects.len(), org);
    }
    result
}

#[tauri::command]
async fn project_data(project_id: String, state: State<'_, AppStateWrapper>, app_handle: AppHandle) -> Result<ProjectData, String> {
    log::debug!("Fetching data for project: {}", project_id);
    let client = GitHubClient::new().map_err(|e| {
        log::error!("Failed to create GitHub client: {}", e);
        e.to_string()
    })?;
    let mut result = client
        .project_data(&project_id)
        .await
        .map_err(|e| {
            log::error!("Failed to fetch project data for {}: {}", project_id, e);
            e.to_string()
        })?;

    // Add the hidden columns information from the current state
    {
        let app_state = state.0.lock().unwrap();
        result.hidden_columns = app_state.hidden_columns.clone();
    }

    log::info!("Successfully fetched project '{}' with {} columns and {} items",
              result.project.title, result.columns.len(), result.items.len());

    // Store the column count and field ID for later use
    let mut app_state = state.0.lock().unwrap();
    app_state.last_column_count = result.columns.len() as u32;
    app_state.status_field_id = result.status_field_id.clone();

    // Update the hide columns menu dynamically
    if let Err(e) = update_column_menu(&app_handle, &result.columns, &app_state.hidden_columns) {
        log::error!("Failed to update column menu: {}", e);
    }

    Ok(result)
}

#[tauri::command]
async fn update_item_column(project_id: String, item_id: String, column_id: String, state: State<'_, AppStateWrapper>) -> Result<(), String> {
    log::info!("\n🎯🎯🎯 UPDATE_ITEM_COLUMN COMMAND CALLED 🎯🎯🎯");
    log::info!("  Project ID: {}", project_id);
    log::info!("  Item ID: {}", item_id);
    log::info!("  Target Column ID: {}", column_id);

    let field_id = {
        let app_state = state.0.lock().unwrap();
        let field_id = app_state.status_field_id.clone();
        log::info!("  Retrieved Status Field ID from state: '{}'", field_id);
        field_id
    };

    if field_id.is_empty() {
        log::error!("❌ Status field ID is empty! Cannot proceed with update.");
        return Err("Status field ID not found - please refresh the project".to_string());
    }

    log::info!("📞 Creating GitHub client...");
    let client = GitHubClient::new().map_err(|e| {
        log::error!("❌ Failed to create GitHub client: {}", e);
        e.to_string()
    })?;
    log::info!("✅ GitHub client created successfully");

    log::info!("🚀 Calling update_item_field on GitHub client...");
    let result = client
        .update_item_field(&project_id, &item_id, &field_id, &column_id)
        .await;

    match result {
        Ok(_) => {
            log::info!("✅✅✅ Successfully updated item column on GitHub!");
            Ok(())
        }
        Err(e) => {
            log::error!("❌❌❌ Failed to update item column: {}", e);
            Err(format!("GitHub API error: {}", e))
        }
    }
}

#[tauri::command]
fn toggle_expanded(state: State<AppStateWrapper>, _app_handle: AppHandle) -> Result<bool, String> {
    log::debug!("Toggling window expanded state");
    let mut app_state = state.0.lock().unwrap();
    app_state.is_expanded = !app_state.is_expanded;
    let is_expanded = app_state.is_expanded;
    let column_count = app_state.last_column_count;
    log::info!("Window expanded state changed to: {}, columns: {}", is_expanded, column_count);

    // Note: Window sizing is now handled dynamically by JavaScript
    // Rust only handles the state toggle, not the sizing
    log::info!("Expanded state toggled to: {}, JavaScript will handle dynamic sizing", is_expanded);

    Ok(is_expanded)
}

#[tauri::command]
fn resize_window_for_columns(column_count: u32, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Resizing window for {} columns", column_count);

    if let Some(window) = app_handle.get_webview_window("main") {
        // Calculate width: base padding + (column width * count) + gaps
        // Using Logical size to handle HiDPI displays correctly
        let column_width = 190; // Width per column
        let padding = 12; // Total padding
        let gap = 4; // Gap between columns

        // Calculate exact width needed for visible columns
        let width = padding + (column_width * column_count) + (gap * column_count.saturating_sub(1));
        // No artificial max width limit

        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: 480.0,  // Compact height
        }));
        log::info!("Window resized to {} x 480 for {} columns", width, column_count);
    }

    Ok(())
}

#[tauri::command]
fn resize_window_with_height(column_count: u32, height: u32, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Resizing window for {} columns with height {}", column_count, height);

    if let Some(window) = app_handle.get_webview_window("main") {
        // Calculate width: base padding + (column width * count) + gaps
        let column_width = 190; // Width per column
        let padding = 12; // Total padding
        let gap = 4; // Gap between columns

        // Calculate exact width needed for visible columns
        let width = padding + (column_width * column_count) + (gap * column_count.saturating_sub(1));

        // Use the exact height from JavaScript without any artificial caps
        let final_height = height;

        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: final_height as f64,
        }));
        log::info!("Window resized to {} x {} for {} columns", width, final_height, column_count);
    }

    Ok(())
}

#[tauri::command]
fn resize_window_to_dimensions(width: u32, height: u32, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Resizing window to exact dimensions: {}x{}", width, height);

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: height as f64,
        }));
        log::info!("Window resized to exact dimensions: {} x {}", width, height);
    }

    Ok(())
}



#[tauri::command]
fn resize_for_context_menu(column_count: u32, show_menu: bool, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Resizing for context menu: show={}, columns={}", show_menu, column_count);

    if let Some(window) = app_handle.get_webview_window("main") {
        let column_width = 190;
        let padding = 12;
        let gap = 4;

        // Calculate base width needed for visible columns
        let content_width = padding + (column_width * column_count) + (gap * column_count.saturating_sub(1));

        // Add extra space for context menu only when shown
        let width = if show_menu {
            // Context menus are max 250px wide, add buffer for submenu
            content_width + 260
        } else {
            content_width
        };

        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: 480.0,
        }));
        log::info!("Window resized to {}x480 (menu: {})", width, show_menu);
    }

    Ok(())
}

#[tauri::command]
fn select_project(project_id: String, state: State<AppStateWrapper>, app_handle: AppHandle) -> Result<(), String> {
    log::info!("Selecting project: {}", project_id);
    let mut app_state = state.0.lock().unwrap();
    let old_project = app_state.selected_project_id.clone();

    // Save current project's hidden columns
    if let Some(old_id) = old_project {
        let hidden_cols = app_state.hidden_columns.clone();
        app_state.project_column_settings.insert(old_id, hidden_cols);
    }

    // Load new project's hidden columns
    app_state.selected_project_id = Some(project_id.clone());
    app_state.hidden_columns = app_state.project_column_settings
        .get(&project_id)
        .cloned()
        .unwrap_or_default();

    save_state(&app_state);

    // Emit event to reload project data
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.emit("project-changed", project_id.clone());
    }

    log::debug!("Project {} selected and state saved", project_id);
    Ok(())
}

#[tauri::command]
fn current_project(state: State<AppStateWrapper>) -> Option<String> {
    let app_state = state.0.lock().unwrap();
    app_state.selected_project_id.clone()
}

#[tauri::command]
fn toggle_my_items(state: State<AppStateWrapper>) -> Result<bool, String> {
    let mut app_state = state.0.lock().unwrap();
    app_state.show_only_my_items = !app_state.show_only_my_items;
    save_state(&app_state);
    Ok(app_state.show_only_my_items)
}

#[tauri::command]
fn toggle_column_visibility(column_id: String, state: State<AppStateWrapper>) -> Result<bool, String> {
    let mut app_state = state.0.lock().unwrap();
    let is_visible = if let Some(index) = app_state.hidden_columns.iter().position(|c| c == &column_id) {
        app_state.hidden_columns.remove(index);
        true
    } else {
        app_state.hidden_columns.push(column_id.clone());
        false
    };

    // Also update the project-specific settings
    let hidden_cols = app_state.hidden_columns.clone();
    if let Some(project_id) = app_state.selected_project_id.clone() {
        app_state.project_column_settings.insert(project_id, hidden_cols);
    }

    save_state(&app_state);
    Ok(is_visible)
}

#[tauri::command]
fn hide_column(project_id: String, column_id: String, state: State<AppStateWrapper>) -> Result<(), String> {
    log::info!("Hiding column {} for project {}", column_id, project_id);
    let mut app_state = state.0.lock().unwrap();

    // Add to hidden columns if not already hidden
    if !app_state.hidden_columns.contains(&column_id) {
        app_state.hidden_columns.push(column_id.clone());
    }

    // Update project-specific settings
    let hidden_cols = app_state.hidden_columns.clone();
    app_state.project_column_settings.insert(project_id, hidden_cols);

    save_state(&app_state);
    log::debug!("Column {} hidden successfully", column_id);
    Ok(())
}

#[tauri::command]
fn show_column(project_id: String, column_id: String, state: State<AppStateWrapper>) -> Result<(), String> {
    log::info!("Showing column {} for project {}", column_id, project_id);
    let mut app_state = state.0.lock().unwrap();

    // Remove from hidden columns
    if let Some(index) = app_state.hidden_columns.iter().position(|c| c == &column_id) {
        app_state.hidden_columns.remove(index);
    }

    // Update project-specific settings
    let hidden_cols = app_state.hidden_columns.clone();
    app_state.project_column_settings.insert(project_id, hidden_cols);

    save_state(&app_state);
    log::debug!("Column {} shown successfully", column_id);
    Ok(())
}

#[tauri::command]
fn hidden_columns(state: State<AppStateWrapper>) -> Vec<String> {
    let app_state = state.0.lock().unwrap();
    app_state.hidden_columns.clone()
}

#[tauri::command]
fn is_expanded(state: State<AppStateWrapper>) -> bool {
    let app_state = state.0.lock().unwrap();
    app_state.is_expanded
}

#[tauri::command]
fn show_only_my_items(state: State<AppStateWrapper>) -> bool {
    let app_state = state.0.lock().unwrap();
    app_state.show_only_my_items
}

/// Find the gh command in common locations
fn find_gh_command() -> Result<String, String> {
    let possible_paths = vec![
        "/opt/homebrew/bin/gh",  // Apple Silicon Homebrew
        "/usr/local/bin/gh",      // Intel Homebrew
        "gh",                      // System PATH
    ];

    for path in &possible_paths {
        let check = std::process::Command::new(path)
            .arg("--version")
            .output();

        if let Ok(output) = check {
            if output.status.success() {
                log::debug!("Found gh at: {}", path);
                return Ok(path.to_string());
            }
        }
    }

    log::error!("Could not find gh CLI in any common location");
    Err("GitHub CLI (gh) not found. Please install it with 'brew install gh' and authenticate with 'gh auth login'".to_string())
}

#[tauri::command]
async fn current_user() -> Result<String, String> {
    log::info!("Fetching current user from GitHub");
    let gh_path = find_gh_command()?;
    let output = std::process::Command::new(&gh_path)
        .args(["api", "user", "--jq", ".login"])
        .output()
        .map_err(|e| format!("Failed to execute gh command: {}", e))?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        log::error!("Failed to get current user: {}", error);
        return Err(format!("Failed to get current user: {}", error));
    }

    let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!("Current GitHub user: {}", username);
    Ok(username)
}

fn save_state(state: &AppState) {
    log::debug!("Saving application state");
    match serde_json::to_string(state) {
        Ok(json) => {
            let path = dirs::config_dir()
                .map(|p| p.join("minik").join("state.json"));

            if let Some(path) = path {
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        log::error!("Failed to create config directory: {}", e);
                        return;
                    }
                }
                match std::fs::write(&path, json) {
                    Ok(_) => log::debug!("State saved successfully to {:?}", path),
                    Err(e) => log::error!("Failed to write state to {:?}: {}", path, e),
                }
            } else {
                log::error!("Could not determine config directory");
            }
        }
        Err(e) => log::error!("Failed to serialize state: {}", e),
    }
}

fn load_state() -> AppState {
    log::debug!("Loading application state");
    let path = dirs::config_dir()
        .map(|p| p.join("minik").join("state.json"));

    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(json) => {
                match serde_json::from_str(&json) {
                    Ok(state) => {
                        log::info!("State loaded successfully from {:?}", path);
                        return state;
                    }
                    Err(e) => log::warn!("Failed to parse state file: {}", e),
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to read state file: {}", e);
                } else {
                    log::debug!("No existing state file found, using defaults");
                }
            }
        }
    } else {
        log::warn!("Could not determine config directory");
    }

    log::info!("Using default application state");
    AppState::default()
}

// Command to update project menu dynamically
#[tauri::command]
async fn update_project_menu(app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Updating project menu");
    rebuild_project_menu(&app_handle).await?;
    Ok(())
}

// Command to update columns menu dynamically
#[tauri::command]
async fn update_columns_menu(columns: Vec<github::ProjectColumn>, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Updating columns menu with {} columns", columns.len());
    rebuild_columns_menu(&app_handle, columns)?;
    Ok(())
}

// Context menu commands
#[tauri::command]
async fn show_project_context_menu(app_handle: AppHandle) -> Result<(), String> {
    use futures::future::join_all;
    log::debug!("Showing project context menu");

    // Get organizations first
    let orgs = list_organizations().await.map_err(|e| format!("Failed to get organizations: {}", e))?;

    // Fetch all projects in parallel
    let org_project_futures: Vec<_> = orgs.iter().map(|org| {
        let org_login = org.login.clone();
        async move {
            let projects = list_org_projects(org_login.clone()).await.unwrap_or_default();
            (org_login, projects)
        }
    }).collect();

    let org_projects = join_all(org_project_futures).await;

    // Build a simple HashMap-like structure for the frontend
    let projects_by_org: std::collections::HashMap<String, Vec<serde_json::Value>> =
        org_projects.into_iter()
            .filter(|(_, projects)| !projects.is_empty())
            .map(|(org_login, projects)| {
                let project_values: Vec<serde_json::Value> = projects.into_iter()
                    .map(|p| serde_json::to_value(p).unwrap_or(serde_json::Value::Null))
                    .collect();
                (org_login, project_values)
            })
            .collect();

    log::info!("Sending projects to frontend: {:?}", projects_by_org.keys().collect::<Vec<_>>());

    if let Some(window) = app_handle.get_webview_window("main") {
        let result = window.emit("show-project-context-menu-with-projects", &projects_by_org);
        log::info!("Event emit result: {:?}", result);
    }

    Ok(())
}

#[tauri::command]
async fn show_column_context_menu(project_id: String, app_handle: AppHandle, state: State<'_, AppStateWrapper>) -> Result<(), String> {
    log::debug!("Showing column context menu for project: {}", project_id);

    // Get project data to build the context menu
    let project_data = project_data(project_id.clone(), state, app_handle.clone()).await?;

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.emit("show-column-context-menu", (project_id, project_data.columns));
    }

    Ok(())
}

async fn rebuild_project_menu<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> Result<(), String> {
    // For now, we'll emit events to the frontend to handle project selection
    // Dynamic menu updates in Tauri v2 are complex and require rebuilding the entire menu
    log::info!("Project menu update requested - using frontend modal instead");
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.emit("show-project-selector", ());
    }
    Ok(())
}

fn rebuild_columns_menu<R: tauri::Runtime>(
    app_handle: &AppHandle<R>,
    columns: Vec<github::ProjectColumn>,
) -> Result<(), String> {
    // Store columns data for frontend use
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.emit("columns-updated", columns);
    }
    Ok(())
}

fn setup_app_menu<R: tauri::Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{PredefinedMenuItem, CheckMenuItem};

    // Create View menu items
    let refresh = MenuItemBuilder::new("Refresh Project")
        .id("refresh")
        .accelerator("CmdOrCtrl+R")
        .build(app)?;

    let toggle_my_items = CheckMenuItem::with_id(
        app,
        "toggle-my-items",
        "Show Only My Items",
        true,
        false,
        Some("CmdOrCtrl+M"),
    )?;

    let toggle_expanded = MenuItemBuilder::new("Toggle Expanded View")
        .id("toggle-expanded")
        .accelerator("CmdOrCtrl+E")
        .build(app)?;

    let open_devtools = MenuItemBuilder::new("Open Developer Tools")
        .id("open-devtools")
        .accelerator("CmdOrCtrl+Option+I")
        .build(app)?;

    // Create View menu
    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&refresh)
        .separator()
        .item(&toggle_my_items)
        .item(&toggle_expanded)
        .separator()
        .item(&open_devtools)
        .build()?;

    // Create simple Project menu (context menu will handle project selection)
    let current_project = MenuItemBuilder::new("No project selected")
        .id("current-project")
        .enabled(false)
        .build(app)?;
    let select_project = MenuItemBuilder::new("Right-click to select project")
        .id("select-project-help")
        .enabled(false)
        .build(app)?;

    let project_menu = SubmenuBuilder::new(app, "Project")
        .item(&current_project)
        .item(&select_project)
        .build()?;

    // Create simple Columns menu (dynamic context menus will handle column toggles)
    let columns_help = MenuItemBuilder::new("Right-click columns to show/hide")
        .id("columns-help")
        .enabled(false)
        .build(app)?;

    let columns_menu = SubmenuBuilder::new(app, "Columns")
        .item(&columns_help)
        .build()?;

    // Build main menu
    #[cfg(target_os = "macos")]
    {
        let app_menu = SubmenuBuilder::new(app, "Minik")
            .item(&PredefinedMenuItem::about(app, Some("Minik"), None)?)
            .separator()
            .item(&PredefinedMenuItem::services(app, None)?)
            .separator()
            .item(&PredefinedMenuItem::hide(app, None)?)
            .item(&PredefinedMenuItem::hide_others(app, None)?)
            .item(&PredefinedMenuItem::show_all(app, None)?)
            .separator()
            .item(&PredefinedMenuItem::quit(app, None)?)
            .build()?;

        let edit_menu = SubmenuBuilder::new(app, "Edit")
            .item(&PredefinedMenuItem::undo(app, None)?)
            .item(&PredefinedMenuItem::redo(app, None)?)
            .separator()
            .item(&PredefinedMenuItem::cut(app, None)?)
            .item(&PredefinedMenuItem::copy(app, None)?)
            .item(&PredefinedMenuItem::paste(app, None)?)
            .item(&PredefinedMenuItem::select_all(app, None)?)
            .build()?;

        let window_menu = SubmenuBuilder::new(app, "Window")
            .item(&PredefinedMenuItem::minimize(app, None)?)
            .build()?;

        let menu = Menu::with_items(app, &[
            &app_menu,
            &edit_menu,
            &view_menu,
            &project_menu,
            &columns_menu,
            &window_menu,
        ])?;

        app.set_menu(menu)?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let file_menu = SubmenuBuilder::new(app, "File")
            .item(&PredefinedMenuItem::quit(app, None)?)
            .build()?;

        let edit_menu = SubmenuBuilder::new(app, "Edit")
            .item(&PredefinedMenuItem::cut(app)?)
            .item(&PredefinedMenuItem::copy(app)?)
            .item(&PredefinedMenuItem::paste(app)?)
            .build()?;

        let menu = Menu::with_items(app, &[
            &file_menu,
            &edit_menu,
            &view_menu,
            &project_menu,
        ])?;

        app.set_menu(menu)?;
    }

    // Set up menu event handler
    app.on_menu_event(move |app_handle, event| {
        log::debug!("Menu event: {:?}", event.id());

        match event.id().as_ref() {
            "refresh" => {
                log::info!("Refresh menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-refresh", ());
                }
            }
            "toggle-my-items" => {
                log::info!("Toggle my items menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-toggle-my-items", ());
                }
            }
            "toggle-expanded" => {
                log::info!("Toggle expanded menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-toggle-expanded", ());
                }
            }
            "open-devtools" => {
                log::info!("Open developer tools menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    #[cfg(debug_assertions)]
                    {
                        window.open_devtools();
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        let _ = window; // Suppress unused variable warning
                        log::warn!("Devtools not available in release builds");
                    }
                }
            }
            "select-project" => {
                log::info!("Select project menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-select-project", ());
                }
            }
            id if id.starts_with("project-") => {
                log::info!("Project selected: {}", id);
                let project_id = id.strip_prefix("project-").unwrap_or("").to_string();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-project-selected", project_id);
                }
            }
            "columns-show-all" => {
                log::info!("Show all columns selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-columns-show-all", ());
                }
            }
            "columns-hide-all" => {
                log::info!("Hide all columns selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-columns-hide-all", ());
                }
            }
            id if id.starts_with("column-") => {
                log::info!("Column visibility toggle: {}", id);
                let column_id = id.strip_prefix("column-").unwrap_or("").to_string();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-toggle-column", column_id);
                }
            }
            _ => {}
        }
    });

    Ok(())
}

fn update_column_menu<R: tauri::Runtime>(
    _app_handle: &AppHandle<R>,
    _columns: &[crate::github::ProjectColumn],
    _hidden_columns: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    // Column menu dynamic update is complex in Tauri v2
    // For now, we'll skip dynamic menu updates
    // The column visibility can still be toggled through state
    log::debug!("Column menu update skipped (not yet implemented)");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging first
    if let Err(e) = logging::init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    log::info!("Starting Minik application");
    let state = load_state();

    tauri::Builder::default()
        .manage(AppStateWrapper(Mutex::new(state)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            github_token,
            list_organizations,
            list_org_projects,
            project_data,
            update_item_column,
            toggle_expanded,
            resize_window_for_columns,
            resize_window_with_height,
            resize_window_to_dimensions,
            resize_for_context_menu,
            select_project,
            current_project,
            toggle_my_items,
            toggle_column_visibility,
            hide_column,
            show_column,
            hidden_columns,
            is_expanded,
            show_only_my_items,
            current_user,
            update_project_menu,
            update_columns_menu,
            show_project_context_menu,
            show_column_context_menu,
        ])
        .setup(|app| {
            let _app_handle = app.handle().clone();

            // Build the application menu
            setup_app_menu(app)?;

            let window = app.get_webview_window("main").unwrap();


            // Restore window position from state
            if let Some(state_wrapper) = app.try_state::<AppStateWrapper>() {
                let app_state = state_wrapper.0.lock().unwrap();
                if app_state.window_x != 100 || app_state.window_y != 50 {
                    let _ = window.set_position(PhysicalPosition::new(app_state.window_x, app_state.window_y));
                    log::info!("Restored window position to ({}, {})", app_state.window_x, app_state.window_y);
                }
            }

            let app_handle_clone = app.handle().clone();
            window.on_window_event(move |event| {
                match event {
                    WindowEvent::Focused(false) => {
                        log::debug!("Window lost focus");
                    }
                    WindowEvent::Focused(true) => {
                        log::debug!("Window gained focus");
                    }
                    WindowEvent::Resized(size) => {
                        log::debug!("Window resized to: {:?}", size);
                    }
                    WindowEvent::Moved(position) => {
                        log::debug!("Window moved to: {:?}", position);
                        // Save window position
                        if let Some(state_wrapper) = app_handle_clone.try_state::<AppStateWrapper>() {
                            let mut app_state = state_wrapper.0.lock().unwrap();
                            app_state.window_x = position.x;
                            app_state.window_y = position.y;
                            save_state(&app_state);
                        }
                    }
                    _ => {
                        log::trace!("Window event: {:?}", event);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    log::info!("Minik application shutting down");
}