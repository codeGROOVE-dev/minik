mod github;
mod logging;

use github::{GitHubClient, Organization, Project, ProjectData};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State, AppHandle, WindowEvent, Emitter};
use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
use tauri::menu::{MenuBuilder, MenuItemBuilder};

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
async fn project_data(project_id: String, state: State<'_, AppStateWrapper>) -> Result<ProjectData, String> {
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
            select_project,
            current_project,
            toggle_my_items,
            toggle_column_visibility,
            hidden_columns,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Create the tray menu
            let quit = MenuItemBuilder::new("Quit")
                .id("quit")
                .build(app)?;

            let _separator = MenuItemBuilder::new("---")
                .id("separator")
                .enabled(false)
                .build(app)?;

            let refresh = MenuItemBuilder::new("Refresh")
                .id("refresh")
                .build(app)?;

            let show_window = MenuItemBuilder::new("Show Window")
                .id("show")
                .build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_window)
                .item(&refresh)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .tooltip("Minik - GitHub Kanban")
                .menu(&menu)
                .on_menu_event(move |app_handle, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            log::info!("Quit menu item selected");
                            app_handle.exit(0);
                        }
                        "refresh" => {
                            log::info!("Refresh menu item selected");
                            // Emit event to frontend to refresh
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.emit("refresh-project", serde_json::json!({}));
                            }
                        }
                        "show" => {
                            log::info!("Show window menu item selected");
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {
                            log::debug!("Unknown menu item: {:?}", event.id);
                        }
                    }
                })
                .on_tray_icon_event(move |_tray, event| {
                    match event {
                        TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => {
                            log::debug!("Left click on tray icon");
                            // Show the window on tray icon click
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {
                            log::trace!("Tray icon event: {:?}", event);
                        }
                    }
                })
                .build(app)?;

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