# AGENTS.md

本文件是仓库的**内容目录**（map），不是百科全书。智能体接到任务后先读本文件与 `docs/*.md` 的入口，按指向去读对应细节，不要全文阅读。

## 项目是什么

`llmrename` 是一个桌面工具：用户指定一个资产库目录，配置一个**重命名模板**（如 `{人物}-{场景}-{动作}-{日夜}`，字段名支持中文），工具用**视觉模型**逐张解析图片并提取模板所需的要素，然后按要素批量重命名文件，全程写入重命名日志。

- 打包：Tauri 2（Rust 后端 + 系统 WebView）
- UI：纯 HTML + CSS + JS（无前端框架、无构建产物依赖），放 `src/`
- 配置与日志：写在应用数据目录（`app.path().app_data_dir()`，macOS 为 `~/Library/Application Support/<identifier>/`），不写进仓库
- 默认模型：OpenAI `gpt-4o`，API 兼容（可通过 base_url 对接兼容 OpenAI 协议的服务，如 vLLM/OneAPI/GPT4Free），当前仅 HTTP JSON 调用

## 知识地图

| 我想知道 | 去看 |
|---|---|
| 架构分层与依赖方向（铁律） | `docs/architecture.md` |
| 执行计划（活跃 / 已完成 / 技术债） | `docs/plan/execution-plan.md` |
| 产品规格 / 验收标准（工作区） | `docs/design/product-spec.md` |
| 对外部模型的假设与契约 | `docs/design/vision-model-contract.md` |
| 命令、检查、本地开发 | `docs/development.md` |
| 外部参考（llms.txt 风格） | `docs/external/` |
| 代码风格与提交规范 | 见下节「工程铁律」 |

## 工程铁律（不可协商）

以下规则适用于本仓库的一切智能体与人工变更。

### 1. 提交纪律

1. **每轮对话过程中和对话结束时，都必须执行 `git commit` 提交**，Commit message 的 body 要**详细说明**本次变更（做了什么、为什么、影响面）。
2. **提交颗粒度 = 单一功能变更**：一次提交只做一件事（一个功能 / 一次修复 / 一次重构 / 一次文档更新），不要把多件事塞进一个提交。进行中的半成品按进度分次提交。
3. **普通提交不受测试和门禁限制**（可带未通过测试的中间态，只要下次提交修好）。
4. **会话结束前的最后一次提交必须通过全部测试和门禁**（`pnpm check` / CI 全绿），并在 Commit message 里注明 `[已测试]` 或 `[未测试]`。
5. **Commit message 必须使用 Conventional Commits 前缀**：`fix:`、`feat:`、`chore:`、`init:`、`docs:`、`refactor:`、`test:` 等。格式：
   ```
   <type>: <一句话概括>

   <详细说明：动机、方案、影响。>
   ```
   type 为 `init:` 时表示仓库初始化 / 脚手架类提交。

### 2. 架构边界（依赖方向铁律）

依赖只能**单向向下**，上层可依赖下层，下层绝不可依赖上层。每层是 Rust crate 内的一个 module，module 间按层引用。加代码前先想清楚它属于哪一层：

```
Types → Config → Repo → Service → Runtime
```

| 层 | 内容 | 可依赖 |
|---|---|---|
| `Types` | 纯数据模型（模板字段、资产、重命名请求/结果、错误）与解析/序列化 | 无（仅 serde） |
| `Config` | 配置的加载与校验（模型、模板、路径、日志） | Types |
| `Repo` | 对文件系统的访问：目录扫描、扩展名过滤、排序、重命名、JSONL 日志追加 | Types |
| `Service` | 业务编排：调用视觉模型、模板渲染、防冲突命名、日志记录 | Types, Config, Repo |
| `Runtime` | Tauri command 层：暴露给前端的命令、状态管理 | Types, Config, Repo, Service |

横切关注点（错误处理、日志、序列化）仅通过显式接口进入，不跨层渗透。任何模块不得使用 `crate::runtime::*`。

### 3. 知识必须仓库本地化

- 一切智能体运行时无法访问的内容（对话历史、人脑知识、外部文档）都必须维护在仓库内：写进 `docs/` 或在代码注释中固化。
- 本文件（AGENTS.md）保持简短（~100 行级别），只做地图；细节放下层文档。
- 新知识落地时，先判断属于哪份文档；没有对应文档就新建并更新本地图。

### 4. 数据形状在边界解析

