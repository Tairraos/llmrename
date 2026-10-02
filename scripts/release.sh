#!/usr/bin/env bash
# 发布构建：bump 版本 → build（默认只 .app）→ 可选 dmg → 清理打包产物（保留编译缓存）。
#
# 用法:
#   ./scripts/release.sh           # 默认：只构建 .app（debug 校验用）
#   ./scripts/release.sh --dmg     # 构建 .app + .dmg
#
# 行为（对应 AGENTS.md 铁律 6.6「构建与发布」）:
#   - 每次 build 自动 bump patch 版本（同步 tauri.conf.json / Cargo.toml / package.json）
#   - build 产物默认只出 .app；需要 dmg 时显式传 --dmg
#   - .app 交付物文件名带版本号：target/LLM Rename_<版本>.app
#     （target/ 只留最新一份；不建 release/ 留档目录，不留历史版本）
#   - build 完成后保留编译缓存（target/release、target/debug 不删，允许以后增量编译），
#     仅清理可再生成的产物：dist/ 与打包目录 target/release/bundle
#   - 需要释放磁盘空间时由用户手动删除 target/（脚本不代劳）
#   - 版本号提升的那次 git commit 的 message 必须写明版本号（脚本不代劳提交）
set -euo pipefail
cd "$(dirname "$0")/.."

DMG=0
for arg in "$@"; do
  case "$arg" in
    --dmg) DMG=1 ;;
    *) echo "未知参数: $arg（仅支持 --dmg）" >&2; exit 1 ;;
  esac
done

# 1) bump 版本
NEW_VER="$(python3 scripts/_bump_version.py)"
echo "==> 新版本: $NEW_VER"

# 2) 构建（tauri build 以 bundle 配置为准，通过环境变量覆盖 targets）
#    macOS: --bundles app 只出 .app；app,dmg 出两个
if [ "$DMG" -eq 1 ]; then
  BUNDLES="app,dmg"
else
  BUNDLES="app"
fi
pnpm tauri build --bundles "$BUNDLES"

# 3) 交付物只保留 target/ 根一份带版本号的 .app（不留档、不留历史）
APP_SRC="target/release/bundle/macos/LLM Rename.app"
APP_OUT="LLM Rename_${NEW_VER}.app"

# 4) 保留编译缓存（target/release、target/debug 允许以后增量编译），只清理打包产物
echo "==> 清理打包产物（编译缓存保留）…"
rm -rf dist
if [ "$DMG" -eq 1 ]; then
  shopt -s nullglob
  for f in target/release/bundle/dmg/*.dmg; do
    mv "$f" "target/$(basename "$f")"
    echo "==> .dmg 保留在 target/$(basename "$f")"
  done
fi
if [ -d "$APP_SRC" ]; then
  # target/ 根始终只有一份带版本号的最新交付物（旧版本直接被替换，不留历史）
  rm -rf "target/LLM Rename.app" target/LLM\ Rename_*.app
  mv "$APP_SRC" "target/$APP_OUT"
  echo "==> .app 保留在 target/$APP_OUT"
fi
# 打包目录是 .app / .dmg 的重复副本且可随时重新生成，删掉；其余编译缓存全部保留
rm -rf "target/release/bundle"
# 防止旧工具链再写入迁移前的历史遗留目录
rm -rf src-tauri/target

echo "✅ 构建完成，产物：target/$APP_OUT（版本 ${NEW_VER}）"