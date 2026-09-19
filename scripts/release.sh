#!/usr/bin/env bash
# 发布构建：bump 版本 → build（默认只 .app）→ 可选 dmg → 清理中间产物。
#
# 用法:
#   ./scripts/release.sh           # 默认：只构建 .app（debug 校验用）
#   ./scripts/release.sh --dmg     # 构建 .app + .dmg
#
# 行为（对应 AGENTS.md 铁律 8「构建与发布」）:
#   - 每次 build 自动 bump patch 版本（同步 tauri.conf.json / Cargo.toml / package.json）
#   - build 产物默认只出 .app；需要 dmg 时显式传 --dmg
#   - build 完成后清理所有中间产物：target/ 只保留 .app（--dmg 时附加 .dmg），
#     删除 target/debug、target/release 与 dist/；release/<version>/ 留档 .app / .dmg 与 VERSION
#   - 版本号提升的那次 git commit 的 message 必须写明版本号（脚本代劳）
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

# 3) 收集产物到 release/<version>/
RELEASE_DIR="release/$NEW_VER"
mkdir -p "$RELEASE_DIR"
APP_SRC="target/release/bundle/macos/LLM Rename.app"
if [ -d "$APP_SRC" ]; then
  rm -rf "$RELEASE_DIR/LLM Rename.app"
  cp -R "$APP_SRC" "$RELEASE_DIR/LLM Rename.app"
  echo "==> .app 产物: $RELEASE_DIR/LLM Rename.app"
fi
if [ "$DMG" -eq 1 ]; then
  DMG_SRC="target/release/bundle/dmg/LLM Rename_${NEW_VER}_aarch64.dmg"
  # dmg 命名可能含架构后缀，尝试 glob
  shopt -s nullglob
  for f in target/release/bundle/dmg/*.dmg; do
    cp "$f" "$RELEASE_DIR/$(basename "$f")"
    echo "==> .dmg 产物: $RELEASE_DIR/$(basename "$f")"
  done
fi
echo "${NEW_VER}" > "${RELEASE_DIR}/VERSION"

# 4) 清理中间产物：target/ 只保留 .app（--dmg 时附加 .dmg），debug 与编译缓存全部删除
echo "==> 清理构建中间产物…"
rm -rf dist
if [ "$DMG" -eq 1 ]; then
  shopt -s nullglob
  for f in target/release/bundle/dmg/*.dmg; do
    mv "$f" "target/$(basename "$f")"
    echo "==> .dmg 保留在 target/$(basename "$f")"
  done
fi
if [ -d "$APP_SRC" ]; then
  rm -rf "target/LLM Rename.app"
  mv "$APP_SRC" "target/LLM Rename.app"
  echo "==> .app 保留在 target/LLM Rename.app"
fi
rm -rf target/release target/debug target/flycheck0
rm -f target/.rustc_info.json target/CACHEDIR.TAG
# 防止旧工具链再写入迁移前的历史遗留目录
rm -rf src-tauri/target

echo "✅ 构建完成，产物在 ${RELEASE_DIR}/（版本 ${NEW_VER}）"