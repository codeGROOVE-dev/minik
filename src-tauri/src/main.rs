// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(e) = minik_app_lib::run() {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
