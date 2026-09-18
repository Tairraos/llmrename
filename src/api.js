/**
 * 后端桥 — 封装 Tauri IPC。浏览器降级（无 Tauri）时返回占位。
 */

export async function invoke(cmd, args = {}) {
  if (typeof window.__TAURI__ !== "undefined") {
    return window.__TAURI__.core.invoke(cmd, args);
  }
  throw new Error(`[浏览器模式] 命令 ${cmd} 不可用，请通过 tauri dev 运行`);
}

export async function listen(event, handler) {
  if (typeof window.__TAURI__ !== "undefined") {
    return window.__TAURI__.event.listen(event, handler);
  }
  throw new Error("[浏览器模式] 事件监听不可用");
}