# 架构文档

llmrename 是一个 Tauri 2 桌面应用：纯 HTML/CSS/JS 前端运行在系统 WebView，Rust 后端通过 Tauri IPC（`invoke`）暴露命令。后端按五层组织，依赖单向向下。

## 分层总览

```
┌────────────────────────────────────────────────┐
│ UI (src/)  纯 HTML+CSS+JS，无框架、无构建依赖    │
│   index.html / styles.css / main.js / api.js    │
└──────────────┬─────────────────────────────────┘
               │ Tauri IPC (invoke / emit)
┌──────────────▼─────────────────────────────────┐
│ Runtime (src-tauri/src/runtime/)                 │
│   Tauri command 层：参数解析、AppState（Mutex<AppConfig>）│
└──────────────┬─────────────────────────────────┘
┌──────────────▼─────────────────────────────────┐
│ Service (service/)                               │
│   renderer：模板渲染与要素提取请求构造           │
│   namer：防冲突重命名编排                       │
└──────────────┬─────────────────────────────────┘
┌──────────────▼─────────────────────────────────┐
│ Config (config/)                                 │
│   AppConfig 加载/保存/校验（JSON 文件）           │
│   ModelConfig / TemplateConfig / RenameOptions   │
└──────────────┬─────────────────────────────────┘
┌──────────────▼─────────────────────────────────┐
│ Repo (repo/)                                     │
│   scanner：目录扫描、扩展名过滤、自然排序        │
│   renamer：重命名执行                           │
│   logbook：JSONL 追加日志                        │
├─────────────────────────────────────────────────┤
│ Types (types/)  纯数据模型（serde）              │
│   asset / template / model / log / 错误          │
└─────────────────────────────────────────────────┘
```

## 依赖规则（铁律）

- 依赖只允许 `Types ← Config ← Repo ← Service ← Runtime` 单向向下。
- `Types` 只依赖 `serde`；`Config` 依赖 `Types` + `serde`；`Repo` 依赖 `Types`；`Service` 依赖 `Types/Config/Repo`；`Runtime` 依赖全部下层。
- 任何模块禁止 `use crate::runtime::*`。
- 跨层数据必须经 `Types` 中的类型传递；在边界（HTTP 响应、磁盘文件、IPC 入参）解析并校验，内部不重复校验。

## 关键数据流

### 1. 保存配置
`UI 表单 → invoke save_config → runtime::save_config → config::AppConfig::save`

### 2. 扫描资产库
`UI → invoke scan_assets → runtime::scan_assets → repo::scanner::scan(path, options) → Vec<AssetEntry>`

### 3. 预览重命名（不发模型请求）
`UI → invoke preview_rename → runtime::preview_rename → service::namer::preview(entries, RenderPlan) → Vec<RenamePreview>`
模板字段按**已存在格式名猜测**（如 `{date}` 视为 YYYY-MM-DD 类别），不保证真实；真实提取在 execute 中由模型完成。

### 4. 执行重命名
`UI → invoke execute_rename → runtime::execute_rename → service::namer::execute(paths, template, options, ModelConfig)`
逐批调用视觉模型（见 `vision-model-contract.md`）→ 得到 `ExtractedFields` → 渲染 base → 防冲突生成最终名 → 校验后缀/黑名单 → `repo::renamer::rename` → `repo::logbook::append`（成功与失败逐条落盘 JSONL）。

### 5. 查看日志
`UI invoke list_logs / open_log_dir` → `repo::logbook`。

## 配置存储

- 路径：`app.path().app_data_dir()`（macOS: `~/Library/Application Support/com.llmrename.app`，Linux: `~/.local/share/com.llmrename.app`）
- `config.json`：模型 + 模板 + 选项
- `rename_log.jsonl`：追加式重命名日志（每条一个 JSON 对象）
- 仓库内不含任何运行期数据。

## UI 状态

前端无框架，全局对象 `window.App` 持有状态：config、assets、previews、logs、selectedPaths、isRunning。UI 更新走 `renderXxx()` 纯函数重建 DOM 片段。

## 日志与错误

- 错误统一为 `String` 或带上下文的 `AppError`（其 Display 包含路径、模板、模型摘要与修复提示），在边界转为 `String` 传给 UI。
- 应用日志（调试用）写控制台，不落盘；用户可见的历史记录靠 JSONL 日志。