#!/usr/bin/env bash
# 门禁：本地与 CI 共用的唯一检查入口。
# 用法：./scripts/check.sh  （或 npm run check）
set -euo pipefail

cd "$(dirname "$0")/.."

# cargo 的配置文件按 CWD 向上发现，--manifest-path 不会让 src-tauri/.cargo/config.toml 生效，
# 不显式指定时构建目录会落在 src-tauri/target（与 AGENTS.md 6.6.1 的项目根 target/ 矛盾）。
export CARGO_TARGET_DIR="$PWD/target"

echo "==> 1/4 cargo fmt --check"
cargo fmt --check --manifest-path src-tauri/Cargo.toml

echo "==> 2/4 cargo clippy --all-targets -- -D warnings"
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

echo "==> 3/4 cargo test"
cargo test --manifest-path src-tauri/Cargo.toml

echo "==> 4/4 node --check（前端语法）"
for f in src/*.js; do
  echo "    $f"
  node --check "$f"
done

echo "✅ 门禁全部通过"