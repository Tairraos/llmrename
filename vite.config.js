// 前端零构建产物的底线：vite 仅作为开发服务器与打包器，
// 源码保持 纯 HTML/CSS/JS，不引入框架与运行时依赖。
import { defineConfig } from "vite";

export default defineConfig({
  // 供 tauri dev/build 使用
  clearScreen: false,
  // 前端源码根目录（index.html 所在），dev 服务器与构建均以此为根
  root: "src",
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // outDir 相对 root 解析；输出到项目根 dist/，对齐 tauri.conf.json 的 frontendDist
    outDir: "../dist",
    emptyOutDir: true,
  },
});