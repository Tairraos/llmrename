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

### 1. 加载 / 保存配置
`UI → invoke load_config → runtime::load_config → config::load`（同步内存 AppState）
`UI 表单 → invoke save_model_config / save_template_config → config::save_model / save_template`（模型与模板分开保存，互不覆盖）

### 2. 收集文件（拖放 / 选择）
`UI（tauri://drag-drop / pick_files / pick_dir）→ invoke collect_targets → repo::targets::collect_targets`
目录递归、扩展名过滤、去重、按文件名自然排序；`AssetEntry` 含 `path / filename / relative_path`。

### 3. AI 填充目标名（逐文件流式）
`UI → invoke ai_fill_targets(items, pattern, user_hint) → service::vision::extract_abs（每张图一次调用）`
- Runtime 逐文件发事件：`vision-status`（connecting/extracting/parsing/item-done/stopped）+ `vision-stream`（SSE 增量回显）
- item-done 携带渲染好的 target，前端**立即更新对应列表行**（不等整体返回）
- 提取结果解析（剥围栏/寒暄、单引号容错）见 `vision-model-contract.md`
- `stop_ai_fill` 置原子标志，在文件边界生效（暂停解析）

### 4. 执行重命名
`UI → invoke rename_items(items) → service::namer::rename_explicit → repo::renamer::rename_path`
- 逐条校验目标名（空/路径分隔符/NUL/扩展名白名单），目标已存在时自动追加序号（`名称 1.ext` 起）
- 每条结果（ok/failed/skipped）写 `repo::logbook::append`（JSONL）并记入内存 HistoryStore
- 返回 `(Vec<RenameOutcome>, HistoryStatus)`；行级 undo/redo 在**前端**完成（每行 10 条版本链），后端 `undo_rename/redo_rename` 保留为 API（当前 UI 未调用）

### 5. 查看日志
`UI invoke list_logs / open_log_dir` → `repo::logbook`。

## 配置存储

- 路径：`app.path().app_data_dir()`（macOS: `~/Library/Application Support/com.llmrename.app`，Linux: `~/.local/share/com.llmrename.app`）
- `config.json`：模型 + 模板 + 选项
- `rename_log.jsonl`：追加式重命名日志（每条一个 JSON 对象）
- 仓库内不含任何运行期数据。

## UI 状态

前端无框架，全局对象 `window.App` 持有状态：`config`、`targets`（行数组：path/filename/relDir/name/ext/history/selected）、`logs`、`running`、`historyApplied`、`fill`（解析状态机 + okPaths）、`regex`（正则预览查找/替换）。UI 更新走 `renderTargets()/renderLogs()` 重建 DOM；行级更新（AI 逐个返回）走 `updateTargetRowDom` 单行替换，避免整表重渲染打断编辑。

## 日志与错误

- 错误统一为 `String` 或带上下文的 `AppError`（其 Display 包含路径、模板、模型摘要与修复提示），在边界转为 `String` 传给 UI。
- 应用日志（调试用）写控制台，不落盘；用户可见的历史记录靠 JSONL 日志。