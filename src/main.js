/**
 * LLM Rename — 前端逻辑（纯 JS，无框架）
 *
 * 状态全部挂在 window.App，UI 更新走 renderXxx() 纯函数。
 * 所有后端调用经 window.__TAURI__.core.invoke（见 api.js）。
 */
import { invoke, listen } from "./api.js";

const $ = (sel) => document.querySelector(sel);

window.App = {
  config: null, // ModelConfig + TemplateConfig + RenameOptions
  targets: [], // { path, filename, target }[]（待重命名单）
  logs: [], // LogEntry[]
  running: false,
  historyApplied: null, // HistoryStatus
  fill: { running: false, stopRequested: false, stopped: false, okPaths: new Set() },
};

/* ---------------- 渲染 ---------------- */

function escapeHtml(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function renderTemplateFields() {
  const pattern = $("#template-pattern").value;
  const fields = extractFields(pattern);
  const ul = $("#template-fields");
  if (fields.length === 0) {
    ul.innerHTML = '<li class="empty-note">在模板中使用 {字段名} 来引用视觉要素</li>';
    return;
  }
  ul.innerHTML = fields
    .map((f) => `<li>${escapeHtml(f.name)} · ${escapeHtml(f.example)}</li>`)
    .join("");
  $("#template-preview").textContent =
    fields.length > 0
      ? `示例：${renderPatternExample(pattern)}.jpg（仅供提示；点「用视觉模型填充目标名」生成真实目标名）`
      : "";
}

// 文件名拆分：最后一个点之后为扩展名（无点或点在开头则视为无扩展名）
function splitNameExt(filename) {
  const i = filename.lastIndexOf(".");
  if (i <= 0) return { name: filename, ext: "" };
  return { name: filename.slice(0, i), ext: filename.slice(i + 1) };
}

// 由 name + ext 合成完整目标文件名
function composeTarget(t) {
  const name = (t.name ?? "").trim();
  return t.ext ? `${name}.${t.ext}` : name;
}

function renderTargets() {
  const body = $("#targets-body");
  const targets = window.App.targets;
  if (targets.length === 0) {
    body.innerHTML =
      '<tr><td colspan="4"><span class="empty-note">尚未添加文件——拖入图片/文件夹，或点下面按钮选择</span></td></tr>';
    $("#target-summary").textContent = "";
    return;
  }
  body.innerHTML = targets
    .map((t, i) => {
      const safeName = escapeHtml(t.filename);
      const safeNamePart = escapeHtml(t.name);
      const extLabel = t.ext
        ? `<span class="target-ext" title="扩展名不可编辑">.${escapeHtml(t.ext)}</span>`
        : "";
      // 目标名 ≠ 当前文件名：本行会在执行重命名时受影响，淡蓝底色
      const changed = composeTarget(t) !== t.filename;
      // 行级版本历史：有可导航的历史（>1 个版本，或目标名有未提交的编辑）才显示操作按钮
      const h = t.history;
      const edited = composeTarget(t) !== h.versions[h.pos];
      const showOps = h.versions.length > 1 || edited;
      // undo：可回上一版本，或被手动改过（回到当前版本）
      const undoDisabled = !(h.pos > 0 || edited);
      // redo：仅在目标名未被手动改动且有下一版本时可用
      const redoDisabled = !(!edited && h.pos < h.versions.length - 1);
      return `<tr class="${changed ? "changed" : ""}" data-idx="${i}">
        <td>${i + 1}</td>
        <td class="current-name" title="${safeName}">${safeName}</td>
        <td class="target-cell">
          <input
            class="target-input"
            data-idx="${i}"
            type="text"
            value="${safeNamePart}"
            spellcheck="false"
          />${extLabel}
        </td>
        <td class="ops-cell">${
          showOps
            ? `<span class="ops">
                <button type="button" class="op-btn" data-op="undo" data-idx="${i}" title="在目标名位置显示上一个文件名" ${undoDisabled ? "disabled" : ""}><svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24" aria-hidden="true"><path d="M0 0h24v24H0z" fill="none" /><path fill="currentColor" d="M15 7H5.06l2.97-2.97l-1.06-1.06l-3.895 3.895a1.26 1.26 0 0 0 0 1.77L6.97 12.53l1.06-1.06L5.06 8.5H15c2.48 0 4.5 2.02 4.5 4.5s-2.02 4.5-4.5 4.5H7V19h8c3.31 0 6-2.69 6-6s-2.69-6-6-6" /></svg></button>
                <button type="button" class="op-btn" data-op="redo" data-idx="${i}" title="在目标名位置显示下一个文件名" ${redoDisabled ? "disabled" : ""}><svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 24 24" aria-hidden="true"><path d="M0 0h24v24H0z" fill="none" /><path fill="currentColor" d="M20.925 6.865L17.03 2.97l-1.06 1.06L18.94 7H9c-3.31 0-6 2.69-6 6s2.69 6 6 6h8v-1.5H9c-2.48 0-4.5-2.02-4.5-4.5S6.52 8.5 9 8.5h9.94l-2.97 2.97l1.06 1.06l3.895-3.895a1.26 1.26 0 0 0 0-1.77" /></svg></button>
              </span>`
            : ""
        }</td>
      </tr>`;
    })
    .join("");
  $("#target-summary").textContent = `共 ${targets.length} 个文件`;
}

function renderLogs() {
  const list = $("#log-list");
  const logs = window.App.logs;
  $("#log-summary").textContent = logs.length > 0 ? `共 ${logs.length} 条` : "暂无日志";
  if (logs.length === 0) {
    list.innerHTML = '<li class="empty-note">还没有重命名记录，先添加文件并执行一次</li>';
    return;
  }
  list.innerHTML = logs
    .map((l) => {
      const cls = l.status === "ok" ? "ok" : l.status === "skipped" ? "skip" : "err";
      return `<li class="${cls}">
        <span class="ts">${escapeHtml(formatTs(l.ts))}</span>
        <span>${escapeHtml(l.asset)}</span>
        ${l.target ? `→ <span class="target">${escapeHtml(l.target)}</span>` : ""}
        ${l.error ? `<span class="err-text">${escapeHtml(l.error)}</span>` : ""}
      </li>`;
    })
    .join("");
}

function renderHistory(historyStatus) {
  window.App.historyApplied = historyStatus;
  $("#btn-undo").disabled = !historyStatus?.undoable;
  $("#btn-redo").disabled = !historyStatus?.redoable;
  if (historyStatus?.applied) {
    showRunHint(`已应用：${historyStatus.applied}`, "ok");
  }
}

function formatTs(ts) {
  if (!ts) return "-";
  const d = new Date(ts * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/* ---------------- 浏览器能力探测 ---------------- */

function hasTauri() {
  return typeof window.__TAURI__ !== "undefined";
}

function showRunHint(text, kind = "") {
  const el = $("#run-hint");
  el.textContent = text;
  el.className = `hint ${kind}`;
}

function showConfigHint(text, kind = "") {
  const el = $("#config-hint");
  el.textContent = text;
  el.className = `hint ${kind}`;
}

function showTemplateHint(text, kind = "") {
  const el = $("#template-hint");
  el.textContent = text;
  el.className = `hint ${kind}`;
}

/* ---------------- 大模型调试台（console） ---------------- */

function consoleAppend(html) {
  const el = $("#console-view");
  if (!el) return;
  if (el.dataset.init !== "1") {
    el.textContent = "";
    el.dataset.init = "1";
  }
  el.insertAdjacentHTML("beforeend", html);
  el.scrollTop = el.scrollHeight;
}

function consoleStatus(text, cls = "") {
  const ts = new Date().toTimeString().slice(0, 8);
  consoleAppend(`<span class="c-ts">[${ts}]</span><span class="${cls}"> ${escapeHtml(text)}</span>\n`);
}

function consoleFileHeader(filename) {
  consoleAppend(`<span class="c-file">── ${escapeHtml(filename)} ──</span>\n`);
}

/* ---------------- 模板设置 dialog ---------------- */

function openTemplateDialog() {
  const cfg = window.App.config;
  $("#template-pattern").value = cfg?.template?.pattern ?? "";
  renderTemplateFields();
  $("#template-dialog").showModal();
}

function closeTemplateDialog() {
  $("#template-dialog").close();
}

function renderTemplateSummary() {
  const pattern = window.App.config?.template?.pattern;
  $("#template-summary").textContent = pattern || "（未设置，点「📝 模板设置」配置）";
}

/* ---------------- 模型配置 dialog ---------------- */

function openModelDialog() {
  const cfg = window.App.config;
  if (cfg) {
    $("#model-base-url").value = cfg.model.base_url || "";
    $("#model-api-key").value = cfg.model.api_key || "";
    $("#model-name").value = cfg.model.model || "gpt-4o";
    $("#model-timeout").value = cfg.model.timeout_secs ?? 60;
  }
  $("#model-dialog").showModal();
  // 打开时静默拉取模型列表（失败不打扰用户）
  loadModels(true);
}

function closeModelDialog() {
  $("#model-dialog").close();
}

/* ---------------- 模型列表加载（⟳） ---------------- */

async function loadModels(silent = false) {
  const base_url = normalizeBaseUrl($("#model-base-url").value.trim());
  const api_key = $("#model-api-key").value.trim();
  const timeout_secs = Number($("#model-timeout").value) || 60;
  if (!hasTauri()) {
    if (!silent) showConfigHint("浏览器模式不支持调用后端", "err");
    return;
  }
  const btn = $("#btn-load-models");
  btn.disabled = true;
  if (!silent) showConfigHint("正在获取模型列表…", "");
  try {
    const models = await invoke("list_models", { baseUrl: base_url, apiKey: api_key, timeoutSecs: timeout_secs });
    $("#model-options").innerHTML = models
      .map((m) => `<option value="${escapeHtml(m)}"></option>`)
      .join("");
    if (!silent) {
      showConfigHint(`已获取 ${models.length} 个模型，下拉选择或继续手动输入`, "ok");
    }
  } catch (err) {
    if (!silent) showConfigHint(String(err), "err");
  } finally {
    btn.disabled = false;
  }
}

async function saveModelFromDialog() {
  const model = {
    base_url: normalizeBaseUrl($("#model-base-url").value.trim()),
    api_key: $("#model-api-key").value.trim(),
    model: $("#model-name").value.trim() || "gpt-4o",
    timeout_secs: Number($("#model-timeout").value) || 60,
  };
  try {
    await invoke("save_model_config", { model });
    window.App.config = { ...window.App.config, model };
    showConfigHint("模型配置已保存", "ok");
    closeModelDialog();
  } catch (err) {
    showConfigHint(String(err), "err");
  }
}

async function saveTemplate() {
  const pattern = $("#template-pattern").value.trim();
  try {
    await invoke("save_template_config", { pattern });
    window.App.config = { ...window.App.config, template: { pattern } };
    showTemplateHint("模板已保存", "ok");
    renderTemplateSummary();
    closeTemplateDialog();
  } catch (err) {
    showTemplateHint(String(err), "err");
  }
}

async function loadConfig() {
  try {
    const cfg = await invoke("load_config");
    window.App.config = cfg;
    // 回填已保存的模板（否则保存模型时会把空模板写进配置）
    $("#template-pattern").value = cfg?.template?.pattern ?? "";
    renderTemplateFields();
    renderTemplateSummary();
  } catch (err) {
    showConfigHint(`读取配置失败：${err}`, "err");
  }
}

/* ---------------- 添加目标（拖放 / 选择） ---------------- */

async function addPaths(paths) {
  if (!paths || paths.length === 0) return;
  setRunning(true);
  try {
    const entries = await invoke("collect_targets", { paths });
    mergeTargets(entries ?? []);
    showRunHint(`已添加 ${entries?.length ?? 0} 个文件`, "ok");
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
    setRunning(false);
  }
}

function mergeTargets(entries) {
  const existing = new Set(window.App.targets.map((t) => t.path));
  for (const e of entries) {
    if (existing.has(e.path)) continue;
    existing.add(e.path);
    const { name, ext } = splitNameExt(e.filename);
    // 默认目标名 = 原文件名（不含扩展名；扩展名固定不可编辑）
    // history：行级版本历史（目标名位置的回溯导航），versions[0] 为最早、末尾为最新；
    // pos 指向目标名当前展示的版本。
    const history = { versions: [e.filename], pos: 0 };
    window.App.targets.push({ path: e.path, filename: e.filename, name, ext, history });
  }
  renderTargets();
}

/* ---------------- 行级版本历史（目标名位置 undo/redo） ---------------- */

// 行记录的版本历史最多保留条数（与全局历史容量一致）
const ROW_HISTORY_CAP = 10;

// 目标名相对当前版本是否被手动改过（未提交）
function rowEdited(t) {
  return composeTarget(t) !== t.history.versions[t.history.pos];
}

// 目标名提交：把目标名（可能手动编辑过）追加为该行的最新版本，pos 指向它
function rowCommit(t) {
  const h = t.history;
  const target = composeTarget(t);
  if (h.versions[h.pos] === target) return;
  // 从 pos 之后截断（新分支），再追加
  h.versions = h.versions.slice(0, h.pos + 1);
  h.versions.push(target);
  // 保留最近 ROW_HISTORY_CAP 个版本
  if (h.versions.length > ROW_HISTORY_CAP) h.versions = h.versions.slice(-ROW_HISTORY_CAP);
  h.pos = h.versions.length - 1;
}

// 行级 undo：在目标名位置显示上一个文件名
function rowUndo(t) {
  const h = t.history;
  const edited = composeTarget(t) !== h.versions[h.pos];
  if (edited) {
    // 目标名被手动改过：undo 先回到未提交时的版本
    const target = h.versions[h.pos];
    const { name } = splitNameExt(target);
    t.name = name;
    return;
  }
  if (h.pos <= 0) return;
  h.pos -= 1;
  const { name } = splitNameExt(h.versions[h.pos]);
  t.name = name;
}

// 行级 redo：在目标名位置显示下一个文件名（目标名被手动改过时无下一个）
function rowRedo(t) {
  const h = t.history;
  if (composeTarget(t) !== h.versions[h.pos]) {
    // 目标名被手动改过（未提交）：redo 无下一个，disabled
    return;
  }
  if (h.pos >= h.versions.length - 1) return;
  h.pos += 1;
  const { name } = splitNameExt(h.versions[h.pos]);
  t.name = name;
}

// 目标文件被真正重命名后：更新当前文件名、路径与版本链
function rowApplied(t, newFilename) {
  t.filename = newFilename;
  t.path = joinPath(t.path, newFilename);
  t.ext = splitNameExt(newFilename).ext;
  t.name = splitNameExt(newFilename).name;
  const h = t.history;
  // 版本链以当前文件名为准重建（历史中如果有同名跳过）
  const versions = [...new Set([...h.versions, newFilename])];
  t.history = { versions: versions.slice(-ROW_HISTORY_CAP), pos: versions.length - 1 };
}

function joinPath(path, filename) {
  const idx = path.lastIndexOf("/");
  return idx < 0 ? filename : path.slice(0, idx + 1) + filename;
}

// 重命名后同步每行路径与名字（行内 undo/redo 只动目标名，真正改名在「开始重命名」）
function syncRowAfterRename(outcomes) {
  const byFromPath = new Map(
    outcomes
      .filter((o) => o.status === "ok")
      .map((o) => [normalizePath(o.from), normalizePath(o.to)]),
  );
  const byToPath = new Map(
    Array.from(byFromPath.entries()).map(([from, to]) => [to, from]),
  );
  for (const t of window.App.targets) {
    const p = normalizePath(t.path);
    if (byFromPath.has(p)) {
      const to = byFromPath.get(p);
      const newFilename = to.slice(to.lastIndexOf("/") + 1);
      rowApplied(t, newFilename);
    }
  }
}

function normalizePath(p) {
  return String(p).replace(/\\/g, "/");
}

async function pickFiles() {
  if (!hasTauri()) {
    const p = prompt("（浏览器模式）请输入图片路径（多个用换行分隔）：");
    if (p) {
      await addPaths(p.split(/\r?\n/).map((s) => s.trim()).filter(Boolean));
    }
    return;
  }
  try {
    const files = await invoke("pick_files");
    await addPaths(files);
  } catch (err) {
    showRunHint(`选择文件失败：${err}`, "err");
  }
}

async function pickDir() {
  if (!hasTauri()) {
    const p = prompt("（浏览器模式）请输入文件夹绝对路径：");
    if (p) await addPaths([p]);
    return;
  }
  try {
    const dir = await invoke("pick_dir");
    if (dir) await addPaths([dir]);
  } catch (err) {
    showRunHint(`选择文件夹失败：${err}`, "err");
  }
}

/* ---------------- AI 填充目标名 ---------------- */

async function aiFillTargets(resume = false) {
  const pattern = $("#template-pattern").value.trim();
  if (!pattern) {
    showRunHint("请先填写模板", "err");
    return;
  }
  const source = resume
    ? window.App.targets.filter((t) => !window.App.fill.okPaths.has(t.path))
    : window.App.targets;
  if (source.length === 0) {
    showRunHint("没有可填充的文件，先添加文件", "err");
    return;
  }
  if (!hasTauri()) {
    showRunHint("浏览器模式不支持调用视觉模型", "err");
    return;
  }
  setRunning(true);
  window.App.fill.running = true;
  window.App.fill.stopRequested = false;
  window.App.fill.stopped = false;
  updateStopButton();
  showRunHint(`开始填充目标名：共 ${source.length} 个文件…`, "");
  try {
    const items = source.map((t) => ({
      path: t.path,
      target: composeTarget(t),
    }));
    const filled = await invoke("ai_fill_targets", { items, pattern });
    const byPath = new Map((filled ?? []).map((f) => [f.path, f.target]));
    for (const t of window.App.targets) {
      if (byPath.has(t.path)) {
        // 模型返回完整文件名（后端已按原扩展名拼接），拆回 name + ext
        const { name } = splitNameExt(byPath.get(t.path));
        t.name = name;
        // AI 填充结果作为新版本提交（行级 undo 可回溯到填充前）
        rowCommit(t);
      }
    }
    renderTargets();
    const remaining = window.App.targets.filter(
      (t) => !window.App.fill.okPaths.has(t.path),
    );
    if (window.App.fill.stopRequested && remaining.length > 0) {
      window.App.fill.stopped = true;
      showRunHint(`已停止：剩余 ${remaining.length} 个文件未解析，点「继续」恢复`, "");
    } else {
      showRunHint("目标名已填充，可手动调整后执行", "ok");
    }
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
    window.App.fill.running = false;
    window.App.fill.stopRequested = false;
    updateStopButton();
    setRunning(false);
  }
}

/* ---------------- 执行 / 撤销 / 重做 ---------------- */

async function executeRename() {
  if (window.App.targets.length === 0) {
    showRunHint("没有待重命名的文件", "err");
    return;
  }
  if (!hasTauri()) {
    showRunHint("浏览器模式不可执行", "err");
    return;
  }
  // 收集当前目标名（可能刚编辑过）
  const inputs = [...document.querySelectorAll(".target-input")];
  for (const inp of inputs) {
    const i = Number(inp.dataset.idx);
    if (Number.isInteger(i) && window.App.targets[i]) {
      window.App.targets[i].name = inp.value.trim();
      // 编辑结果提交为版本（行级 undo 可回溯）
      rowCommit(window.App.targets[i]);
    }
  }
  // 目标文件名与当前文件名一致的行：无需重命名动作（跳过），不发后端
  const changedItems = window.App.targets.filter((t) => composeTarget(t) !== t.filename);
  const skipSame = window.App.targets.length - changedItems.length;
  if (changedItems.length === 0) {
    showRunHint(`当前文件名与目标文件名一致，无需重命名（跳过 ${skipSame} 行）`, "");
    return;
  }
  const items = changedItems.map((t) => ({
    path: t.path,
    target: composeTarget(t),
  }));
  setRunning(true);
  showRunHint("重命名中…", "");
  try {
    const [outcomes, historyStatus] = await invoke("rename_items", { items });
    const okCount = outcomes.filter((o) => o.status === "ok").length;
    const failCount = outcomes.filter((o) => o.status === "failed").length;
    const skipCount = outcomes.filter((o) => o.status === "skipped").length;
    renderHistory(historyStatus);
    await refreshLogs();
    // 成功的行保留在列表：更新当前文件名/路径/版本链，背景色随一致恢复
    syncRowAfterRename(outcomes);
    renderTargets();
    showRunHint(
      `完成：成功 ${okCount} · 失败 ${failCount} · 跳过 ${skipCount + skipSame}`,
      failCount + skipCount > 0 ? "" : "ok",
    );
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
    setRunning(false);
  }
}

async function undoRename() {
  try {
    const st = await invoke("undo_rename");
    renderHistory(st);
    await refreshLogs();
  } catch (err) {
    showRunHint(`撤销失败：${err}`, "err");
  }
}

async function redoRename() {
  try {
    const st = await invoke("redo_rename");
    renderHistory(st);
    await refreshLogs();
  } catch (err) {
    showRunHint(`重做失败：${err}`, "err");
  }
}

async function refreshHistory() {
  try {
    const st = await invoke("history_status");
    renderHistory(st);
  } catch {
    // 非 Tauri 忽略
  }
}

async function refreshLogs() {
  try {
    const logs = await invoke("list_logs", { limit: 200 });
    window.App.logs = logs ?? [];
    renderLogs();
  } catch (err) {
    $("#log-summary").textContent = `读取日志失败：${err}`;
  }
}

function openLogsDialog() {
  $("#logs-dialog").showModal();
  refreshLogs();
}

function closeLogsDialog() {
  $("#logs-dialog").close();
}

async function openLogDir() {
  try {
    await invoke("open_log_dir");
  } catch (err) {
    showRunHint(String(err), "err");
  }
}

async function loadVersion() {
  try {
    const v = await invoke("get_version");
    $("#app-version").textContent = `v${v}`;
  } catch {
    // 浏览器模式无版本
  }
}

/* ---------------- 填充停止/继续按钮状态机 ---------------- */
/* 未开始 → disabled「停止」；进行中 → 可点「停止」；停止中 → disabled「停止中…」；
   已停止且有未解析文件 → 可点「继续」；全部完成 → disabled */
function updateStopButton() {
  const btn = $("#btn-stop-fill");
  if (!btn) return;
  const f = window.App.fill;
  if (f.running && f.stopRequested) {
    btn.disabled = true;
    btn.textContent = "停止中…";
  } else if (f.running) {
    btn.disabled = false;
    btn.textContent = "停止";
  } else if (f.stopped) {
    btn.disabled = false;
    btn.textContent = "继续";
  } else {
    btn.disabled = true;
    btn.textContent = "停止";
  }
}

function setRunning(r) {
  window.App.running = r;
  for (const id of ["btn-execute", "btn-ai-fill", "btn-pick-files", "btn-pick-dir"]) {
    const el = $(`#${id}`);
    if (el) el.disabled = r;
  }
}

/* ---------------- 工具 ---------------- */

function normalizeBaseUrl(s) {
  if (!s) return "https://api.openai.com/v1";
  return s.replace(/\/+$/, "");
}

const FIELD_EXAMPLES = {
  人物: "女人",
  人数: "2",
  场景: "街头",
  动作: "跳舞",
  季节: "夏天",
  造型: "叉腰",
  天气: "晴天",
  日夜: "夜晚",
  色调: "暖",
  date: "2026-09-18",
  time: "14-30-05",
  camera: "a7m4",
  scene: "city_night",
  location: "tokyo",
  subject: "cat",
  description: "sunset_walk",
  version: "v1",
  author: "tairraos",
  event: "wedding",
};
const DEFAULT_EXAMPLE = "值";

// 推荐的中文字段（与后端 field_example 对应，视觉模型可从图片提取）
const RECOMMENDED_FIELDS = ["人物", "人数", "场景", "动作", "季节", "造型", "天气", "日夜", "色调"];

function renderFieldChips() {
  $("#field-chips").innerHTML = RECOMMENDED_FIELDS.map(
    (f) => `<button type="button" class="chip" data-field="${f}">{${f}}</button>`,
  ).join("");
}

function insertFieldChip(field) {
  const inp = $("#template-pattern");
  const prefix = inp.value.trim() ? "-" : "";
  inp.value = inp.value.trim() + prefix + `{${field}}`;
  inp.dispatchEvent(new Event("input"));
  inp.focus();
}

// 按模板实际分隔符渲染示例（字段替换为示例值）
function renderPatternExample(pattern) {
  return pattern.replace(/\{([^{}]+)\}/g, (_m, name) => {
    const key = name.trim();
    return FIELD_EXAMPLES[key] ?? DEFAULT_EXAMPLE;
  });
}

function extractFields(pattern) {
  const out = [];
  const re = /\{([^{}]+)\}/g;
  let m;
  const seen = new Set();
  while ((m = re.exec(pattern)) !== null) {
    const name = m[1].trim();
    // 与后端 extract_fields 一致：字段名为 Unicode 字母数字（含中文）+ 下划线
    if (!name || !/^[\p{L}\p{N}_]+$/u.test(name) || seen.has(name)) continue;
    seen.add(name);
    out.push({ name, example: FIELD_EXAMPLES[name] ?? DEFAULT_EXAMPLE });
  }
  return out;
}

/* ---------------- 事件绑定 ---------------- */

function bindEvents() {
  $("#btn-open-model").addEventListener("click", openModelDialog);
  $("#btn-open-template").addEventListener("click", openTemplateDialog);
  $("#btn-open-logs").addEventListener("click", openLogsDialog);
  $("#btn-close-logs").addEventListener("click", closeLogsDialog);

  // 模板 dialog 防误关（同模型 dialog：点遮罩/Esc/回车不关闭）
  $("#template-form").addEventListener("submit", (e) => e.preventDefault());
  $("#template-dialog").addEventListener("cancel", (e) => e.preventDefault());
  $("#btn-cancel-template").addEventListener("click", closeTemplateDialog);
  $("#btn-save-config").addEventListener("click", (e) => {
    e.preventDefault();
    saveModelFromDialog();
  });
  $("#btn-cancel-config").addEventListener("click", closeModelDialog);
  $("#btn-save-template").addEventListener("click", saveTemplate);
  $("#btn-load-models").addEventListener("click", (e) => {
    e.preventDefault();
    loadModels();
  });

  // 防误关：这不是 alert——点遮罩/Esc/回车都不关闭，避免丢掉已输入的内容
  $("#model-form").addEventListener("submit", (e) => e.preventDefault());
  $("#model-dialog").addEventListener("cancel", (e) => e.preventDefault());

  $("#template-pattern").addEventListener("input", renderTemplateFields);
  $("#field-chips").addEventListener("click", (e) => {
    const chip = e.target.closest(".chip");
    if (chip) insertFieldChip(chip.dataset.field);
  });
  $("#btn-ai-fill").addEventListener("click", () => aiFillTargets(false));
  $("#btn-stop-fill").addEventListener("click", () => {
    const f = window.App.fill;
    if (f.running && !f.stopRequested) {
      f.stopRequested = true;
      updateStopButton();
      invoke("stop_ai_fill").catch((err) => showRunHint(String(err), "err"));
    } else if (!f.running && f.stopped) {
      aiFillTargets(true);
    }
  });

  $("#btn-pick-files").addEventListener("click", (e) => {
    e.stopPropagation();
    pickFiles();
  });
  $("#btn-pick-dir").addEventListener("click", (e) => {
    e.stopPropagation();
    pickDir();
  });

  // hover 原文件名：弹出图片预览（asset 协议加载本地文件）
  const preview = $("#img-preview");
  const previewImg = $("#img-preview-img");
  const fileSrc = (path) => {
    try {
      return window.__TAURI__.core.convertFileSrc(path);
    } catch {
      return null;
    }
  };
  $("#targets-body").addEventListener("mouseover", (e) => {
    const cell = e.target.closest(".current-name");
    if (!cell || !hasTauri()) return;
    const i = Number(cell.closest("tr")?.querySelector(".target-input")?.dataset.idx);
    const t = window.App.targets[i];
    if (!t) return;
    const src = fileSrc(t.path);
    if (!src) return;
    previewImg.src = src;
    preview.hidden = false;
  });
  $("#targets-body").addEventListener("mousemove", (e) => {
    if (preview.hidden) return;
    const pad = 14;
    const w = preview.offsetWidth || 280;
    const h = preview.offsetHeight || 280;
    let x = e.clientX + pad;
    let y = e.clientY + pad;
    if (x + w > window.innerWidth - 8) x = e.clientX - w - pad;
    if (y + h > window.innerHeight - 8) y = e.clientY - h - pad;
    preview.style.left = `${Math.max(8, x)}px`;
    preview.style.top = `${Math.max(8, y)}px`;
  });
  $("#targets-body").addEventListener("mouseout", (e) => {
    if (e.target.closest(".current-name")) preview.hidden = true;
  });
  // 图片加载失败（文件被移动/非图片）时隐藏弹层
  previewImg.addEventListener("error", () => {
    preview.hidden = true;
  });

  // 目标名编辑（仅文件名部分；扩展名固定不可编辑）
  $("#targets-body").addEventListener("input", (e) => {
    const inp = e.target;
    if (!inp.classList.contains("target-input")) return;
    const i = Number(inp.dataset.idx);
    if (Number.isInteger(i) && window.App.targets[i]) {
      window.App.targets[i].name = inp.value;
      // 实时刷新行高亮（目标名 ≠ 当前名时淡蓝）
      const tr = inp.closest("tr");
      if (tr) tr.classList.toggle("changed", composeTarget(window.App.targets[i]) !== window.App.targets[i].filename);
    }
  });
  // 失焦提交版本（行级 undo 可回溯到编辑前）
  $("#targets-body").addEventListener("change", (e) => {
    const inp = e.target;
    if (!inp.classList.contains("target-input")) return;
    const i = Number(inp.dataset.idx);
    const t = window.App.targets[i];
    if (Number.isInteger(i) && t) rowCommit(t);
    renderTargets();
  });
  // 操作列：行内 undo/redo（在目标名位置显示上/下一个文件名）
  $("#targets-body").addEventListener("click", (e) => {
    const btn = e.target.closest(".op-btn");
    if (!btn) return;
    const i = Number(btn.dataset.idx);
    const t = window.App.targets[i];
    if (!Number.isInteger(i) || !t) return;
    if (btn.dataset.op === "undo") rowUndo(t);
    else if (btn.dataset.op === "redo") rowRedo(t);
    renderTargets();
  });

  $("#btn-clear-targets").addEventListener("click", () => {
    window.App.targets = [];
    window.App.fill.okPaths.clear();
    window.App.fill.stopped = false;
    updateStopButton();
    renderTargets();
  });
  $("#btn-execute").addEventListener("click", executeRename);
  $("#btn-undo").addEventListener("click", undoRename);
  $("#btn-redo").addEventListener("click", redoRename);

  $("#btn-refresh-log").addEventListener("click", refreshLogs);
  $("#btn-open-log").addEventListener("click", openLogDir);
}

/* ---------------- 启动 ---------------- */

async function boot() {
  bindEvents();
  await loadConfig();
  await loadVersion();
  await refreshLogs();
  await refreshHistory();
  renderFieldChips();
  renderTemplateFields();
  renderTargets();
  renderLogs();

  // 监听后端推送：流式回显 + 预留的长任务进度
  try {
    // 文件拖放：Tauri 在窗口级拦截文件拖放（DOM drop 事件不触发，
    // 且 WKWebView 的 File 对象没有 path 属性），必须用官方 drag-drop 事件拿真实路径。
    // 整个窗口任意位置都可拖放；光标落在收集框内时高亮。
    const zone = $("#drop-zone");
    const zoneHit = (pos) => {
      if (!pos || !zone) return false;
      const dpr = window.devicePixelRatio || 1;
      const r = zone.getBoundingClientRect();
      const x = pos.x / dpr;
      const y = pos.y / dpr;
      return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
    };
    await listen("tauri://drag-enter", (e) => {
      if (zoneHit(e.payload?.position)) zone?.classList.add("dragging");
    });
    await listen("tauri://drag-over", (e) => {
      zone?.classList.toggle("dragging", zoneHit(e.payload?.position));
    });
    await listen("tauri://drag-leave", () => zone?.classList.remove("dragging"));
    await listen("tauri://drag-drop", (e) => {
      zone?.classList.remove("dragging");
      const paths = e.payload?.paths ?? [];
      if (paths.length > 0) addPaths(paths);
    });

    let streamPath = null;
    await listen("vision-stream", (e) => {
      const p = e.payload ?? {};
      if (p.path !== streamPath) {
        streamPath = p.path;
        consoleFileHeader(p.filename ?? p.path);
      }
      // 模型流式输出（含思考过程字段）原样进调试台
      if (p.delta) consoleAppend(escapeHtml(p.delta));
      if (p.done && p.error) consoleStatus(p.error, "c-err");
    });
    await listen("vision-status", (e) => {
      const p = e.payload ?? {};
      switch (p.phase) {
        case "connecting":
          consoleStatus(`正在连接大模型（${p.url ?? "?"}）…`, "c-info");
          showRunHint(`正在连接大模型（${p.url ?? "?"}）…`, "");
          break;
        case "extracting":
          showRunHint(`正在解析 ${p.filename ?? "?"}（模型输出中）…`, "");
          break;
        case "parsing":
          consoleStatus(`正在解析 ${p.filename ?? "?"} 的模型返回…`, "c-info");
          showRunHint(`正在解析 ${p.filename ?? "?"} 的模型返回…`, "");
          break;
        case "item-done": {
          const line = `[${p.index ?? "?"}/${p.total ?? "?"}] ${p.filename ?? "?"} ${p.ok ? "✓ 已填充" : "✗ 失败"}${p.target ? ` → ${p.target}` : ""}`;
          consoleStatus(line, p.ok ? "c-ok" : "c-err");
          showRunHint(line, p.ok ? "" : "err");
          if (p.ok) window.App.fill.okPaths.add(p.path);
          break;
        }
        case "stopped":
          consoleStatus(
            `已停止：剩余 ${p.remaining ?? "?"} 个文件未解析，可点「继续」恢复`,
            "c-err",
          );
          break;
      }
    });
    await listen("rename-progress", (e) => {
      showRunHint(`进度：${e.payload?.done ?? "?"}/${e.payload?.total ?? "?"}`, "");
    });
  } catch {
    // 浏览器模式无 listen，忽略
  }

  if (!hasTauri()) {
    showRunHint("浏览器模式：未检测到 Tauri 运行时，仅演示布局");
  }
}

boot();
