mod github;
mod logging;

use github::{GitHubClient, Organization, Project, ProjectData};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State, AppHandle, WindowEvent, Emitter};
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
    let result = client
        .project_data(&project_id)
        .await
        .map_err(|e| {
            log::error!("Failed to fetch project data for {}: {}", project_id, e);
            e.to_string()
        });
    if let Ok(ref data) = result {
        log::info!("Successfully fetched project '{}' with {} columns and {} items",
                  data.project.title, data.columns.len(), data.items.len());
        // Store the column count and field ID for later use
        let mut app_state = state.0.lock().unwrap();
        app_state.last_column_count = data.columns.len() as u32;
        app_state.status_field_id = data.status_field_id.clone();

        // Update the hide columns menu dynamically
        if let Err(e) = update_column_menu(&app_handle, &data.columns, &app_state.hidden_columns) {
            log::error!("Failed to update column menu: {}", e);
        }
    }
    result
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
fn toggle_expanded(state: State<AppStateWrapper>, app_handle: AppHandle) -> Result<bool, String> {
    log::debug!("Toggling window expanded state");
    let mut app_state = state.0.lock().unwrap();
    app_state.is_expanded = !app_state.is_expanded;
    let is_expanded = app_state.is_expanded;
    let column_count = app_state.last_column_count;
    log::info!("Window expanded state changed to: {}, columns: {}", is_expanded, column_count);

    if let Some(window) = app_handle.get_webview_window("main") {
        if is_expanded {
            // Calculate dynamic width based on column count
            // Using Logical size instead of Physical to handle HiDPI displays correctly
            let column_width = 190; // Width per column
            let padding = 12; // Total padding (bare minimum)
            let gap = 4; // Gap between columns (bare minimum)
            let width = padding + (column_width * column_count) + (gap * column_count.saturating_sub(1));
            let width = width.min(1200).max(600); // Clamp between 600 and 1200

            log::info!("Setting expanded window size to {}x480 for {} columns", width, column_count);
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: width as f64,
                height: 480.0,
            }));
            let _ = window.set_resizable(true);
        } else {
            // Increased width to show all column badges properly
            let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: 600.0,  // Increased from 400 to accommodate column badges
                height: 60.0,
            }));
            let _ = window.set_resizable(false);
        }
    }

    Ok(is_expanded)
}

#[tauri::command]
fn resize_window_for_columns(column_count: u32, app_handle: AppHandle) -> Result<(), String> {
    log::debug!("Resizing window for {} columns", column_count);

    if let Some(window) = app_handle.get_webview_window("main") {
        // Calculate width: base padding + (column width * count) + gaps
        // Using Logical size to handle HiDPI displays correctly
        let column_width = 190; // Width per column
        let padding = 12; // Total padding (bare minimum)
        let gap = 4; // Gap between columns (bare minimum)
        let width = padding + (column_width * column_count) + (gap * (column_count - 1).max(0));
        let width = width.min(1200).max(600); // Clamp between 600 and 1200

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
        let padding = 12; // Total padding (bare minimum)
        let gap = 4; // Gap between columns (bare minimum)
        let width = padding + (column_width * column_count) + (gap * (column_count - 1).max(0));
        let width = width.min(1200).max(600); // Clamp between 600 and 1200

        // Use the calculated height from JavaScript, but cap it at 480 max
        let final_height = height.min(480);

        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: width as f64,
            height: final_height as f64,
        }));
        log::info!("Window resized to {} x {} for {} columns", width, final_height, column_count);
    }

    Ok(())
}

#[tauri::command]
fn select_project(project_id: String, state: State<AppStateWrapper>) -> Result<(), String> {
    log::info!("Selecting project: {}", project_id);
    let mut app_state = state.0.lock().unwrap();
    app_state.selected_project_id = Some(project_id.clone());
    save_state(&app_state);
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
fn toggle_column_visibility(column_id: String, state: State<AppStateWrapper>) -> Result<(), String> {
    let mut app_state = state.0.lock().unwrap();
    if let Some(index) = app_state.hidden_columns.iter().position(|c| c == &column_id) {
        app_state.hidden_columns.remove(index);
    } else {
        app_state.hidden_columns.push(column_id);
    }
    save_state(&app_state);
    Ok(())
}

#[tauri::command]
fn hidden_columns(state: State<AppStateWrapper>) -> Vec<String> {
    let app_state = state.0.lock().unwrap();
    app_state.hidden_columns.clone()
}

#[tauri::command]
fn show_only_my_items(state: State<AppStateWrapper>) -> bool {
    let app_state = state.0.lock().unwrap();
    app_state.show_only_my_items
}

#[tauri::command]
async fn current_user() -> Result<String, String> {
    log::info!("Fetching current user from GitHub");
    let output = std::process::Command::new("gh")
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


    // Create View menu
    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&refresh)
        .separator()
        .item(&toggle_my_items)
        .item(&toggle_expanded)
        .build()?;

    // Create Project menu
    let select_project = MenuItemBuilder::new("Select Project...")
        .id("select-project")
        .accelerator("CmdOrCtrl+P")
        .build(app)?;

    let project_menu = SubmenuBuilder::new(app, "Project")
        .item(&select_project)
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
            "select-project" => {
                log::info!("Select project menu item selected");
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.emit("menu-select-project", ());
                }
            }
            id if id.starts_with("column-") => {
                log::info!("Column visibility toggle: {}", id);
                let column_id = id.strip_prefix("column-").unwrap_or("");
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
            select_project,
            current_project,
            toggle_my_items,
            toggle_column_visibility,
            hidden_columns,
            show_only_my_items,
            current_user,
        ])
        .setup(|app| {
            let _app_handle = app.handle().clone();

            // Build the application menu
            setup_app_menu(app)?;

            let window = app.get_webview_window("main").unwrap();

            window.on_window_event(|event| {
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