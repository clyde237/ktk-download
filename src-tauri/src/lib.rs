pub mod commands;
pub mod downloader;
pub mod error;
pub mod extractor;
pub mod models;
pub mod validator;

use commands::{download_video, inspect_url, validate_url};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            inspect_url,
            download_video,
            validate_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
