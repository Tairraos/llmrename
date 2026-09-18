// 前端零构建产物的底线：vite 仅作为开发服务器与打包器，
// 源码保持 纯 HTML/CSS/JS，不引入框架与运行时依赖。
import { defineConfig } from "vite";

export default defineConfig({
  // 供 tauri dev/build 使用
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});