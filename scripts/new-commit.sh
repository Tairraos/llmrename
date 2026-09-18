#!/usr/bin/env bash
# 铁律提交辅助：一次提交 = 一个功能变更，message 需带前缀与详细 body。
# 用法：./scripts/new-commit.sh <type>: <summary>
# 例：./scripts/new-commit.sh "feat: 新增扫描命令"
# 然后按提示粘贴 body（可多行，空行结束）。
set -euo pipefail

cd "$(dirname "$0")/.."

if [ $# -eq 0 ]; then
  echo "用法: $0 '<type>: <summary>'" >&2
  echo "type: init|feat|fix|chore|docs|refactor|test" >&2
  exit 1
fi

TITLE="$*"

git add -A
git commit -m "$TITLE"
echo "已提交：$TITLE"
echo "提示：若需详细 body，请用 git commit --amend 补写。"