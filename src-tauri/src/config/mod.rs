//! 配置层：AppConfig 的加载 / 保存 / 校验。
//! 文件位于应用数据目录（仓库外），由 runtime 层传入路径。

use crate::types::{AppConfig, AppError, Result};

pub const CONFIG_FILE: &str = "config.json";

/// 加载配置；文件不存在时返回默认配置（不报错）。
pub fn load(dir: &std::path::Path) -> Result<AppConfig> {
    let path = dir.join(CONFIG_FILE);
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AppError::config(format!("读取配置文件失败：{e}（{}）", path.display())))?;
    serde_json::from_str(&raw)
        .map_err(|e| AppError::config(format!("配置文件格式错误：{e}（{}）", path.display())))
}

/// 保存配置：先校验，再原子写入（先写临时文件再改名）。
pub fn save(dir: &std::path::Path, cfg: &AppConfig) -> Result<()> {
    validate(cfg)?;
    std::fs::create_dir_all(dir)
        .map_err(|e| AppError::config(format!("无法创建配置目录：{e}（{}）", dir.display())))?;
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::internal(format!("序列化配置失败：{e}")))?;
    let path = dir.join(CONFIG_FILE);
    write_atomic(&path, json.as_bytes())
}

/// 应用数据子目录（config.json + rename_log.jsonl 所在）。
pub fn new_dir(base: &std::path::Path) -> Result<std::path::PathBuf> {
    let p = base.join("llmrename");
    std::fs::create_dir_all(&p)
        .map_err(|e| AppError::config(format!("无法创建数据目录：{e}（{}）", p.display())))?;
    Ok(p)
}

fn validate(cfg: &AppConfig) -> Result<()> {
    if cfg.model.base_url.trim().is_empty() {
        return Err(AppError::config(
            "base_url 不能为空（如 https://api.openai.com/v1）",
        ));
    }
    if cfg.model.model.trim().is_empty() {
        return Err(AppError::config("模型名不能为空（如 gpt-4o）"));
    }
    if cfg.model.timeout_secs < 1 || cfg.model.timeout_secs > 3600 {
        return Err(AppError::config("超时秒数需在 1–3600 之间"));
    }
    if cfg.template.pattern.trim().is_empty() {
        return Err(AppError::template("模板不能为空，使用 {字段} 占位符"));
    }
    Ok(())
}

fn write_atomic(path: &std::path::Path, content: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)
        .map_err(|e| AppError::config(format!("写入配置失败：{e}（{}）", tmp.display())))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| AppError::config(format!("替换配置文件失败：{e}（{}）", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = AppConfig::default();
        cfg.model.model = "gpt-4o-mini".into();
        cfg.template.pattern = "{date}_{scene}".into();
        save(dir.path(), &cfg).unwrap();
        let back = load(dir.path()).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn save_rejects_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = AppConfig::default();
        cfg.model.model = "".into();
        assert!(save(dir.path(), &cfg).is_err());
    }
}
