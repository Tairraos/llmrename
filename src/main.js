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
      const safeTarget = escapeHtml(t.target);
      return `<tr>
        <td>${i + 1}</td>
        <td class="old-name" title="${safeName}">${safeName}</td>
        <td>
          <input
            class="target-input"
            data-idx="${i}"
            type="text"
            value="${safeTarget}"
            spellcheck="false"
          />
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
}

function closeModelDialog() {
  $("#model-dialog").close();
}

async function saveConfigFromDialog() {
  const cfg = {
    model: {
      base_url: normalizeBaseUrl($("#model-base-url").value.trim()),
      api_key: $("#model-api-key").value.trim(),
      model: $("#model-name").value.trim() || "gpt-4o",
      timeout_secs: Number($("#model-timeout").value) || 60,
    },
    template: {
      pattern: $("#template-pattern").value.trim(),
    },
    options: {
      extensions: ["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff"],
      blacklist: [],
      allowed_suffixes: [".jpg", ".jpeg", ".png", ".webp", ".gif", ".bmp", ".tiff"],
    },
  };
  try {
    await invoke("save_config", { config: cfg });
    window.App.config = cfg;
    showConfigHint("配置已保存", "ok");
    closeModelDialog();
  } catch (err) {
    showConfigHint(String(err), "err");
  }
}

async function loadConfig() {
  try {
    const cfg = await invoke("load_config");
    window.App.config = cfg;
    renderTemplateFields();
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
    window.App.targets.push({
      path: e.path,
      filename: e.filename,
      // 默认目标名 = 原文件名（用户后续编辑或 AI 填充）
      target: e.filename,
    });
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
  showRunHint("视觉模型填充中，请稍候…", "");
  try {
    const items = window.App.targets.map((t) => ({ path: t.path, target: t.target }));
    const filled = await invoke("ai_fill_targets", { items, pattern });
    const byPath = new Map((filled ?? []).map((f) => [f.path, f.target]));
    for (const t of window.App.targets) {
      if (byPath.has(t.path)) t.target = byPath.get(t.path);
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
  const items = window.App.targets.map((t) => ({ path: t.path, target: t.target }));
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
const DEFAULT_EXAMPLE = "value";

function extractFields(pattern) {
  const out = [];
  const re = /\{([a-zA-Z0-9_]+)\}/g;
  let m;
  const seen = new Set();
  while ((m = re.exec(pattern)) !== null) {
    if (seen.has(m[1])) continue;
    seen.add(m[1]);
    out.push({ name: m[1], example: FIELD_EXAMPLES[m[1]] ?? DEFAULT_EXAMPLE });
  }
  return out;
}

/* ---------------- 事件绑定 ---------------- */

function bindEvents() {
  $("#btn-open-model").addEventListener("click", openModelDialog);
  $("#btn-save-config").addEventListener("click", (e) => {
    e.preventDefault();
    saveConfigFromDialog();
  });
  $("#btn-cancel-config").addEventListener("click", closeModelDialog);
  $("#model-dialog").addEventListener("click", (e) => {
    if (e.target === $("#model-dialog")) closeModelDialog();
  });

  $("#template-pattern").addEventListener("input", renderTemplateFields);
  $("#btn-ai-fill").addEventListener("click", aiFillTargets);

  // 拖放
  const zone = $("#drop-zone");
  zone.addEventListener("dragover", (e) => {
    e.preventDefault();
    zone.classList.add("dragging");
  });
  zone.addEventListener("dragleave", () => zone.classList.remove("dragging"));
  zone.addEventListener("drop", (e) => {
    e.preventDefault();
    zone.classList.remove("dragging");
    const paths = [...(e.dataTransfer?.files ?? [])].map((f) => f.path);
    if (paths.length > 0) addPaths(paths);
  });

  $("#btn-pick-files").addEventListener("click", (e) => {
    e.stopPropagation();
    pickFiles();
  });
  $("#btn-pick-dir").addEventListener("click", (e) => {
    e.stopPropagation();
    pickDir();
  });

  // 目标名编辑
  $("#targets-body").addEventListener("input", (e) => {
    const inp = e.target;
    if (!inp.classList.contains("target-input")) return;
    const i = Number(inp.dataset.idx);
    if (Number.isInteger(i) && window.App.targets[i]) {
      window.App.targets[i].target = inp.value;
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
  renderTemplateFields();
  renderTargets();
  renderLogs();

  // 监听后端推送：流式回显 + 预留的长任务进度
  try {
    let streamPath = null;
    await listen("vision-stream", (e) => {
      const p = e.payload ?? {};
      const view = $("#stream-view");
      if (!view) return;
      view.hidden = false;
      if (p.path !== streamPath) {
        streamPath = p.path;
        view.textContent = `── ${p.filename ?? p.path} ──\n`;
      }
      if (p.delta) {
        view.textContent += p.delta;
        view.scrollTop = view.scrollHeight;
      }
      if (p.done) {
        view.textContent += p.error ? `\n✗ ${p.error}\n` : `\n✓ 完成\n`;
        view.scrollTop = view.scrollHeight;
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
