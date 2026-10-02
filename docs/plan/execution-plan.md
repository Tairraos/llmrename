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

- [ ] 重命名一致性：跳过黑名单（未实现，见技术债）；同批内同名已由「自动序号」覆盖（v1.0.5 起）
- [ ] 模板字段校验失败时的跳过与报告（对应 LogEntry::Skipped）
- [ ] 并发限制与取消按钮（running 状态真正可中断）
- [ ] 图片预览缩略图列（图片最近才可能加载 -> webview 原生 img 路径）
- [ ] dmg 产物验证（release.sh --dmg）

## 已完成

- [x] （初始化）AGENTS.md 铁律、docs 体系、分层骨架、门禁脚本、CI、git 远程关联
- [x] （R2）拖放/多路径收集、显式目标名可编辑列表、AI 填充、undo/redo（每文件 10 条）、构建发布流程

## 技术债清单

- [ ] `src/main.js` 已达 ~1500 行，远超「单文件 < 300 行」品味不变量：按区块拆模块（渲染 / 行级历史 / 正则 / 事件绑定 / 启动），需要小心 `window.App` 与模块内共享的可变状态（nameSortDir、findCursor、lastRowCheckedIdx 等）。
- [ ] 后端死命令：`undo_rename` / `redo_rename` / `scan_assets` / `preview_rename` 四个 Tauri command 前端已不调用（行级撤销走前端版本链、收集走 collect_targets、预览走前端示例）。`scan_assets/preview_rename` 建议删除；`undo_rename/redo_rename`（HistoryStore 内存撤销）留作 API 待决策。
- [ ] `options.blacklist`（新文件名禁用词）已在 config 建模但 rename_explicit 未校验——product-spec US-6 承诺的「黑名单命中 → skipped」缺失，接线时补单测。
- [ ] `ai_fill_targets` 收尾的批量回填与 item-done 逐行更新重复（幂等兜底，保留）；若拆分 Service 层应把「事件推送 + 渲染」从 command 下沉。

## 验证记录（手工验证的 UI 行为）

| 日期 | 行为 | 验证人 | 结果 |
|---|---|---|---|
| 2026-09-20 | 列表 4 列布局（# / 当前文件名 / 目标文件名 / 操作）、变更行淡蓝高亮、行内 ↶/↷ 按钮渲染 | xiaole | ✅ 通过（headless Chrome 像素校验：淡蓝为成行条带；Node 逻辑测试 19 条断言全过；pnpm check 全绿） |
| 2026-09-20 | 行级历史导航（AI 填充提交版本、手动编辑失焦提交、undo/redo、cap=10、最旧 disabled、编辑态 redo disabled） | xiaole | ✅ 通过（Node 逻辑测试 T1–T8 全过） |
| 2026-09-20 | 执行重命名：跳过目标名一致行、成功后保留行并更新当前名/路径、高亮恢复 | xiaole | ✅ 通过（逻辑测试 T6；真实重命名待 tauri dev 手工复核） |
| 2026-10-02 | 列表行随 AI 逐个返回即时更新（item-done 单行替换、焦点保护、失败不动） | agent | ✅ headless 15 项断言 |
| 2026-10-02 | {智能} 字段（独占/恢复）+ 推荐字段点击切换 + 示例文案精简 | agent | ✅ headless 13 项断言；cargo 单测覆盖智能指南 |
| 2026-10-02 | 解析容错：寒暄包裹/花括号配平/单引号 JSON 多级候选 | agent | ✅ cargo 4 项新单测（含真实故障原文） |
| 2026-10-02 | 正则改名预览（弹层/只读/历史隔离/关闭采用落定/执行消费）+ 查找/选中 | agent | ✅ headless 25+16 项断言 |
| 2026-10-02 | 勾选体系：编辑自动勾选、成功取消勾选、自绘复选框、shift 范围勾选、全局禁选 | agent | ✅ headless 16+17+6 项断言 |
| 2026-10-02 | 同名冲突自动序号（单个/批量连续）+ 用户提示词全链路 | agent | ✅ cargo 3 项新单测；headless 16 项断言 |
| 2026-10-02 | 表头排序（正/倒序、勾选与目标名随行）+ hover 相对路径 | agent | ✅ cargo 1 项新单测；headless 10 项断言 |