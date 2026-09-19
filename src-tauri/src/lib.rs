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
            runtime::commands::save_model_config,
            runtime::commands::save_template_config,
            runtime::commands::load_config,
            runtime::commands::list_models,
            runtime::commands::scan_assets,
            runtime::commands::preview_rename,
            runtime::commands::collect_targets,
            runtime::commands::ai_fill_targets,
            runtime::commands::rename_items,
            runtime::commands::undo_rename,
            runtime::commands::redo_rename,
            runtime::commands::history_status,
            runtime::commands::get_version,
            runtime::commands::list_logs,
            runtime::commands::open_log_dir,
            runtime::commands::pick_dir,
            runtime::commands::pick_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
