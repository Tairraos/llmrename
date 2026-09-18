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
  assets: [], // AssetEntry[]
  previews: [], // 与 assets 等长的猜测新名
  selected: new Set(), // assets 下标集合
  logs: [], // LogEntry[]
  running: false,
  logView: "latest",
};

/* ---------------- 渲染 ---------------- */

function fmtSize(bytes) {
  if (bytes == null) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function fmtTime(secs) {
  if (!secs) return "-";
  const d = new Date(secs * 1000);
  return d.toLocaleString();
}

function statusText(status) {
  switch (status) {
    case "ok":
      return "ok";
    case "failed":
      return "err";
    case "skipped":
      return "skip";
    default:
      return "";
  }
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
      ? `示例：${fields.map((f) => f.example).join("_")}.jpg（预览为字段名示例，执行时由视觉模型真实提取）`
      : "";
}

function renderAssets() {
  const body = $("#assets-body");
  const assets = window.App.assets;
  if (assets.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5"><span class="empty-note">尚未扫描，或目录中没有匹配的图片</span></td></tr>';
    $("#asset-summary").textContent = "";
    return;
  }
  body.innerHTML = assets
    .map((a, i) => {
      const selected = window.App.selected.has(i);
      const guess = window.App.previews[i] ?? "";
      const safeName = escapeHtml(a.filename);
      return `<tr>
        <td><input type="checkbox" data-idx="${i}" ${selected ? "checked" : ""} /></td>
        <td class="old-name" title="${safeName}">${safeName}</td>
        <td>${fmtSize(a.size_bytes)}</td>
        <td>${fmtTime(a.modified_secs)}</td>
        <td>${escapeHtml(guess) || "-"}</td>
      </tr>`;
    })
    .join("");
  $("#asset-summary").textContent = `共 ${assets.length} 个文件，已勾选 ${window.App.selected.size} 个`;
  $("#check-all").checked =
    assets.length > 0 && window.App.selected.size === assets.length;
}

function renderLogs() {
  const list = $("#log-list");
  const logs = window.App.logs;
  $("#log-summary").textContent =
    logs.length > 0
      ? `共 ${logs.length} 条`
      : "暂无日志";
  if (logs.length === 0) {
    list.innerHTML = '<li class="empty-note">还没有重命名记录，先扫描并执行一次</li>';
    return;
  }
  // 后端已经倒序返回（最新在前）
  list.innerHTML = logs
    .map(
      (l) => `<li class="${statusText(l.status)}">
        <span class="ts">${escapeHtml(formatTs(l.ts))}</span>
        <span>${escapeHtml(l.asset)}</span>
        ${l.target ? `→ <span class="target">${escapeHtml(l.target)}</span>` : ""}
        ${l.error ? `<span class="err-text">${escapeHtml(l.error)}</span>` : ""}
      </li>`,
    )
    .join("");
}

function formatTs(ts) {
  if (!ts) return "-";
  const d = new Date(ts * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/* ---------------- 浏览器能力探测（无 Tauri 时降级） ---------------- */

function hasTauri() {
  return typeof window.__TAURI__ !== "undefined";
}

async function pickDir() {
  if (!hasTauri()) {
    const p = prompt("（浏览器模式）请输入图片目录绝对路径：");
    if (p) $("#asset-dir").value = p;
    return;
  }
  try {
    const path = await invoke("pick_dir");
    if (path) $("#asset-dir").value = path;
  } catch (err) {
    showRunHint(`选择目录失败：${err}`, "err");
  }
}

/* ---------------- 动作 ---------------- */

function showConfigHint(text, kind = "") {
  const el = $("#config-hint");
  el.textContent = text;
  el.className = `hint ${kind}`;
}

function showRunHint(text, kind = "") {
  const el = $("#run-hint");
  el.textContent = text;
  el.className = `hint ${kind}`;
}

async function saveConfig() {
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
  } catch (err) {
    showConfigHint(String(err), "err");
  }
}

async function loadConfig() {
  try {
    const cfg = await invoke("load_config");
    window.App.config = cfg;
    $("#model-base-url").value = cfg.model.base_url || "";
    $("#model-api-key").value = cfg.model.api_key || "";
    $("#model-name").value = cfg.model.model || "gpt-4o";
    $("#model-timeout").value = cfg.model.timeout_secs ?? 60;
    $("#template-pattern").value = cfg.template.pattern || "";
    renderTemplateFields();
  } catch (err) {
    showConfigHint(`读取配置失败：${err}`, "err");
  }
}

async function scanAssets() {
  const dir = $("#asset-dir").value.trim();
  if (!dir) {
    showRunHint("请先选择目录", "err");
    return;
  }
  setRunning(true);
  try {
    const res = await invoke("scan_assets", { dir });
    window.App.assets = res ?? [];
    window.App.selected.clear();
    window.App.previews = new Array(window.App.assets.length).fill("");
    renderAssets();
    showRunHint(`扫描完成：${window.App.assets.length} 个文件`, "ok");
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
    setRunning(false);
  }
}

async function previewRename() {
  const dir = $("#asset-dir").value.trim();
  const pattern = $("#template-pattern").value.trim();
  if (!dir || !pattern) {
    showRunHint("需要目录和模板", "err");
    return;
  }
  try {
    const previews = await invoke("preview_rename", { dir, pattern });
    window.App.previews = previews ?? [];
    renderAssets();
  } catch (err) {
    showRunHint(String(err), "err");
  }
}

async function executeRename() {
  const dir = $("#asset-dir").value.trim();
  const pattern = $("#template-pattern").value.trim();
  if (!dir || !pattern) {
    showRunHint("需要目录和模板", "err");
    return;
  }
  if (window.App.selected.size === 0) {
    showRunHint("请先勾选要重命名的文件", "err");
    return;
  }
  const selected = window
    .App.selected;
  const paths = window.App.assets.filter((_, i) => selected.has(i)).map((a) => a.path);
  setRunning(true);
  showRunHint("执行中，请稍候…", "");
  try {
    const result = await invoke("execute_rename", {
      dir,
      pattern,
      paths,
      model: window.App.config?.model,
      options: window.App.config?.options,
    });
    window.App.previews.fill("");
    renderAssets();
    await refreshLogs();
    showRunHint(
      `完成：成功 ${result.ok_count} · 失败 ${result.fail_count} · 跳过 ${result.skip_count}`,
      result.fail_count + result.skip_count > 0 ? "" : "ok",
    );
  } catch (err) {
    showRunHint(String(err), "err");
  } finally {
    setRunning(false);
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
    const ok = await invoke("open_log_dir");
    if (typeof ok === "boolean") return;
  } catch (err) {
    showRunHint(String(err), "err");
  }
}

function setRunning(r) {
  window.App.running = r;
  for (const id of ["btn-scan", "btn-execute", "btn-save-config"]) {
    $(`#${id}`).disabled = r;
  }
}

/* ---------------- 工具 ---------------- */

function normalizeBaseUrl(s) {
  if (!s) return "https://api.openai.com/v1";
  return s.replace(/\/+$/, "");
}

function escapeHtml(s) {
  return String(s ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

/**
 * 提取模板中的 {字段}，并按字段名猜测示例值（仅供预览提示）。
 * 真实提取由视觉模型在 execute 阶段完成。
 */
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
  $("#btn-save-config").addEventListener("click", saveConfig);
  $("#btn-pick-dir").addEventListener("click", pickDir);
  $("#btn-scan").addEventListener("click", scanAssets);
  $("#btn-execute").addEventListener("click", executeRename);
  $("#btn-open-log").addEventListener("click", openLogDir);
  $("#btn-refresh-log").addEventListener("click", refreshLogs);

  $("#template-pattern").addEventListener("input", () => {
    renderTemplateFields();
    // 模板变更后旧猜测失效
    window.App.previews = new Array(window.App.assets.length).fill("");
    renderAssets();
  });

  $("#assets-body").addEventListener("change", (e) => {
    const input = e.target;
    if (input.type !== "checkbox" || input.dataset.idx == null) return;
    const i = Number(input.dataset.idx);
    if (input.checked) window.App.selected.add(i);
    else window.App.selected.delete(i);
    renderAssets();
  });

  $("#check-all").addEventListener("change", (e) => {
    if (e.target.checked) {
      window.App.selected = new Set(window.App.assets.map((_, i) => i));
    } else {
      window.App.selected.clear();
    }
    renderAssets();
  });
}

/* ---------------- 启动 ---------------- */

async function boot() {
  bindEvents();
  await loadConfig();
  await refreshLogs();
  if (!hasTauri()) {
    showConfigHint("浏览器模式：未检测到 Tauri 运行时，仅本地演示");
  }
  // 渲染后的模板字段（loadConfig 已触发，再兜底一次）
  renderTemplateFields();
  renderAssets();
  renderLogs();
}

// 监听后端推送（预留：长任务进度）
try {
  await listen("rename-progress", (e) => {
    showRunHint(`进度：${e.payload?.done ?? "?"}/${e.payload?.total ?? "?"}`, "");
  });
} catch {
  // 浏览器模式无 listen，忽略
}

boot();