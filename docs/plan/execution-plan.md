# 执行计划

状态图例：`[x]` 已完成　`[ ]` 活跃　`[~]` 冻结 / 待决策　`[!]` 技术债

## 活跃计划

### MVP-1：可运行的端到端重命名（当前迭代）

- [x] 后端：`types` 层完整模型与解析（AssetEntry / HistoryStatus / RenameItem / LogEntry）
- [x] 后端：`config` 层配置加载保存（模型、模板、路径、选项）
- [x] 后端：`repo` 层扫描（递归多路径）/ 重命名 / JSONL 日志 / collect_targets
- [x] 后端：`service` 层模板渲染、视觉模型调用、显式目标名执行、undo/redo 历史
- [x] 后端：`runtime` 层 Tauri commands（含 collect_targets / ai_fill_targets / rename_items / undo_rename / redo_rename / history_status / get_version）
- [x] 前端：模型配置 dialog
- [x] 前端：拖放区（拖文件/文件夹/点击多选）+ 可编辑目标名列
- [x] 前端：模板字段自动发现与示例提示 + AI 填充目标名
- [x] 撤销 / 重做（全局按钮 + 行级 ↶/↷，每文件 10 条历史）
- [x] 日志面板（查看 / 打开日志目录）
- [x] 构建发布：target 在项目根、release.sh（bump 版本/默认 app/可选 dmg/清中间产物）、版本显示在标题
- [x] 门禁：`scripts/check.sh` 全绿 + CI 工作流

### MVP-2（推荐顺序）

- [ ] 重命名一致性：同批内同名去重、跳过黑名单、只改扩展名不碰内容
- [ ] 模板字段校验失败时的跳过与报告（对应 LogEntry::Skipped）
- [ ] 并发限制与取消按钮（running 状态真正可中断）
- [ ] 图片预览缩略图列（图片最近才可能加载 -> webview 原生 img 路径）
- [ ] dmg 产物验证（release.sh --dmg）

## 已完成

- [x] （初始化）AGENTS.md 铁律、docs 体系、分层骨架、门禁脚本、CI、git 远程关联
- [x] （R2）拖放/多路径收集、显式目标名可编辑列表、AI 填充、undo/redo（每文件 10 条）、构建发布流程

## 技术债清单

- [ ] 无

## 验证记录（手工验证的 UI 行为）

| 日期 | 行为 | 验证人 | 结果 |
|---|---|---|---|
| 2026-09-20 | 列表 4 列布局（# / 当前文件名 / 目标文件名 / 操作）、变更行淡蓝高亮、行内 ↶/↷ 按钮渲染 | xiaole | ✅ 通过（headless Chrome 像素校验：淡蓝为成行条带；Node 逻辑测试 19 条断言全过；pnpm check 全绿） |
| 2026-09-20 | 行级历史导航（AI 填充提交版本、手动编辑失焦提交、undo/redo、cap=10、最旧 disabled、编辑态 redo disabled） | xiaole | ✅ 通过（Node 逻辑测试 T1–T8 全过） |
| 2026-09-20 | 执行重命名：跳过目标名一致行、成功后保留行并更新当前名/路径、高亮恢复 | xiaole | ✅ 通过（逻辑测试 T6；真实重命名待 tauri dev 手工复核） |