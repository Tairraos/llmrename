# 执行计划

状态图例：`[x]` 已完成　`[ ]` 活跃　`[~]` 冻结 / 待决策　`[!]` 技术债

## 活跃计划

### MVP-1：可运行的端到端重命名（当前迭代）

- [ ] 后端：`types` 层完整模型与解析（AssetEntry / RenameRequest / ExtractedFields / LogEntry）
- [ ] 后端：`config` 层配置加载保存（模型、模板、路径、选项）
- [ ] 后端：`repo` 层扫描 / 重命名 / JSONL 日志
- [ ] 后端：`service` 层模板渲染、视觉模型调用、防冲突命名
- [ ] 后端：`runtime` 层 Tauri commands（save_config / load_config / scan_assets / preview_rename / execute_rename / list_logs）
- [ ] 前端：配置表单（模型 + 模板 + 选项）
- [ ] 前端：资产库选择 + 表格预览 + 执行入口
- [ ] 前端：模板字段自动发现与示例提示
- [ ] 日志面板（查看 / 打开日志目录）
- [ ] 端到端冒烟：`tauri dev` 下扫描 → 预览 → 执行 → 日志落盘
- [ ] 门禁：`scripts/check.sh` 全绿 + CI 工作流

### MVP-2（推荐顺序）

- [ ] 重命名一致性：同批内同名去重、跳过黑名单、只改扩展名不碰内容
- [ ] 模板字段校验失败时的跳过与报告（对应 LogEntry::Skipped）
- [ ] 并发限制与取消按钮（running 状态真正可中断）
- [ ] 图片预览缩略图列（图片最近才可能加载 -> webview 原生 img 路径）

## 已完成

- [x] （初始化）AGENTS.md 铁律、docs 体系、分层骨架、门禁脚本、CI、git 远程关联

## 技术债清单

- [ ] 无

## 验证记录（手工验证的 UI 行为）

| 日期 | 行为 | 验证人 | 结果 |
|---|---|---|---|
| - | （本表用于登记人工验证，尚未有记录） | - | - |