- 从 HTTP / JSON / 文件系统读入的数据，一律在**边界**（各层入口）解析为强类型，内部只传递类型化数据，"parse, don't validate"。
- 不做"YOLO 式"探测：不靠猜测依赖库行为或数据格式，用类型化 SDK / 明确的 schema。

### 5. 测试与门禁

- 会话结束前的最后提交必须 `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` 全绿，前端 `node --check` 通过（见 `scripts/check.sh`，本地从 `pnpm check` 调用）。
- 本仓库的合并门尽量少而稳：`fmt + clippy + test` 是唯一硬门禁，偶发失败重跑而非改门。
- 新增逻辑（尤其 Repo 层重命名、Service 层模板渲染与防冲突）必须带单元测试。

### 6. 品味不变量

- 结构化错误：错误必须携带足够的上下文（路径、模板、模型响应摘要），供用户直接判断；错误信息里给出可执行的修复提示。
- 命名规范：Rust 用 snake_case，前端 JS 函数用 camelCase，文件/目录用 kebab-case。
- 文件大小限制：单个源文件尽量 < 300 行；超限先考虑拆模块。
- 优先共享实用工具（本仓库 `types/`、`scripts/` 内的共享代码），不复制粘贴辅助函数。
- 偏好"枯燥"技术：可组合、API 稳定、文档清晰的技术；遇到问题先读官方文档，不绕开不透明行为。

### 6.5 包管理（pnpm）

- **本仓库统一使用 pnpm 作为唯一包管理器**：安装依赖用 `pnpm install`，运行脚本用 `pnpm <script>`（如 `pnpm check`、`pnpm tauri dev`）。
- 不使用 npm / yarn / bun；新增脚本与文档一律写 pnpm 命令。
- 项目必须提交 `pnpm-lock.yaml` 锁定依赖；`package.json` 中 `packageManager` 字段固定 pnpm 版本。
- CI 与本地脚本（`scripts/check.sh`）中的包管理相关命令统一为 pnpm。

### 6.6 构建与发布

1. **构建目标目录是项目根的 `target/`**（`src-tauri/.cargo/config.toml` 设定 `target-dir = "../target"`）。
2. **每次 build 都必须提升版本号**：用 `./scripts/release.sh`（或 `python3 scripts/_bump_version.py`）自动 bump patch 版本，同步 `tauri.conf.json` / `Cargo.toml` / `package.json`。
3. **默认只构建 `.app`**；需要 `.dmg` 时显式传 `--dmg`（`./scripts/release.sh --dmg`）。
4. **build 完成后保留编译缓存，只清理打包产物**：`target/release`、`target/debug` 等编译缓存一律不删，允许以后增量编译（冷全量约 1 分钟 vs 缓存增量约 1 秒）；需要释放空间时由用户手动清理。仅删除可再生成的产物：`dist/` 与 `target/release/bundle/`；遗留的 `src-tauri/target` 仍删除。`.app` 以版本号命名移到 `target/` 根（如 `LLM Rename_0.1.5.app`，只保留最新一份）；`--dmg` 时附加 `.dmg`。`release/<version>/` 留档 `.app` / `.dmg` 与 `VERSION` 文件。
5. **每次会话结束必须产出新版 .app**：会话收尾执行 `./scripts/release.sh`（自动 bump 版本 → 构建 → 留档 → 清理打包产物），确保每轮会话都有一个带新版本号的可运行 `.app` 交付物。
6. **版本号显示在窗口标题**：前端运行时请求 `get_version` 注入 title（`LLM Rename v0.x.y`）。
7. **build 产物不进 repo**：`target/`、`dist/`、`release/` 均在 `.gitignore`。
8. **提升了版本号的那次 git commit，message 必须写明版本号**（如 `feat: 升级到 0.1.1`），便于追溯发布物与源码对应。

### 7. 熵管理

- 不允许"AI 残渣"：不复制仓库中已存在但不理想的反模式。
- 定期检查 `docs/plan/execution-plan.md` 的技术债清单；新发现的偏差记入其中，以小步提交持续偿还，不累积。

## 快速开始的下一步

1. 读 `docs/development.md` —— 本地运行、构建、检查命令。
2. 读 `docs/architecture.md` —— 分层与数据流。
3. 读 `docs/plan/execution-plan.md` —— 当前活跃计划与技术债。
4. 动工前在自己的计划里标注所属层次，动工后按铁律拆分提交。