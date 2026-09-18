//! Runtime 层：Tauri command 暴露给前端的界面。
//! 只做「参数解析 → 调用下层 → 结果序列化」，不承载业务规则。

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::config;
use crate::repo;
use crate::service;
use crate::types::{
    AppConfig, AppError, AssetEntry, ModelConfig, RenameOptions, RenameResult, Result,
};
use crate::AppState;

/// 应用数据目录（config.json / rename_log.jsonl 所在）。
fn data_dir(app: &AppHandle) -> Result<PathBuf> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("无法获取应用数据目录：{e}")))?;
    config::new_dir(&base)
}

/// 保存配置。
#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<()> {
    let dir = data_dir(&app)?;
    config::save(&dir, &config)
}

/// 读取配置。
#[tauri::command]
pub fn load_config(app: AppHandle) -> Result<AppConfig> {
    let dir = data_dir(&app)?;
    config::load(&dir)
}

/// 扫描资产目录（不递归）。
#[tauri::command]
pub async fn scan_assets(
    _app: AppHandle,
    dir: String,
    state: State<'_, AppState>,
) -> Result<Vec<AssetEntry>> {
    let dir = PathBuf::from(&dir);
    let options = state.config.lock().unwrap().options.clone();
    repo::scanner::scan(&dir, &options.extensions)
}

/// 预览：不清模型，仅按字段名猜测示例生成新名。
#[tauri::command]
pub async fn preview_rename(app: AppHandle, dir: String, pattern: String) -> Result<Vec<String>> {
    let _ = app;
    let pattern_ok = crate::types::extract_fields(&pattern);
    let entries = repo::scanner::scan(
        &PathBuf::from(&dir),
        &crate::types::RenameOptions::default().extensions,
    )?;
    let guesses = service::namer::preview_guesses(&pattern_ok);
    Ok(entries
        .iter()
        .map(|a| {
            let base = service::renderer::render(&pattern, &guesses);
            format!("{base}.{}", a.ext())
        })
        .collect())
}

/// 执行重命名。
#[tauri::command]
pub async fn execute_rename(
    app: AppHandle,
    dir: String,
    pattern: String,
    paths: Vec<String>,
    model: Option<ModelConfig>,
    options: Option<RenameOptions>,
) -> Result<RenameResult> {
    let dir = PathBuf::from(&dir);
    let log_dir = data_dir(&app)?;
    let model = model.unwrap_or_default();
    let options = options.unwrap_or_default();
    service::namer::execute(&dir, &pattern, &paths, &model, &options, &log_dir).await
}

/// 读取最近日志（最新在前）。
#[tauri::command]
pub async fn list_logs(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<Vec<crate::types::LogEntry>> {
    let dir = data_dir(&app)?;
    repo::logbook::recent(&dir, limit.unwrap_or(200))
}

/// 打开日志目录（系统文件管理器）。
#[tauri::command]
pub async fn open_log_dir(app: AppHandle) -> Result<()> {
    let dir = data_dir(&app)?;
    open::that(&dir).map_err(|e| AppError::fs(format!("无法打开日志目录 {}：{e}", dir.display())))
}

/// 原生目录选择对话框。返回选中的目录绝对路径；取消时返回 None。
#[tauri::command]
pub async fn pick_dir() -> Result<Option<String>> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("选择资产库目录")
        .pick_folder()
        .await;
    Ok(picked.map(|p| p.path().to_string_lossy().into_owned()))
}
