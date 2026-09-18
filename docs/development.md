# 开发指南

## 前置依赖

- [Rust](https://rustup.rs)（1.77+，本仓库按 1.77 MSRV 编写）
- [Node.js](https://nodejs.org) 18+（仅用于前端语法检查与本地脚本，无运行时依赖）
- Tauri 2 系统依赖：macOS 需要 Xcode Command Line Tools；见 [Tauri 官方文档](https://v2.tauri.app/start/prerequisites/)

## 常用命令

```bash
# 安装前端 devDependencies（首次）
pnpm install

# 本地开发运行（vite dev server + tauri dev）
pnpm tauri dev

# 构建发布（自动 bump 版本，默认只出 .app；加 --dmg 出 dmg；target/ 只保留 .app）
./scripts/release.sh
./scripts/release.sh --dmg

# 全量检查（门禁）：fmt + clippy + test + 前端语法检查
pnpm check            # 等价 npx tauri build 前的本地门禁
./scripts/check.sh       # 底层脚本，所有检查的单一事实来源

# 仅向后端
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# 仅向前端（语法级检查，作用于 src/*.js）
node --check src/main.js
```

`pnpm check` 只是 `scripts/check.sh` 的门面，CI 直接跑脚本，保证本地与 CI 一致。

## 目录结构

```
├── AGENTS.md               # 智能体地图 + 铁律（先读）
├── docs/                   # 记录系统
│   ├── architecture.md     # 分层与数据流
│   ├── development.md      # 本文件
│   ├── plan/execution-plan.md
│   ├── design/product-spec.md
│   ├── design/vision-model-contract.md
│   └── external/           # 外部参考（llms.txt）
├── scripts/
│   ├── check.sh            # 门禁脚本
│   └── new-commit.sh       # 铁律提交辅助
├── src/                    # 前端：纯 HTML/CSS/JS（无框架）
│   ├── index.html
│   ├── styles.css
│   └── main.js
├── src-tauri/              # Tauri 2 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs / lib.rs
│       ├── types/ config/ repo/ service/ runtime/
│       └── tests/          # 测试
├── package.json            # devDependencies: @tauri-apps/cli 仅用于启动/构建
└── .github/workflows/ci.yml
```

## 写代码时的检查清单

1. 代码落在正确的层（看 `docs/architecture.md` 的分层表），不跨层引用。
2. 文件系统 / HTTP / IPC 数据在边界解析为 `types::*`。
3. Repo 层重命名与 Service 层渲染逻辑带单元测试。
4. 错误信息带上下文和修复提示。
5. 一次提交 = 一个功能变更，message 带前缀与详细 body（见 AGENTS.md 铁律 1）。

## 测试

- `cargo test`：后端单元测试（模板渲染、防冲突、配置往返、日志追加、扫描排序）。
- 前端暂无自动化测试（纯 DOM 渲染，语法检查兜底）；UI 行为变更需在 `execution-plan.md` 的记录表登记人工验证。

## 门禁

本地与 CI 同脚本 `./scripts/check.sh`，含：

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`
4. `node --check` 遍历 `src/*.js`

所有输出 0 才通过。会话结束前的最后一次提交必须通过（AGENTS.md 铁律 1.4）。