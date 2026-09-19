//! 配置层：AppConfig 的加载 / 保存 / 校验。
//! 文件位于应用数据目录（仓库外），由 runtime 层传入路径。

use crate::types::{AppConfig, AppError, ModelConfig, Result};

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

/// 保存完整配置：先校验，再原子写入（先写临时文件再改名）。
pub fn save(dir: &std::path::Path, cfg: &AppConfig) -> Result<()> {
    validate(cfg)?;
    write_cfg(dir, cfg)
}

/// 只保存模型配置：合并进现有配置（模板与选项保持不变），仅校验模型部分。
pub fn save_model(dir: &std::path::Path, model: &ModelConfig) -> Result<()> {
    validate_model(model)?;
    let mut cfg = load(dir)?;
    cfg.model = model.clone();
    write_cfg(dir, &cfg)
}

/// 只保存模板：合并进现有配置，仅校验模板部分。
pub fn save_template(dir: &std::path::Path, pattern: &str) -> Result<()> {
    validate_template(pattern)?;
    let mut cfg = load(dir)?;
    cfg.template.pattern = pattern.trim().to_string();
    write_cfg(dir, &cfg)
}

fn write_cfg(dir: &std::path::Path, cfg: &AppConfig) -> Result<()> {
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
    validate_model(&cfg.model)?;
    validate_template(&cfg.template.pattern)
}

/// 模型部分校验（与模板解耦：保存模型不要求模板非空）。
fn validate_model(model: &ModelConfig) -> Result<()> {
    if model.base_url.trim().is_empty() {
        return Err(AppError::config(
            "base_url 不能为空（如 https://api.openai.com/v1）",
        ));
    }
    if model.model.trim().is_empty() {
        return Err(AppError::config("模型名不能为空（如 gpt-4o）"));
    }
    if model.timeout_secs < 1 || model.timeout_secs > 3600 {
        return Err(AppError::config("超时秒数需在 1–3600 之间"));
    }
    Ok(())
}

/// 模板部分校验。
fn validate_template(pattern: &str) -> Result<()> {
    if pattern.trim().is_empty() {
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

    #[test]
    fn save_model_keeps_template_even_if_template_empty() {
        // 回归：模型设置里保存不应被模板校验拦住（模板可为空/未配置）
        let dir = tempfile::tempdir().unwrap();
        let model = ModelConfig {
            model: "gpt-4o-mini".into(),
            ..Default::default()
        };
        save_model(dir.path(), &model).unwrap();
        let back = load(dir.path()).unwrap();
        assert_eq!(back.model, model);
        assert_eq!(back.template, crate::types::TemplateConfig::default());
    }

    #[test]
    fn save_model_preserves_existing_template_and_options() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = AppConfig {
            template: crate::types::TemplateConfig {
                pattern: "{人物}_{场景}".into(),
            },
            options: crate::types::RenameOptions {
                blacklist: vec!["bad".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        save(dir.path(), &cfg).unwrap();

        let model = ModelConfig {
            api_key: "sk-x".into(),
            ..Default::default()
        };
        save_model(dir.path(), &model).unwrap();

        let back = load(dir.path()).unwrap();
        assert_eq!(back.model.api_key, "sk-x");
        assert_eq!(back.template.pattern, "{人物}_{场景}");
        assert_eq!(back.options.blacklist, vec!["bad".to_string()]);
    }

    #[test]
    fn save_model_still_validates_model() {
        let dir = tempfile::tempdir().unwrap();
        let model = ModelConfig {
            base_url: "  ".into(),
            ..Default::default()
        };
        assert!(save_model(dir.path(), &model).is_err());
    }

    #[test]
    fn save_template_updates_only_pattern() {
        let dir = tempfile::tempdir().unwrap();
        save_template(dir.path(), "  {人物}_{日夜}  ").unwrap();
        let back = load(dir.path()).unwrap();
        assert_eq!(back.template.pattern, "{人物}_{日夜}");
        assert_eq!(back.model, ModelConfig::default());
        // 空模板仍被拒绝
        assert!(save_template(dir.path(), "   ").is_err());
    }
}
