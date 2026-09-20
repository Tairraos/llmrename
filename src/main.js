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
      ? `示例：${fields.map((f) => f.example).join("_")}.jpg（仅供提示；点「用视觉模型填充目标名」生成真实目标名）`
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
      '<tr><td colspan="3"><span class="empty-note">尚未添加文件——拖入图片/文件夹，或点下面按钮选择</span></td></tr>';
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
      return `<tr>
        <td>${i + 1}</td>
        <td class="old-name" title="${safeName}">${safeName}</td>
        <td class="target-cell">
          <input
            class="target-input"
            data-idx="${i}"
            type="text"
            value="${safeNamePart}"
            spellcheck="false"
          />${extLabel}
        </td>
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
    window.App.targets.push({ path: e.path, filename: e.filename, name, ext });
  }
  renderTargets();
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

async function aiFillTargets() {
  const pattern = $("#template-pattern").value.trim();
  if (!pattern) {
    showRunHint("请先填写模板", "err");
    return;
  }
  if (window.App.targets.length === 0) {
    showRunHint("没有可填充的文件，先添加文件", "err");
    return;
  }
  if (!hasTauri()) {
    showRunHint("浏览器模式不支持调用视觉模型", "err");
    return;
  }
  setRunning(true);
  showRunHint(`开始填充目标名：共 ${window.App.targets.length} 个文件…`, "");
  try {
    const items = window.App.targets.map((t) => ({
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
      }
    }
    renderTargets();
    showRunHint("目标名已填充，可手动调整后执行", "ok");
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
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
      window.App.targets[i].target = inp.value.trim();
    }
  }
  const items = window.App.targets.map((t) => ({
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
    showRunHint(
      `完成：成功 ${okCount} · 失败 ${failCount} · 跳过 ${skipCount}`,
      failCount + skipCount > 0 ? "" : "ok",
    );
    // 从列表移除已成功的（保留失败/跳过便于重试）
    const failedPaths = new Set(
      outcomes.filter((o) => o.status !== "ok").map((o) => o.from),
    );
    window.App.targets = window.App.targets.filter((t) => failedPaths.has(t.path));
    renderTargets();
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
const RECOMMENDED_FIELDS = ["人物", "人数", "场景", "动作", "季节", "造型", "天气", "日夜"];

function renderFieldChips() {
  $("#field-chips").innerHTML = RECOMMENDED_FIELDS.map(
    (f) => `<button type="button" class="chip" data-field="${f}">{${f}}</button>`,
  ).join("");
}

function insertFieldChip(field) {
  const inp = $("#template-pattern");
  const prefix = inp.value.trim() ? "_" : "";
  inp.value = inp.value.trim() + prefix + `{${field}}`;
  inp.dispatchEvent(new Event("input"));
  inp.focus();
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
  $("#btn-ai-fill").addEventListener("click", aiFillTargets);

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
    const cell = e.target.closest(".old-name");
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
    if (e.target.closest(".old-name")) preview.hidden = true;
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
    }
  });

  $("#btn-clear-targets").addEventListener("click", () => {
    window.App.targets = [];
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
          break;
        }
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
