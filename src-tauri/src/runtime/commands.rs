//! Runtime 层：Tauri command 暴露给前端的界面。
//! 只做「参数解析 → 调用下层 → 结果序列化」，不承载业务规则。

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};

use crate::config;
use crate::repo;
use crate::service;
use crate::types::{AppConfig, AppError, AssetEntry, HistoryStatus, LogEntry, RenameItem, Result};
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
pub async fn preview_rename(_app: AppHandle, dir: String, pattern: String) -> Result<Vec<String>> {
    let entries = repo::scanner::scan(
        &PathBuf::from(&dir),
        &crate::types::RenameOptions::default().extensions,
    )?;
    let fields = crate::types::extract_fields(&pattern);
    let guesses = service::namer::preview_guesses(&fields);
    Ok(entries
        .iter()
        .map(|a| {
            let base = service::renderer::render(&pattern, &guesses);
            format!("{base}.{}", a.ext())
        })
        .collect())
}

/// 拖放/选择的任意路径归一化为文件列表（目录递归、文件直收）。
#[tauri::command]
pub async fn collect_targets(
    _app: AppHandle,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<AssetEntry>> {
    let options = state.config.lock().unwrap().options.clone();
    repo::targets::collect_targets(&paths, &options.extensions, 8)
}

/// 视觉模型填充目标名：对每张图提取字段 → 渲染模板 → 返回「路径 → 目标名」。
#[tauri::command]
pub async fn ai_fill_targets(
    _app: AppHandle,
    items: Vec<RenameItem>,
    pattern: String,
    state: State<'_, AppState>,
) -> Result<Vec<RenameItem>> {
    let model = state.config.lock().unwrap().model.clone();
    let mut out = Vec::with_capacity(items.len());
    for it in &items {
        let path = PathBuf::from(&it.path);
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        match service::vision::extract_abs(&path, &pattern, &model).await {
            Ok(fields_json) => {
                let base = service::renderer::render_plan(&pattern, &fields_json);
                let ext = filename
                    .rsplit_once('.')
                    .map(|(_, e)| e.to_lowercase())
                    .unwrap_or_default();
                out.push(RenameItem {
                    path: it.path.clone(),
                    target: format!("{base}.{ext}"),
                });
            }
            Err(_) => {
                out.push(RenameItem {
                    path: it.path.clone(),
                    target: it.target.clone(),
                });
            }
        }
    }
    Ok(out)
}

/// 按显式目标名批量重命名。
///
/// 返回 (结果列表, 历史状态)；失败/跳过逐项在 outcome 里，不整体失败。
#[tauri::command]
pub async fn rename_items(
    app: AppHandle,
    items: Vec<RenameItem>,
    state: State<'_, AppState>,
) -> Result<(Vec<service::namer::RenameOutcome>, HistoryStatus)> {
    let log_dir = data_dir(&app)?;
    let options = state.config.lock().unwrap().options.clone();

    let mut outcomes = service::namer::rename_explicit(&items, &options);

    // 写日志 + 记历史（仅成功的）
    let mut history = state.history.lock().unwrap();
    for (it, oc) in items.iter().zip(outcomes.iter_mut()) {
        match oc.status.as_str() {
            "ok" => {
                let entry = LogEntry::ok(
                    Path::new(&it.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    oc.to
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    None,
                    None,
                );
                let _ = repo::logbook::append(&log_dir, &entry);
                history.record(&oc.from, &oc.to);
            }
            "skipped" => {
                let entry = LogEntry::skipped(
                    Path::new(&it.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    oc.error.clone().unwrap_or_default(),
                );
                let _ = repo::logbook::append(&log_dir, &entry);
            }
            _ => {
                let entry = LogEntry::failed(
                    Path::new(&it.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    oc.error.clone().unwrap_or_default(),
                );
                let _ = repo::logbook::append(&log_dir, &entry);
            }
        }
    }
    let status = history.status();
    Ok((outcomes, status))
}

/// 撤销最近一次重命名。
#[tauri::command]
pub async fn undo_rename(_app: AppHandle, state: State<'_, AppState>) -> Result<HistoryStatus> {
    let mut history = state.history.lock().unwrap();
    let Some((to, from)) = history.undo() else {
        return Ok(history.status());
    };
    // rename(to → from)：撤销
    let _ = repo::renamer::rename_path(&to, &from);
    Ok(history.status())
}

/// 重做最近一次撤销。
#[tauri::command]
pub async fn redo_rename(_app: AppHandle, state: State<'_, AppState>) -> Result<HistoryStatus> {
    let mut history = state.history.lock().unwrap();
    let Some((from, to)) = history.redo() else {
        return Ok(history.status());
    };
    let _ = repo::renamer::rename_path(&from, &to);
    Ok(history.status())
}

/// 当前历史状态（按钮可用态/摘要）。
#[tauri::command]
pub async fn history_status(state: State<'_, AppState>) -> Result<HistoryStatus> {
    Ok(state.history.lock().unwrap().status())
}

/// 应用版本号（tauri.conf.json version，启动时注入 title）。
#[tauri::command]
pub fn get_version(app: AppHandle) -> Result<String> {
    Ok(app.package_info().version.to_string())
}

/// 读取最近日志（最新在前）。
#[tauri::command]
pub async fn list_logs(app: AppHandle, limit: Option<usize>) -> Result<Vec<LogEntry>> {
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

/// 原生文件（多选）对话框。返回选中的文件路径列表；取消时返回空。
#[tauri::command]
pub async fn pick_files() -> Result<Vec<String>> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("选择图片文件")
        .add_filter(
            "图片",
            &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff"],
        )
        .pick_files()
        .await;
    Ok(picked
        .map(|files| {
            files
                .into_iter()
                .map(|f| f.path().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default())
}
