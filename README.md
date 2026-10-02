# LLM Rename

用视觉模型按模板批量重命名图片的 macOS 桌面工具（Tauri 2 + 纯 HTML/CSS/JS）。

把图片或文件夹拖进窗口 → 配置一个重命名模板（如 `{人物}-{场景}-{动作}-{日夜}`）→ 用视觉模型逐张解析图片、提取模板字段 → 目标名可手动微调 → 一键批量重命名，全程可撤销/重做，并写入 JSONL 日志。

## 功能

- **拖放添加**：拖入图片文件或整个文件夹（递归收集），hover 显示相对路径；列表可按当前文件名排序（勾选与目标名随行）
- **AI 解析改名**：视觉模型按模板逐张提取要素，**每张完成即更新列表行**（不等全部完成），流式回显模型输出；支持 `{智能}` 字段（完全由模型起 ≤12 汉字中文名）与「用户提示词」补充要求
- **正则改名**：正则查找/替换实时预览（扩展名不参与），可逐个定位命中行、一键勾选全部命中行，关闭弹层即采用预览值
- **目标名可编辑**：点击当前文件名快速聚焦编辑；变更行淡蓝高亮
- **行级版本回溯**：每行 ↶/↷ 导航最近 10 个历史文件名；改名成功后仍可撤销回旧名（再次执行即改回）
- **防冲突**：目标名已存在时自动追加顺序数字（`名称 1.ext` 起），绝不覆盖
- **批量勾选**：shift+点击范围勾选；已改名的文件自动取消勾选，失败行保留勾选
- **JSONL 日志**：每次重命名记录原/新文件名与结果（成功/失败/跳过）
- **OpenAI 协议兼容**：可对接任意 Chat Completions 兼容端点（OpenAI / vLLM / 本地聚合代理等），支持流式

## 模型配置

右上角「⚙ 模型设置」：

| 项 | 说明 |
|---|---|
| API Base URL | 如 `https://api.openai.com/v1`，或本地服务 `http://127.0.0.1:8317/v1` |
| API Key | 仅保存在本机应用数据目录，不入库 |
| 模型 | 必须是**支持图像输入**的视觉模型（如 `gpt-4o`、`n/llama-3.2-11b-vision`） |
| 超时 | 单请求超时秒数 |

> 模型无关性：本工具只依赖 OpenAI Chat Completions 协议（含 SSE 流式）。纯文本 LLM 无法完成视觉要素提取，请选择视觉模型。

### 本地模型实测记录（2026-09-19）

对本地聚合服务（127.0.0.1:8317）的 4 个候选模型做过连通性 / 视觉能力探测，结论：

| 模型 | 类型 | 结果 |
|---|---|---|
| `n/llama-3.2-11b-vision` | 视觉 | ✅ 唯一可用，1x1 红色测试图正确识别，SSE 流式正常（**推荐**） |
| `n/gemma-4` | 视觉 | ❌ 上游连接断开 |
| `n/mistral-large-2` | 文本 LLM | ❌ 上游认证不可用 |
| `n/llama-3.1-nemotron` | 文本 LLM | ❌ 上游认证不可用 |

## 开发

```bash
pnpm install          # 安装前端 devDependencies
pnpm tauri dev        # 本地开发运行
pnpm check            # 门禁：cargo fmt + clippy + test + node --check
```

### 构建发布

```bash
./scripts/release.sh          # 自动 bump 版本，默认只出 .app
./scripts/release.sh --dmg    # 同时出 .dmg
```

产物在 `release/<version>/`（带版本号的 .app / .dmg / VERSION），最新交付物为 `target/` 下的 `LLM Rename_<版本>.app`（只保留最新一份）。编译缓存（`target/release`、`target/debug`）构建后保留以加速下次增量编译，需要释放空间时手动 `rm -rf target`。版本号显示在窗口标题。

### 流式集成测试（需要本地 LLM 服务）

```bash
cd src-tauri
LLMRENAME_TEST_KEY=<你的key> cargo test vision_stream_smoke -- --ignored
```

可用环境变量覆盖默认目标：`LLMRENAME_TEST_BASE_URL`、`LLMRENAME_TEST_MODEL`。

## 架构

五层单向依赖（详见 [docs/architecture.md](docs/architecture.md)）：

```
Types → Config → Repo → Service → Runtime → UI (src/)
```

更多文档：[产品规格](docs/design/product-spec.md) · [视觉模型契约](docs/design/vision-model-contract.md) · [执行计划](docs/plan/execution-plan.md)

## 配置与日志位置

- macOS：`~/Library/Application Support/com.llmrename.app/llmrename/`
  - `config.json` — 模型 / 模板 / 选项
  - `rename_log.jsonl` — 重命名日志（每行一条）

## License

[MIT](LICENSE)
