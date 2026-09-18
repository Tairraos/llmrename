//! 后端库入口：模块声明与 Tauri 应用装配。

pub mod config;
pub mod repo;
pub mod runtime;
pub mod service;
pub mod types;

pub use runtime::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            runtime::commands::save_config,
            runtime::commands::load_config,
            runtime::commands::scan_assets,
            runtime::commands::preview_rename,
            runtime::commands::execute_rename,
            runtime::commands::list_logs,
            runtime::commands::open_log_dir,
            runtime::commands::pick_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
