pub mod commands;
pub mod downloader;
pub mod error;
pub mod extractor;
pub mod models;
pub mod quality;
pub mod validator;

use commands::{cancel_download, download_video, inspect_url, validate_url};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(downloader::DownloadRegistry::default())
        .invoke_handler(tauri::generate_handler![
            greet,
            inspect_url,
            download_video,
            validate_url,
            cancel_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
