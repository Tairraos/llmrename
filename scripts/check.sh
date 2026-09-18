#!/usr/bin/env bash
# 门禁：本地与 CI 共用的唯一检查入口。
# 用法：./scripts/check.sh  （或 npm run check）
set -euo pipefail

cd "$(dirname "$0")/.."

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